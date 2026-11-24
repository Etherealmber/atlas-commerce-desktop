//! Clients (CRM): searchable list with the worker-computed balances,
//! a detail panel and a create/edit editor. The wire contract is
//! `worker/routes/clients.ts`; validation mirrors `validateClientWrite`.

use atlas_api::ApiError;
use atlas_api::models::{Client, ClientWrite};
use atlas_core::format::money;
use atlas_core::jobs::Job;
use egui::{RichText, Sense, vec2};

use crate::icons::Icon;
use crate::state::AtlasApp;
use crate::theme::Palette;
use crate::widgets::{
    card, empty_state, error_banner, icon, info_banner, labelled_field, page_title, primary_button,
    secondary_button,
};
use crate::{t, widgets};

/// Web-compatible ten-second undo followed by an explicit confirmation.
pub(crate) struct ClientDelete {
    id: i64,
    name: String,
    deadline: std::time::Instant,
    pub(crate) confirming: bool,
    job: Option<Job<Result<atlas_api::clients::ClientDeletion, ApiError>>>,
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
    error: Option<String>,
}

impl ClientDelete {
    pub(crate) fn saving(&self) -> bool {
        self.job.is_some()
    }
}
impl Drop for ClientDelete {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
}

pub(crate) fn poll_client_delete(app: &mut AtlasApp, ctx: &egui::Context) {
    let on_clients = matches!(
        app.screen,
        crate::state::Screen::Shell(crate::state::Route::Clients)
    );
    if let Some(pending) = app.client_delete.as_mut() {
        if pending.job.is_none() && !on_clients {
            app.client_delete = None;
            return;
        }
        if !pending.confirming && pending.deadline <= std::time::Instant::now() {
            pending.confirming = true;
            ctx.request_repaint();
        }
        if let Some(result) = pending.job.as_ref().and_then(Job::poll) {
            pending.job = None;
            pending.cancel = None;
            match result {
                Ok(result) => {
                    let id = pending.id;
                    app.client_delete = None;
                    if app.selected_client == Some(id) {
                        app.selected_client = None;
                        app.client_invoice_id = None;
                        app.client_invoices = Default::default();
                        app.invoice_detail_id = None;
                        app.invoice_detail = Default::default();
                        app.client_report_id = None;
                        app.client_report = Default::default();
                    }
                    app.notice(
                        result
                            .message
                            .unwrap_or_else(|| t!(app.locale, "Client deleted.").into_owned()),
                    );
                    app.load_clients();
                }
                Err(error) => pending.error = Some(error.message),
            }
            ctx.request_repaint();
        } else if !pending.confirming || pending.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

fn delete_client_button(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette, client: &Client) {
    let current = app
        .client_delete
        .as_ref()
        .filter(|pending| pending.id == client.id);
    if let Some(pending) = current {
        if !pending.confirming {
            let seconds = pending
                .deadline
                .saturating_duration_since(std::time::Instant::now())
                .as_secs_f32()
                .ceil() as u32;
            if secondary_button(
                ui,
                format!("{} ({seconds})", t!(app.locale, "Undo")),
                palette,
            ) {
                app.client_delete = None;
            }
        }
    } else {
        ui.add_enabled_ui(app.client_delete.is_none(), |ui| {
            if secondary_button(ui, t!(app.locale, "Delete"), palette) {
                app.client_delete = Some(ClientDelete {
                    id: client.id,
                    name: client.name.clone(),
                    deadline: std::time::Instant::now() + std::time::Duration::from_secs(10),
                    confirming: false,
                    job: None,
                    cancel: None,
                    error: None,
                });
                ui.ctx().request_repaint();
            }
        });
    }
}

fn draw_delete_confirmation(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let Some(pending) = app
        .client_delete
        .as_ref()
        .filter(|pending| pending.confirming)
    else {
        return;
    };
    let name = pending.name.clone();
    let saving = pending.saving();
    let error = pending.error.clone();
    let screen = ui.ctx().input(|i| i.viewport_rect());
    let rect = egui::Rect::from_center_size(
        screen.center(),
        vec2(
            440.0_f32.min((screen.width() - 32.0).max(1.0)),
            230.0_f32.min((screen.height() - 32.0).max(1.0)),
        ),
    );
    if !saving && widgets::scrim_clicked(ui, screen, rect, egui::Id::new("client-delete-scrim")) {
        app.client_delete = None;
        return;
    }
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(t!(app.locale, "Delete client"))
                    .strong()
                    .color(palette.text)
                    .size(17.0),
            );
            ui.add_space(8.0);
            ui.label(format!(
                "{} : {}",
                t!(app.locale, "This action is permanent. Confirm to continue."),
                name
            ));
            if let Some(error) = error {
                error_banner(ui, &error, palette);
            }
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                if primary_button(ui, t!(app.locale, "Delete"), palette, !saving)
                    && let (Some(api), Some(pending)) =
                        (app.client.clone(), app.client_delete.as_mut())
                {
                    let id = pending.id;
                    let (cancel, cancelled) = tokio::sync::oneshot::channel();
                    pending.cancel = Some(cancel);
                    pending.error = None;
                    pending.job = Some(app.runner.spawn(async move {
                        tokio::select! {
                            result = api.delete_client(id) => result,
                            _ = cancelled => Err(ApiError::client("Suppression annulée.")),
                        }
                    }));
                }
                if !saving && secondary_button(ui, t!(app.locale, "Cancel"), palette) {
                    app.client_delete = None;
                }
            });
        });
    });
}

