//! Barcode scan bar embedded in the Products screen. Wire contract: the
//! `/api/scan` block of the worker — a hot-path barcode/SKU lookup and the
//! "quick product" creation flow used by the mobile apps, including the
//! 409 `DUPLICATE_PRODUCT` force-retry.

use atlas_api::ApiError;
use atlas_api::models::{Product, QuickProductWrite, ScannedProduct};
use atlas_core::format::money;
use atlas_core::jobs::Job;
use egui::{RichText, vec2};

use crate::icons::Icon;
use crate::state::AtlasApp;
use crate::theme::Palette;
use crate::widgets::{
    card, error_banner, icon, info_banner, labelled_field, primary_button, secondary_button,
};
use crate::{t, widgets};

/// All state behind the scan bar: the code field, the in-flight jobs, the
/// last match and the optional quick-product dialog.
#[derive(Default)]
pub struct ScanState {
    pub code: String,
    pub result: Option<ScannedProduct>,
    pub lookup_job: Option<Job<Result<ScannedProduct, ApiError>>>,
    pub create_job: Option<Job<Result<Product, ApiError>>>,
    pub error: Option<String>,
    pub quick: Option<QuickProductDraft>,
    /// A 409 `DUPLICATE_PRODUCT` was answered; the next save forces it.
    pub duplicate: bool,
}

/// The "create from barcode" dialog.
#[derive(Default, Clone)]
pub struct QuickProductDraft {
    pub barcode: String,
    pub name: String,
    pub price: String,
    pub cost: String,
    pub stock: String,
    pub category: String,
    pub unit: String,
}

impl QuickProductDraft {
    #[must_use]
    fn new(barcode: &str) -> Self {
        Self {
            barcode: barcode.to_owned(),
            unit: "unité".to_owned(),
            ..Self::default()
        }
    }

    fn to_write(&self, force: bool) -> QuickProductWrite {
        QuickProductWrite {
            barcode: self.barcode.trim().to_owned(),
            name: self.name.trim().to_owned(),
            price: parse_amount(&self.price),
            cost: parse_amount(&self.cost),
            category: if self.category.trim().is_empty() {
                "Général".to_owned()
            } else {
                self.category.trim().to_owned()
            },
            unit: if self.unit.trim().is_empty() {
                "unité".to_owned()
            } else {
                self.unit.trim().to_owned()
            },
            stock_quantity: parse_amount(&self.stock),
            // An empty SKU makes the worker fall back to the barcode.
            sku: self.barcode.trim().to_owned(),
            force_duplicate: force,
            low_stock_threshold: 5.0,
        }
    }
}

fn parse_amount(value: &str) -> f64 {
    value
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .unwrap_or(0.0)
}

/// Draws the scan bar and, when present, the match card and dialog.
pub fn draw_scan_bar(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    poll_jobs(app);

    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            icon(ui, Icon::QrCode, palette.secondary, 18.0);
            let code_response = ui.add(
                egui::TextEdit::singleline(&mut app.scan.code)
                    .hint_text(t!(
                        app.locale,
                        "Scan or type a barcode / SKU, then press Enter"
                    ))
                    .margin(egui::Margin::symmetric(10, 8))
                    .desired_width((ui.available_width() - 220.0).max(160.0)),
            );
            if code_response.lost_focus()
                && ui.input(|input| input.key_pressed(egui::Key::Enter))
            {
                start_lookup(app);
            }
            let busy = app.scan.lookup_job.is_some();
            if primary_button(ui, t!(app.locale, "Search"), palette, !busy) {
                start_lookup(app);
            }
            if secondary_button(ui, t!(app.locale, "Create from barcode"), palette) {
                let code = app.scan.code.trim().to_owned();
                if !code.is_empty() {
                    app.scan.duplicate = false;
                    app.scan.quick = Some(QuickProductDraft::new(&code));
                }
            }
        });
    });

    if let Some(error) = app.scan.error.clone() {
        ui.add_space(8.0);
        error_banner(ui, &error, palette);
    }

    if app.scan.result.is_some() {
        ui.add_space(10.0);
        draw_result(app, ui, palette);
    }

    if app.scan.quick.is_some() {
        draw_quick_dialog(app, ui, palette);
    }
}

fn start_lookup(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let code = app.scan.code.trim().to_owned();
    if code.is_empty() {
        return;
    }
    app.scan.error = None;
    app.scan.result = None;
    let job = app
        .runner
        .spawn(async move { client.scan_barcode(&code).await });
    app.scan.lookup_job = Some(job);
}

