//! Ventes: the invoice history and the quick-sale (vente rapide) writer.
//! Quick sales follow the worker contract: `quick_sale: true`,
//! `doc_kind: "quick_sale"`, no VAT, and one idempotency key per draft,
//! reused across retries so a timed-out request cannot create a duplicate.

use atlas_api::ApiError;
use atlas_api::models::{CreatedInvoice, QuickSale, QuickSaleItem};
use atlas_core::format::{money, short_date};
use atlas_core::jobs::Job;
use egui::RichText;

use crate::icons::Icon;
use crate::state::AtlasApp;
use crate::theme::Palette;
use crate::widgets::{
    card, empty_state, error_banner, icon, labelled_field, page_title, primary_button,
    secondary_button,
};
use crate::{t, widgets};

/// One line of a quick sale being written.
pub struct QuickSaleLine {
    pub product_id: i64,
    pub name: String,
    pub quantity: String,
    pub unit_price: f64,
}

/// The quick-sale form state.
#[derive(Default)]
pub struct QuickSaleDraft {
    pub client_name: String,
    pub lines: Vec<QuickSaleLine>,
    pub selected_product: usize,
    pub saving: bool,
    pub error: Option<String>,
    pub job: Option<Job<Result<CreatedInvoice, ApiError>>>,
    /// Stable for the life of the draft: retries reuse it so the worker's
    /// idempotency dedup catches a request that timed out after committing.
    /// Cleared on success, when the whole draft is replaced.
    pub idempotency_key: String,
}

impl QuickSaleDraft {
    fn total(&self) -> f64 {
        self.lines
            .iter()
            .map(|line| line.quantity.parse::<f64>().unwrap_or(0.0) * line.unit_price)
            .sum()
    }

    fn valid(&self) -> bool {
        !self.client_name.trim().is_empty()
            && self
                .lines
                .iter()
                .any(|line| line.quantity.parse::<f64>().is_ok_and(|qty| qty > 0.0))
    }
}

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    poll_sale_job(app);

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        page_title(ui, t!(app.locale, "Sales"), palette);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if secondary_button(ui, t!(app.locale, "Refresh"), palette) {
                app.load_invoices();
            }
        });
    });
    ui.add_space(12.0);

    ui.horizontal_top(|ui| {
        card(palette).show(ui, |ui| {
            ui.set_width((ui.available_width() * 0.55).max(300.0));
            invoice_history(app, ui, palette);
        });
        card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            quick_sale_form(app, ui, palette);
        });
    });
}

fn poll_sale_job(app: &mut AtlasApp) {
    let Some(draft) = app.quick_sale.as_mut() else {
        return;
    };
    if let Some(job) = &draft.job
        && let Some(result) = job.poll()
    {
        draft.job = None;
        draft.saving = false;
        match result {
            Ok(invoice) => {
                let number = invoice.number.unwrap_or_else(|| format!("#{}", invoice.id));
                app.notice(format!(
                    "{} ({number})",
                    t!(app.locale, "Quick sale recorded.")
                ));
                app.quick_sale = Some(QuickSaleDraft::default());
                app.load_invoices();
                app.load_dashboard();
            }
            Err(error) => draft.error = Some(error.message),
        }
    }
}

fn invoice_history(app: &AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.label(
        RichText::new(t!(app.locale, "Invoice history"))
            .color(palette.text)
            .strong()
            .size(15.0),
    );
    ui.add_space(8.0);
    if app.invoices.loading && app.invoices.data.is_none() {
        super::spinner(ui, palette);
        return;
    }
    if let Some(error) = &app.invoices.error {
        error_banner(ui, error, palette);
        return;
    }
    let invoices = app.invoices.data.clone().unwrap_or_default();
    if invoices.is_empty() {
        empty_state(
            ui,
            Icon::Receipt,
            t!(app.locale, "No sale yet"),
            t!(app.locale, "Recorded sales appear here."),
            palette,
        );
        return;
    }
    let currency = app
        .dashboard
        .data
        .as_ref()
        .map(|data| data.currency())
        .unwrap_or("MAD");
    for invoice in invoices {
        ui.horizontal(|ui| {
            // No full-width pin inside a horizontal row: the amount and
            // status pill on the right would be starved and overlap.
            icon(ui, Icon::Receipt, palette.secondary, 16.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(
                        invoice
                            .number
                            .clone()
                            .unwrap_or_else(|| format!("#{}", invoice.id)),
                    )
                    .color(palette.text)
                    .size(13.5),
                );
                let client = invoice
                    .client_name
                    .clone()
                    .unwrap_or_else(|| "—".to_owned());
                let date = invoice
                    .issue_date
                    .as_deref()
                    .map(short_date)
                    .unwrap_or_default();
                ui.label(
                    RichText::new(format!("{client} · {date}"))
                        .color(palette.dim)
                        .size(11.5),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(money(invoice.total, currency))
                        .color(palette.text)
                        .strong()
                        .size(13.0),
                );
                if let Some(status) = &invoice.status {
                    super::dashboard::status_pill(ui, palette, status);
                }
            });
        });
        ui.separator();
    }
}

