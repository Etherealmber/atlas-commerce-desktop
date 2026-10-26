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

/// Create/edit form. `editing_id` is `None` for creation.
#[derive(Default)]
pub struct ClientEditor {
    pub editing_id: Option<i64>,
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

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        page_title(ui, t!(app.locale, "Clients"), palette);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if primary_button(ui, t!(app.locale, "New client"), palette, true) {
                app.client_editor = Some(ClientEditor::new());
            }
            if secondary_button(ui, t!(app.locale, "Refresh"), palette) {
                app.load_clients();
            }
        });
    });
    ui.add_space(12.0);

    if let Some(error) = app.clients.error.clone() {
        error_banner(ui, &error, palette);
        ui.add_space(10.0);
    }

    ui.horizontal_top(|ui| {
        card(palette).show(ui, |ui| {
            ui.set_width((ui.available_width() * 0.55).max(300.0));
            ui.vertical(|ui| {
                client_list(app, ui, palette);
            });
        });
        card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical(|ui| {
                client_detail(app, ui, palette);
            });
        });
    });

    if app.client_editor.is_some() {
        draw_editor(app, ui, palette);
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
        super::centered_spinner(ui, palette);
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
        .unwrap_or("MAD");
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
            app.selected_client = Some(client.id);
        }
        widgets::overlay_at(
            ui,
            egui::Rect::from_min_size(rect.min + vec2(8.0, 0.0), rect.size() - vec2(16.0, 0.0)),
            |ui| {
                ui.horizontal_centered(|ui| {
                    icon(ui, Icon::User, palette.secondary, 18.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(client.name.clone())
                                .color(palette.text)
                                .strong()
                                .size(13.5),
                        );
                        let subline = [client.city.clone(), client.phone.clone()]
                            .into_iter()
                            .filter(|part| !part.is_empty())
                            .collect::<Vec<_>>()
                            .join(" · ");
                        ui.label(
                            RichText::new(if subline.is_empty() {
                                "—".to_owned()
                            } else {
                                subline
                            })
                            .color(palette.dim)
                            .size(11.5),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let tint = if client.balance_due > 0.001 {
                            palette.text
                        } else {
                            palette.dim
                        };
                        ui.label(
                            RichText::new(money(client.balance_due, currency))
                                .color(tint)
                                .strong()
                                .size(12.5),
                        );
                    });
                });
            },
        );
        ui.separator();
    }
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

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(client.name.clone())
                .color(palette.text)
                .font(crate::fonts::semibold(17.0)),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if secondary_button(ui, t!(app.locale, "Edit"), palette) {
                app.client_editor = Some(ClientEditor::for_client(client));
            }
            // Web parity: the printable situation report opens in the system
            // browser through a short-lived report ticket (never the JWT).
            if secondary_button(ui, t!(app.locale, "Full detail"), palette) {
                app.open_client_situation(client, true);
            }
            if secondary_button(ui, t!(app.locale, "Situation"), palette) {
                app.open_client_situation(client, false);
            }
            if secondary_button(ui, t!(app.locale, "Invoices"), palette) {
                app.open_client_invoices(client);
            }
        });
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
        money(client.credit_limit, &currency),
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
    let rect = egui::Rect::from_center_size(screen.center(), vec2(520.0, 620.0));
    if widgets::scrim_clicked(ui, screen, rect, egui::Id::new("client-editor-scrim")) {
        app.client_editor = None;
        return;
    }

    let locale = app.locale;
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_min_size(vec2(476.0, 0.0));
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

            if let Some(editor) = app.client_editor.as_mut() {
                labelled_field(ui, t!(locale, "Name"), &mut editor.name, palette, false);
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
                ui.add_space(8.0);
                labelled_field(ui, t!(locale, "Phone"), &mut editor.phone, palette, false);
                ui.add_space(8.0);
                labelled_field(ui, t!(locale, "Email"), &mut editor.email, palette, false);
                ui.add_space(8.0);
                labelled_field(
                    ui,
                    t!(locale, "Address"),
                    &mut editor.address,
                    palette,
                    false,
                );
                ui.add_space(8.0);
                labelled_field(ui, t!(locale, "City"), &mut editor.city, palette, false);
                ui.add_space(8.0);
                labelled_field(
                    ui,
                    t!(locale, "Credit limit (MAD)"),
                    &mut editor.credit_limit,
                    palette,
                    false,
                );
                ui.add_space(10.0);
                ui.checkbox(&mut editor.tax_exempt, t!(locale, "VAT exempt").to_string());
            }

            if let Some(error) = app
                .client_editor
                .as_ref()
                .and_then(|editor| editor.error.clone())
            {
                ui.add_space(8.0);
                error_banner(ui, &error, palette);
            }

            ui.add_space(12.0);
            widgets::dialog_actions(ui, |ui| {
                let saving = app
                    .client_editor
                    .as_ref()
                    .is_some_and(|editor| editor.saving);
                let save = primary_button(ui, t!(locale, "Save"), palette, !saving);
                let cancel = secondary_button(ui, t!(locale, "Cancel"), palette);
                if cancel {
                    app.client_editor = None;
                }
                if save {
                    submit_editor(app);
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