/// Create/edit form. `editing_id` is `None` for creation.
#[derive(Default)]
pub struct ClientEditor {
    pub editing_id: Option<i64>,
    pub step: usize,
    pub name: String,
    pub ice: String,
    pub if_number: String,
    pub address: String,
    pub city: String,
    pub phone: String,
    pub email: String,
    pub credit_limit: String,
    pub tax_exempt: bool,
    pub saving: bool,
    pub error: Option<String>,
    pub job: Option<Job<Result<Client, ApiError>>>,
}

impl ClientEditor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            credit_limit: "0".to_owned(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn for_client(client: &Client) -> Self {
        Self {
            editing_id: Some(client.id),
            name: client.name.clone(),
            ice: client.ice.clone(),
            if_number: client.if_number.clone(),
            address: client.address.clone(),
            city: client.city.clone(),
            phone: client.phone.clone(),
            email: client.email.clone(),
            credit_limit: format_float(client.credit_limit),
            tax_exempt: client.tax_exempt != 0,
            ..Self::new()
        }
    }

    /// Worker rules (`validateClientWrite`): name 1..=200, optional valid
    /// email, credit limit 0..=1e12, phone and ICE at most 40 characters.
    /// Returns the English source string; the caller translates it.
    fn validate(&self) -> Option<&'static str> {
        if self.name.trim().is_empty() {
            return Some("The name is required.");
        }
        if self.name.chars().count() > 200 {
            return Some("The name must be 200 characters or fewer.");
        }
        if self.ice.chars().count() > 40 {
            return Some("ICE is too long (40 characters max).");
        }
        if self.phone.chars().count() > 40 {
            return Some("Phone is too long (40 characters max).");
        }
        let email = self.email.trim();
        if !email.is_empty() && (!email.contains('@') || !email.contains('.')) {
            return Some("The e-mail address is invalid.");
        }
        match self.credit_limit.trim().parse::<f64>() {
            Ok(limit) if (0.0..=1e12).contains(&limit) => None,
            _ => Some("The credit limit must be a number between 0 and 1,000,000,000,000."),
        }
    }

    fn to_write(&self) -> ClientWrite {
        ClientWrite {
            name: self.name.trim().to_owned(),
            ice: self.ice.trim().to_owned(),
            if_number: self.if_number.trim().to_owned(),
            address: self.address.trim().to_owned(),
            city: self.city.trim().to_owned(),
            phone: self.phone.trim().to_owned(),
            email: self.email.trim().to_owned(),
            tax_exempt: self.tax_exempt,
            credit_limit: self.credit_limit.trim().parse().unwrap_or(0.0),
        }
    }
}