fn draw_result(app: &AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let Some(product) = &app.scan.result else {
        return;
    };
    let currency = app
        .dashboard
        .data
        .as_ref()
        .map(|data| data.currency())
        .unwrap_or("MAD");
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            icon(ui, Icon::CircleCheck, palette.accent, 20.0);
            ui.vertical(|ui| {
                let title = product
                    .display_name
                    .clone()
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| product.name.clone());
                ui.label(
                    RichText::new(title)
                        .color(palette.text)
                        .strong()
                        .size(14.5),
                );
                let subline = [
                    product.sku.clone().unwrap_or_default(),
                    product.barcode.clone().unwrap_or_default(),
                    product.category.clone().unwrap_or_default(),
                    product.variant_label.clone().unwrap_or_default(),
                ]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
                ui.label(RichText::new(subline).color(palette.dim).size(12.0));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.vertical(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(money(product.price, currency))
                                .color(palette.text)
                                .strong()
                                .size(14.0),
                        );
                    });
                    ui.label(
                        RichText::new(format!(
                            "{} : {}",
                            t!(app.locale, "Stock"),
                            atlas_core::format::number(product.stock_quantity)
                        ))
                        .color(palette.secondary)
                        .size(12.0),
                    );
                });
            });
        });
    });
}

fn draw_quick_dialog(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let screen = ui.ctx().input(|input| input.viewport_rect());
    let rect = egui::Rect::from_center_size(screen.center(), vec2(480.0, 480.0));
    ui.painter()
        .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(140));

    let locale = app.locale;
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_min_size(vec2(436.0, 0.0));
            ui.label(
                RichText::new(t!(locale, "Create from barcode"))
                    .color(palette.text)
                    .strong()
                    .size(17.0),
            );
            ui.add_space(10.0);

            if let Some(draft) = app.scan.quick.as_mut() {
                labelled_field(ui, t!(locale, "Barcode"), &mut draft.barcode, palette, false);
                ui.add_space(8.0);
                labelled_field(ui, t!(locale, "Name"), &mut draft.name, palette, false);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        labelled_field(ui, t!(locale, "Sale price"), &mut draft.price, palette, false);
                    });
                    ui.vertical(|ui| {
                        labelled_field(ui, t!(locale, "Cost"), &mut draft.cost, palette, false);
                    });
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        labelled_field(ui, t!(locale, "Initial stock"), &mut draft.stock, palette, false);
                    });
                    ui.vertical(|ui| {
                        labelled_field(ui, t!(locale, "Category"), &mut draft.category, palette, false);
                    });
                });
                ui.add_space(8.0);
                labelled_field(ui, t!(locale, "Unit"), &mut draft.unit, palette, false);
            }

            if app.scan.duplicate {
                ui.add_space(8.0);
                info_banner(
                    ui,
                    t!(
                        locale,
                        "This barcode already exists. Save again to create the product anyway."
                    ),
                    palette,
                );
            }

            if let Some(error) = app.scan.error.clone() {
                ui.add_space(8.0);
                error_banner(ui, &error, palette);
            }

            ui.add_space(12.0);
            widgets::dialog_actions(ui, |ui| {
                let saving = app.scan.create_job.is_some();
                let label = if app.scan.duplicate {
                    t!(locale, "Create anyway")
                } else {
                    t!(locale, "Save")
                };
                let save = primary_button(ui, label, palette, !saving);
                let cancel = secondary_button(ui, t!(locale, "Cancel"), palette);
                if cancel {
                    app.scan.quick = None;
                    app.scan.duplicate = false;
                    app.scan.error = None;
                }
                if save {
                    submit_quick(app);
                }
            });
        });
    });
}

fn submit_quick(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let Some(draft) = app.scan.quick.clone() else {
        return;
    };
    if draft.barcode.trim().is_empty() {
        app.scan.error = Some(
            t!(app.locale, "The barcode is required.")
                .into_owned(),
        );
        return;
    }
    let force = app.scan.duplicate;
    let body = draft.to_write(force);
    app.scan.error = None;
    let job = app
        .runner
        .spawn(async move { client.quick_product(&body).await });
    app.scan.create_job = Some(job);
}

fn poll_jobs(app: &mut AtlasApp) {
    if let Some(job) = &app.scan.lookup_job
        && let Some(result) = job.poll()
    {
        app.scan.lookup_job = None;
        match result {
            Ok(product) => {
                app.scan.result = Some(product);
                app.scan.error = None;
            }
            Err(error) => {
                app.scan.result = None;
                app.scan.error = Some(error.message);
            }
        }
    }
    if let Some(job) = &app.scan.create_job
        && let Some(result) = job.poll()
    {
        app.scan.create_job = None;
        match result {
            Ok(product) => {
                app.scan.quick = None;
                app.scan.duplicate = false;
                app.scan.error = None;
                app.notice(t!(app.locale, "Product created.").into_owned());
                app.scan.code = product
                    .barcode
                    .clone()
                    .unwrap_or_else(|| product.sku.clone().unwrap_or_default());
                app.load_products();
                start_lookup(app);
            }
            Err(error) if error.code.as_deref() == Some("DUPLICATE_PRODUCT") => {
                app.scan.duplicate = true;
                app.scan.error = Some(error.message);
            }
            Err(error) => app.scan.error = Some(error.message),
        }
    }
}