fn quick_sale_form(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    if app.quick_sale.is_none() {
        app.quick_sale = Some(QuickSaleDraft::default());
    }
    ui.label(
        RichText::new(t!(app.locale, "Quick sale"))
            .color(palette.text)
            .strong()
            .size(15.0),
    );
    ui.add_space(8.0);

    let products = app.products.data.clone().unwrap_or_default();
    if products.is_empty() {
        empty_state(
            ui,
            Icon::Package,
            t!(app.locale, "No product available"),
            t!(app.locale, "Create products before recording a sale."),
            palette,
        );
        return;
    }

    if let Some(draft) = app.quick_sale.as_mut() {
        labelled_field(
            ui,
            t!(app.locale, "Client name"),
            &mut draft.client_name,
            palette,
            false,
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            let names: Vec<String> = products
                .iter()
                .map(|product| {
                    if product.sku.as_deref().is_some_and(|sku| !sku.is_empty()) {
                        format!(
                            "{} ({})",
                            product.name,
                            product.sku.as_deref().unwrap_or_default()
                        )
                    } else {
                        product.name.clone()
                    }
                })
                .collect();
            draft.selected_product = draft.selected_product.min(products.len().saturating_sub(1));
            egui::ComboBox::from_id_salt("quick-sale-product")
                .selected_text(names[draft.selected_product].clone())
                .show_ui(ui, |ui| {
                    for (index, name) in names.iter().enumerate() {
                        ui.selectable_value(&mut draft.selected_product, index, name);
                    }
                });
            if primary_button(ui, t!(app.locale, "Add"), palette, true) {
                let product = &products[draft.selected_product];
                if !draft.lines.iter().any(|line| line.product_id == product.id) {
                    draft.lines.push(QuickSaleLine {
                        product_id: product.id,
                        name: product.name.clone(),
                        quantity: "1".to_owned(),
                        unit_price: product.price,
                    });
                }
            }
        });

        ui.add_space(10.0);
        if draft.lines.is_empty() {
            widgets::info_banner(
                ui,
                t!(app.locale, "Add at least one product to the sale."),
                palette,
            );
        } else {
            let mut remove_index = None;
            for (index, line) in draft.lines.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    icon(ui, Icon::Package, palette.secondary, 15.0);
                    ui.label(
                        RichText::new(line.name.clone())
                            .color(palette.text)
                            .size(13.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut line.quantity)
                            .desired_width(70.0)
                            .margin(egui::Margin::symmetric(8, 6)),
                    );
                    let currency = app
                        .dashboard
                        .data
                        .as_ref()
                        .map(|data| data.currency())
                        .unwrap_or("MAD");
                    ui.label(
                        RichText::new(money(
                            line.quantity.parse::<f64>().unwrap_or(0.0) * line.unit_price,
                            currency,
                        ))
                        .color(palette.secondary)
                        .size(12.5),
                    );
                    if ui
                        .add(
                            egui::Button::new(RichText::new("×").color(palette.danger).strong())
                                .fill(palette.surface)
                                .corner_radius(8.0),
                        )
                        .clicked()
                    {
                        remove_index = Some(index);
                    }
                });
            }
            if let Some(index) = remove_index {
                app.quick_sale.as_mut().unwrap().lines.remove(index);
            }
        }

        if let Some(error) = app
            .quick_sale
            .as_ref()
            .and_then(|draft| draft.error.clone())
        {
            ui.add_space(8.0);
            error_banner(ui, &error, palette);
        }

        ui.add_space(10.0);
        let total = app.quick_sale.as_ref().map_or(0.0, QuickSaleDraft::total);
        let currency = app
            .dashboard
            .data
            .as_ref()
            .map(|data| data.currency())
            .unwrap_or("MAD");
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!(app.locale, "Total"))
                    .color(palette.secondary)
                    .strong(),
            );
            ui.label(
                RichText::new(money(total, currency))
                    .color(palette.text)
                    .strong()
                    .size(16.0),
            );
        });
        ui.add_space(8.0);

        let valid = app.quick_sale.as_ref().is_some_and(QuickSaleDraft::valid);
        let saving = app.quick_sale.as_ref().is_some_and(|draft| draft.saving);
        if primary_button(ui, t!(app.locale, "Record sale"), palette, valid && !saving) {
            submit_quick_sale(app);
        }
    }
}

fn submit_quick_sale(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let Some(draft) = app.quick_sale.as_mut() else {
        return;
    };
    let items: Vec<QuickSaleItem> = draft
        .lines
        .iter()
        .filter_map(|line| {
            let quantity = line.quantity.parse::<f64>().ok()?;
            (quantity > 0.0).then_some(QuickSaleItem {
                product_id: line.product_id,
                product_name: line.name.clone(),
                quantity,
                unit_price: line.unit_price,
                tax_rate: 0.0,
            })
        })
        .collect();
    if items.is_empty() {
        draft.error = Some("Le nom du client et au moins une ligne sont obligatoires".to_owned());
        return;
    }
    let sale = QuickSale {
        client_name: draft.client_name.trim().to_owned(),
        client_phone: None,
        issue_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
        status: "sent".to_owned(),
        quick_sale: true,
        doc_kind: "quick_sale".to_owned(),
        tax_exempt: true,
        notes: None,
        items,
    };
    // First submit mints the key; retries after a timeout keep reusing it so
    // the worker cannot record the same sale twice.
    if draft.idempotency_key.is_empty() {
        draft.idempotency_key = format!(
            "atlas-desktop-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        );
    }
    let idempotency_key = draft.idempotency_key.clone();
    let job = app
        .runner
        .spawn(async move { client.create_quick_sale(&sale, &idempotency_key).await });
    draft.saving = true;
    draft.error = None;
    draft.job = Some(job);
}