/// Formats a stored decimal without a trailing `.0` when it is integral.
fn format_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    poll_editor(app);
    if let Some(id) = app.client_report_id {
        let client = app
            .clients
            .data
            .as_ref()
            .and_then(|items| items.iter().find(|item| item.id == id))
            .cloned();
        if let Some(client) = client {
            ui.add_space(8.0);
            draw_client_report(app, ui, palette, &client);
            return;
        }
        app.client_report_id = None;
        app.client_report = Default::default();
    }

    ui.add_space(8.0);
    page_title(ui, t!(app.locale, "Clients"), palette);
    ui.add_space(12.0);

    if let Some(error) = app.clients.error.clone() {
        error_banner(ui, &error, palette);
        ui.add_space(10.0);
    }

    if let Some(clients) = &app.clients.data {
        let currency = app
            .dashboard
            .data
            .as_ref()
            .map(|d| d.currency())
            .unwrap_or("MAD");
        let total: f64 = clients.iter().map(|client| client.balance_due).sum();
        let count = clients.len();
        ui.horizontal_wrapped(|ui| {
            let width = ((ui.available_width() - ui.spacing().item_spacing.x) / 2.0).max(180.0);
            for (label, value) in [
                (
                    t!(app.locale, "Due balance of loaded clients"),
                    money(total, currency),
                ),
                (t!(app.locale, "Loaded clients"), count.to_string()),
            ] {
                card(palette).show(ui, |ui| {
                    ui.set_width(width - 32.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(label).color(palette.secondary).size(11.0));
                        ui.label(RichText::new(value).color(palette.text).strong().size(18.0));
                    });
                });
            }
        });
        ui.add_space(12.0);
    }

    if app.role_policy().can_view_reports() {
        ui.horizontal_wrapped(|ui| {
            if secondary_button(ui, t!(app.locale, "Client receivables report"), palette) {
                app.load_receivables_report();
            }
            ui.label(
                RichText::new(t!(app.locale, "Official server report · administrators"))
                    .color(palette.secondary)
                    .size(12.0),
            );
        });
        ui.add_space(12.0);
        if app.receivables_open {
            draw_receivables_report(app, ui, palette);
            ui.add_space(12.0);
        }
    }

    // Full-width list, like the web page. The former horizontal card layout
    // allowed the first card to consume the remaining width, squeezing rows.
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        client_list(app, ui, palette);
    });
    if app.selected_client.is_some() {
        ui.add_space(12.0);
        card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            client_detail(app, ui, palette);
        });
    }
}

pub(super) fn has_dialogs(app: &AtlasApp) -> bool {
    app.client_editor.is_some()
        || app
            .client_delete
            .as_ref()
            .is_some_and(|pending| pending.confirming)
}

pub(super) fn draw_dialogs(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    if app.client_editor.is_some() {
        draw_editor(app, ui, palette);
    }
    if app
        .client_delete
        .as_ref()
        .is_some_and(|pending| pending.confirming)
    {
        draw_delete_confirmation(app, ui, palette);
    }
}

fn client_list(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.horizontal(|ui| {
        icon(ui, Icon::Search, palette.secondary, 16.0);
        let field_width = ui.available_width() - 24.0;
        widgets::search_field(
            ui,
            &mut app.client_search,
            t!(app.locale, "Search by name, city, phone or ICE"),
            field_width,
        );
    });
    ui.add_space(10.0);

    if app.clients.loading && app.clients.data.is_none() {
        super::list_skeleton(ui, palette, app.locale, app.settings.reduced_motion);
        return;
    }
    if app.clients.error.is_some() && app.clients.data.is_none() {
        return;
    }
    let clients = app.clients.data.clone().unwrap_or_default();
    if clients.is_empty() {
        empty_state(
            ui,
            Icon::Users,
            t!(app.locale, "No client yet"),
            t!(app.locale, "Created clients appear here."),
            palette,
        );
        return;
    }

    let query = app.client_search.to_lowercase();
    let currency = app
        .dashboard
        .data
        .as_ref()
        .map(|data| data.currency())
        .unwrap_or("MAD")
        .to_owned();
    let selected = app.selected_client;
    for client in clients.iter().filter(|client| {
        let q = query.as_str();
        query.is_empty()
            || client.name.to_lowercase().contains(q)
            || client.city.to_lowercase().contains(q)
            || client.phone.contains(q)
            || client.ice.to_lowercase().contains(q)
    }) {
        let is_active = selected == Some(client.id);
        let height = 58.0;
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
        if is_active {
            ui.painter().rect_filled(rect, 10.0, palette.surface_active);
        } else if response.hovered() {
            ui.painter().rect_filled(rect, 10.0, palette.surface_hover);
        }
        if response.clicked() {
            select_client(app, client.id);
        }
        widgets::overlay_at(ui, rect.shrink2(vec2(8.0, 0.0)), |ui| {
            // Explicit bounded columns keep long names from consuming the
            // balance column or wrapping into a one-character-wide strip.
            let width = ui.available_width();
            ui.horizontal(|ui| {
                icon(ui, Icon::User, palette.secondary, 18.0);
                let wide = width >= 850.0;
                let text_width = (width - if wide { 550.0 } else { 160.0 }).max(40.0);
                ui.allocate_ui_with_layout(
                    vec2(text_width, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&client.name)
                                    .color(palette.text)
                                    .strong()
                                    .size(13.5),
                            )
                            .truncate(),
                        );
                        let subline = [client.city.as_str(), client.phone.as_str()]
                            .into_iter()
                            .filter(|part| !part.is_empty())
                            .collect::<Vec<_>>()
                            .join(" · ");
                        ui.add(
                            egui::Label::new(
                                RichText::new(if subline.is_empty() { "—" } else { &subline })
                                    .color(palette.dim)
                                    .size(11.5),
                            )
                            .truncate(),
                        );
                    },
                );
                ui.label(
                    RichText::new(money(client.balance_due, &currency))
                        .color(palette.text)
                        .strong()
                        .size(12.5),
                );
                if wide {
                    if secondary_button(ui, t!(app.locale, "Invoices"), palette) {
                        select_client(app, client.id);
                        app.open_client_invoices(client);
                    }
                    if secondary_button(ui, t!(app.locale, "Situation"), palette) {
                        select_client(app, client.id);
                        app.load_client_report(client, false);
                    }
                    if secondary_button(ui, t!(app.locale, "Edit"), palette) {
                        app.client_editor = Some(ClientEditor::for_client(client));
                    }
                    delete_client_button(app, ui, palette, client);
                }
            });
        });
        ui.separator();
    }
}

fn select_client(app: &mut AtlasApp, id: i64) {
    if app.selected_client != Some(id) {
        app.client_invoice_id = None;
        app.client_invoices = Default::default();
        app.invoice_detail_id = None;
        app.invoice_detail = Default::default();
        app.client_report_id = None;
        app.client_report = Default::default();
    }
    app.selected_client = Some(id);
}

fn client_detail(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let clients = app.clients.data.clone().unwrap_or_default();
    let Some(client) = clients
        .iter()
        .find(|item| Some(item.id) == app.selected_client)
    else {
        empty_state(
            ui,
            Icon::Users,
            t!(app.locale, "No client selected"),
            t!(
                app.locale,
                "Choose a client on the left to see its details."
            ),
            palette,
        );
        return;
    };

    // Owned `String`: the action closure below needs a unique `&mut app`,
    // so no borrow of `app.dashboard` may still be alive when it runs.
    let currency = app
        .dashboard
        .data
        .as_ref()
        .map(|data| data.currency())
        .unwrap_or("MAD")
        .to_owned();

    ui.label(
        RichText::new(client.name.clone())
            .color(palette.text)
            .font(crate::fonts::semibold(17.0)),
    );
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        if secondary_button(ui, t!(app.locale, "Invoices"), palette) {
            app.open_client_invoices(client);
        }
        if secondary_button(ui, t!(app.locale, "Situation"), palette) {
            app.load_client_report(client, false);
        }
        if secondary_button(ui, t!(app.locale, "Full detail"), palette) {
            app.load_client_report(client, true);
        }
        if secondary_button(ui, t!(app.locale, "Edit"), palette) {
            app.client_editor = Some(ClientEditor::for_client(client));
        }
        delete_client_button(app, ui, palette, client);
    });
    ui.add_space(10.0);

    if client.balance_due > 0.001 {
        info_banner(
            ui,
            format!(
                "{} : {}",
                t!(app.locale, "Outstanding balance"),
                money(client.balance_due, &currency)
            ),
            palette,
        );
        ui.add_space(10.0);
    }

    detail_row(
        ui,
        palette,
        t!(app.locale, "Credit limit"),
        if client.credit_limit > 0.0 {
            money(client.credit_limit, &currency)
        } else {
            t!(app.locale, "Unlimited").into_owned()
        },
    );
    detail_row(
        ui,
        palette,
        t!(app.locale, "VAT exempt"),
        if client.tax_exempt != 0 {
            t!(app.locale, "Yes").into_owned()
        } else {
            t!(app.locale, "No").into_owned()
        },
    );
    detail_row(ui, palette, "ICE", blank(&client.ice));
    detail_row(
        ui,
        palette,
        t!(app.locale, "Tax ID (IF)"),
        blank(&client.if_number),
    );
    detail_row(ui, palette, t!(app.locale, "Phone"), blank(&client.phone));
    detail_row(ui, palette, t!(app.locale, "Email"), blank(&client.email));
    detail_row(
        ui,
        palette,
        t!(app.locale, "Address"),
        blank(&client.address),
    );
    detail_row(ui, palette, t!(app.locale, "City"), blank(&client.city));
    if app.client_invoice_id == Some(client.id) {
        ui.add_space(14.0);
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(t!(app.locale, "Invoices"))
                    .color(palette.text)
                    .strong(),
            );
            if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette) {
                app.open_client_invoices(client);
            }
            if secondary_button(ui, t!(app.locale, "Open in browser"), palette) {
                app.open_client_invoices_in_browser(client);
            }
        });
        if app.client_invoices.loading {
            super::centered_spinner(ui, palette);
        } else if let Some(error) = &app.client_invoices.error {
            error_banner(ui, error, palette);
        } else if let Some(invoices) = app.client_invoices.data.clone() {
            // Match the web Factures page; the separate server situation
            // report also includes quick-sale tickets and credit notes.
            let invoices: Vec<_> = invoices
                .into_iter()
                .filter(crate::client_invoices::is_standard_invoice)
                .collect();
            ui.label(RichText::new(t!(app.locale, "This list excludes quotes, quick-sale tickets and credit notes; see Situation for the server report.")).color(palette.secondary).size(12.0));
            if invoices.is_empty() {
                ui.label(
                    RichText::new(t!(app.locale, "No invoice for this client."))
                        .color(palette.secondary),
                );
            }
            for invoice in invoices {
                ui.separator();
                if secondary_button(ui, t!(app.locale, "View"), palette) {
                    app.open_invoice_detail(invoice.id);
                }
                ui.label(
                    RichText::new(
                        invoice
                            .number
                            .clone()
                            .unwrap_or_else(|| format!("#{}", invoice.id)),
                    )
                    .color(palette.text)
                    .strong(),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(invoice.issue_date.as_deref().unwrap_or("—"))
                            .color(palette.secondary),
                    );
                    if let Some(status) = invoice.status.as_deref() {
                        super::dashboard::status_pill(ui, palette, status);
                    }
                });
                ui.label(
                    RichText::new(format!(
                        "{} : {} · {} : {}",
                        t!(app.locale, "Total"),
                        money(invoice.total, &currency),
                        t!(app.locale, "Remaining"),
                        money(invoice.remaining(), &currency)
                    ))
                    .color(palette.text),
                );
            }
        }
        let detail_in_list = app.invoice_detail_id.is_some_and(|id| {
            !app.client_invoices.loading
                && app.client_invoices.error.is_none()
                && app.client_invoices.data.as_ref().is_some_and(|invoices| {
                    invoices.iter().any(|invoice| {
                        invoice.id == id && crate::client_invoices::is_standard_invoice(invoice)
                    })
                })
        });
        if detail_in_list {
            ui.add_space(12.0);
            super::sales::draw_invoice_detail(app, ui, palette);
        }
    }
}

/// The report is distinct from the sum of the currently loaded client list.
fn draw_receivables_report(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t!(app.locale, "Client receivables report")).strong().size(16.0).color(palette.text));
            if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette) {
                app.load_receivables_report();
            }
            if secondary_button(ui, t!(app.locale, "Print / PDF"), palette) {
                app.open_printable("/api/reports/receivables", &[]);
            }
            if secondary_button(ui, t!(app.locale, "Close"), palette) {
                app.receivables_open = false;
                app.receivables_report = Default::default();
            }
        });
        ui.add_space(8.0);
        ui.label(RichText::new(t!(app.locale, "Server figures may differ from the loaded client list. Print / PDF opens the official report in your browser.")).color(palette.secondary).size(12.0));
        if app.receivables_report.loading {
            super::centered_spinner(ui, palette);
        } else if let Some(error) = &app.receivables_report.error {
            error_banner(ui, error, palette);
        } else if let Some(report) = &app.receivables_report.data {
            draw_report_blocks(ui, palette, report);
        }
    });
}

fn draw_client_report(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette, client: &Client) {
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t!(app.locale, "Client situation")).strong().size(16.0).color(palette.text));
            if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette) {
                app.load_client_report(client, app.client_report_full);
            }
            if secondary_button(ui, t!(app.locale, "Print / PDF"), palette) {
                app.open_client_situation(client, app.client_report_full);
            }
            if secondary_button(ui, t!(app.locale, "Close"), palette) {
                app.client_report_id = None;
                app.client_report = Default::default();
            }
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            let summary = t!(app.locale, "Summary");
            let full = t!(app.locale, "Full detail (products)");
            if let Some(choice) = widgets::segmented(ui, palette,
                &[(summary.as_ref(), false), (full.as_ref(), true)],
                Some(app.client_report_full))
                && choice != app.client_report_full
            {
                app.load_client_report(client, choice);
            }
        });
        ui.add_space(6.0);
        ui.label(RichText::new(t!(app.locale, "Amounts and documents are provided by the server. Print / PDF opens the official report in your browser.")).color(palette.secondary).size(12.0));
        ui.add_space(10.0);
        if app.client_report.loading {
            super::centered_spinner(ui, palette);
            return;
        }
        if let Some(error) = &app.client_report.error {
            error_banner(ui, error, palette);
            return;
        }
        let Some(report) = &app.client_report.data else { return; };
        draw_report_blocks(ui, palette, report);
    });
}

/// Render only inert text and values returned by the worker, without calculations.
pub(crate) fn draw_report_blocks(
    ui: &mut egui::Ui,
    palette: &Palette,
    report: &crate::client_report::ClientReport,
) {
    use crate::client_report::ReportBlock;
    let mut index = 0;
    while index < report.blocks.len() {
        match &report.blocks[index] {
            ReportBlock::Heading(value) => {
                ui.add_space(12.0);
                ui.label(RichText::new(value).color(palette.text).strong().size(15.0));
                ui.add_space(4.0);
                index += 1;
            }
            ReportBlock::Statistic { .. } => {
                let start = index;
                while index < report.blocks.len()
                    && matches!(&report.blocks[index], ReportBlock::Statistic { .. })
                {
                    index += 1;
                }
                let columns = if ui.available_width() >= 640.0 {
                    4
                } else if ui.available_width() >= 320.0 {
                    2
                } else {
                    1
                };
                for chunk in report.blocks[start..index].chunks(columns) {
                    ui.columns(columns, |uis| {
                        for (column, block) in uis.iter_mut().zip(chunk) {
                            if let ReportBlock::Statistic { label, value } = block {
                                egui::Frame::NONE
                                    .fill(palette.surface_hover)
                                    .corner_radius(8.0)
                                    .inner_margin(egui::Margin::same(10))
                                    .show(column, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.label(
                                            RichText::new(label)
                                                .color(palette.secondary)
                                                .size(11.0),
                                        );
                                        ui.label(
                                            RichText::new(value)
                                                .color(palette.text)
                                                .strong()
                                                .size(15.0),
                                        );
                                    });
                            }
                        }
                    });
                    ui.add_space(6.0);
                }
            }
            ReportBlock::Text(value) => {
                ui.label(RichText::new(value).color(palette.secondary).size(12.5));
                index += 1;
            }
            ReportBlock::Table(rows) => {
                draw_report_rows(ui, palette, rows);
                index += 1;
            }
        }
    }
}

/// Use compact, wrapping rows rather than an unscrollable six-column table.
/// Headings are retained verbatim from the report and an empty colspan row
/// is displayed as-is instead of being mislabeled as a document number.
fn draw_report_rows(
    ui: &mut egui::Ui,
    palette: &Palette,
    rows: &[crate::client_report::ReportRow],
) {
    let mut headings: Option<&[String]> = None;
    for row in rows {
        if row.header {
            headings = Some(&row.cells);
            continue;
        }
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(16.0, 6.0);
            for (index, value) in row.cells.iter().enumerate() {
                ui.vertical(|ui| {
                    if let Some(name) = headings
                        .filter(|h| h.len() == row.cells.len())
                        .and_then(|h| h.get(index))
                    {
                        ui.label(RichText::new(name).color(palette.secondary).size(10.5));
                    }
                    ui.label(RichText::new(value).color(palette.text).size(12.5));
                });
            }
        });
        ui.add_space(6.0);
    }
}

fn blank(value: &str) -> &str {
    if value.trim().is_empty() {
        "—"
    } else {
        value
    }
}

fn detail_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    label: impl AsRef<str>,
    value: impl AsRef<str>,
) {
    ui.horizontal(|ui| {
        // No set_width(available_width): pinning the whole row to the full
        // available width starved the value column until label and value
        // overlapped. Let the label take its natural width and push the
        // value to the right edge instead.
        ui.label(
            RichText::new(label.as_ref())
                .color(palette.secondary)
                .size(12.5),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value.as_ref()).color(palette.text).size(13.0));
        });
    });
    ui.add_space(6.0);
}

fn draw_editor(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let screen = ui.ctx().input(|input| input.viewport_rect());
    let width = 720.0_f32.min((screen.width() - 32.0).max(1.0));
    let rect = egui::Rect::from_center_size(
        screen.center(),
        vec2(width, 620.0_f32.min((screen.height() - 32.0).max(1.0))),
    );
    if widgets::scrim_clicked(ui, screen, rect, egui::Id::new("client-editor-scrim"))
        && !app
            .client_editor
            .as_ref()
            .is_some_and(|editor| editor.saving)
    {
        app.client_editor = None;
        return;
    }

    let locale = app.locale;
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_width((width - 44.0).max(1.0));
            let title = if app
                .client_editor
                .as_ref()
                .is_some_and(|editor| editor.editing_id.is_some())
            {
                t!(locale, "Edit client")
            } else {
                t!(locale, "New client")
            };
            ui.label(
                RichText::new(title)
                    .color(palette.text)
                    .font(crate::fonts::semibold(17.0)),
            );
            ui.add_space(10.0);
            egui::ScrollArea::vertical()
                .id_salt((
                    "client-wizard-step",
                    app.client_editor.as_ref().map_or(0, |editor| editor.step),
                ))
                .max_height((rect.height() - 210.0).max(1.0))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(editor) = app.client_editor.as_mut() {
                        match editor.step {
                            0 => {
                                labelled_field(
                                    ui,
                                    t!(locale, "Name"),
                                    &mut editor.name,
                                    palette,
                                    false,
                                );
                                ui.add_space(8.0);
                                labelled_field(ui, "ICE", &mut editor.ice, palette, false);
                                ui.add_space(8.0);
                                labelled_field(
                                    ui,
                                    t!(locale, "Tax ID (IF)"),
                                    &mut editor.if_number,
                                    palette,
                                    false,
                                );
                            }
                            1 => {
                                labelled_field(
                                    ui,
                                    t!(locale, "Address"),
                                    &mut editor.address,
                                    palette,
                                    false,
                                );
                                ui.add_space(8.0);
                                labelled_field(
                                    ui,
                                    t!(locale, "City"),
                                    &mut editor.city,
                                    palette,
                                    false,
                                );
                                ui.add_space(8.0);
                                labelled_field(
                                    ui,
                                    t!(locale, "Phone"),
                                    &mut editor.phone,
                                    palette,
                                    false,
                                );
                                ui.add_space(8.0);
                                labelled_field(
                                    ui,
                                    t!(locale, "Email"),
                                    &mut editor.email,
                                    palette,
                                    false,
                                );
                            }
                            _ => {
                                ui.checkbox(
                                    &mut editor.tax_exempt,
                                    t!(locale, "VAT exempt").to_string(),
                                );
                                ui.add_space(10.0);
                                labelled_field(
                                    ui,
                                    t!(locale, "Credit limit (MAD)"),
                                    &mut editor.credit_limit,
                                    palette,
                                    false,
                                );
                            }
                        }
                    }

                    if let Some(error) = app
                        .client_editor
                        .as_ref()
                        .and_then(|editor| editor.error.clone())
                    {
                        ui.add_space(8.0);
                        error_banner(ui, &error, palette);
                    }
                });
            ui.add_space(12.0);
            let step = app.client_editor.as_ref().map_or(0, |editor| editor.step);
            let step_name = match step {
                0 => t!(locale, "Identity"),
                1 => t!(locale, "Contact"),
                _ => t!(locale, "Options"),
            };
            ui.label(
                RichText::new(format!(
                    "{} {}/3 · {}",
                    t!(locale, "Step"),
                    step + 1,
                    step_name
                ))
                .color(palette.secondary),
            );
            ui.add(
                egui::ProgressBar::new((step + 1) as f32 / 3.0).desired_width(ui.available_width()),
            );
            widgets::dialog_actions(ui, |ui| {
                let saving = app
                    .client_editor
                    .as_ref()
                    .is_some_and(|editor| editor.saving);
                if step == 2 {
                    if primary_button(ui, t!(locale, "Save"), palette, !saving) {
                        submit_editor(app);
                    }
                } else if primary_button(ui, t!(locale, "Next"), palette, !saving)
                    && let Some(editor) = app.client_editor.as_mut()
                {
                    editor.step += 1;
                    editor.error = None;
                }
                if step > 0
                    && !saving
                    && secondary_button(ui, t!(locale, "Previous"), palette)
                    && let Some(editor) = app.client_editor.as_mut()
                {
                    editor.step -= 1;
                    editor.error = None;
                }
                if secondary_button(ui, t!(locale, "Cancel"), palette) && !saving {
                    app.client_editor = None;
                }
            });
        });
    });
}

fn poll_editor(app: &mut AtlasApp) {
    if let Some(editor) = app.client_editor.as_mut()
        && let Some(job) = &editor.job
        && let Some(result) = job.poll()
    {
        editor.job = None;
        editor.saving = false;
        match result {
            Ok(client) => {
                let was_edit = editor.editing_id.is_some();
                app.client_editor = None;
                app.selected_client = Some(client.id);
                app.notice(if was_edit {
                    t!(app.locale, "Client updated.").into_owned()
                } else {
                    t!(app.locale, "Client created.").into_owned()
                });
                app.load_clients();
            }
            Err(error) => {
                if let Some(editor) = app.client_editor.as_mut() {
                    editor.error = Some(error.message);
                }
            }
        }
    }
}

fn submit_editor(app: &mut AtlasApp) {
    let Some(client_api) = app.client.clone() else {
        return;
    };
    let Some(editor) = app.client_editor.as_mut() else {
        return;
    };
    if let Some(validation) = editor.validate() {
        editor.step = match validation {
            "The name is required."
            | "The name must be 200 characters or fewer."
            | "ICE is too long (40 characters max)." => 0,
            "Phone is too long (40 characters max)." | "The e-mail address is invalid." => 1,
            _ => 2,
        };
        editor.error = Some(crate::i18n::gettext(app.locale, validation).into_owned());
        return;
    }
    let body = editor.to_write();
    let editing_id = editor.editing_id;
    let job = app.runner.spawn(async move {
        match editing_id {
            Some(id) => client_api.update_client(id, &body).await,
            None => client_api.create_client(&body).await,
        }
    });
    editor.saving = true;
    editor.error = None;
    editor.job = Some(job);
}
