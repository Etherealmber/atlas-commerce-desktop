//! Produits: searchable catalogue, product creation and stock movements.
//! The worker contract is read-only here apart from `POST /api/products`
//! and `POST /api/products/:id/adjust-stock`.

use atlas_api::ApiError;
use atlas_api::models::{Product, StockAdjust};
use atlas_core::format::number;
use atlas_core::jobs::Job;
use egui::{RichText, vec2};

use crate::dialogs::{Confirm, ConfirmAction};
use crate::icons::Icon;
use crate::state::AtlasApp;
use crate::theme::Palette;
use crate::widgets::{
    card, empty_state, error_banner, icon, labelled_field, page_title, pill, primary_button,
    secondary_button,
};
use crate::{t, widgets};

/// The create-product dialog state.
#[derive(Default)]
pub struct ProductEditor {
    pub name: String,
    pub sku: String,
    pub barcode: String,
    pub category: String,
    pub unit: String,
    pub price: String,
    pub stock: String,
    pub threshold: String,
    pub description: String,
    pub saving: bool,
    /// A 409 `DUPLICATE_PRODUCT` was answered; the next save forces it.
    pub force_duplicate: bool,
    pub error: Option<String>,
    pub job: Option<Job<Result<Product, ApiError>>>,
}

impl ProductEditor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            unit: "unité".to_owned(),
            ..Self::default()
        }
    }
}

/// The stock-adjustment dialog for one product.
#[allow(clippy::type_complexity)]
pub struct StockDialog {
    pub product_id: i64,
    pub product_name: String,
    pub quantity: String,
    pub kind: StockKind,
    pub reason: String,
    pub saving: bool,
    pub error: Option<String>,
    pub job: Option<Job<Result<Product, ApiError>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StockKind {
    In,
    Out,
    Adjustment,
}

impl StockKind {
    fn worker_value(self) -> &'static str {
        match self {
            Self::In => "in",
            Self::Out => "out",
            Self::Adjustment => "adjustment",
        }
    }

    fn label(self, locale: crate::i18n::Locale) -> String {
        match self {
            Self::In => t!(locale, "Stock in").into_owned(),
            Self::Out => t!(locale, "Stock out").into_owned(),
            Self::Adjustment => t!(locale, "Adjustment").into_owned(),
        }
    }
}

impl StockDialog {
    #[must_use]
    pub fn new(product: &Product) -> Self {
        Self {
            product_id: product.id,
            product_name: product.name.clone(),
            quantity: String::new(),
            kind: StockKind::In,
            reason: String::new(),
            saving: bool::default(),
            error: None,
            job: None,
        }
    }
}

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        page_title(ui, t!(app.locale, "Products"), palette);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if primary_button(ui, t!(app.locale, "New product"), palette, true) {
                app.product_editor = Some(ProductEditor::new());
            }
            if secondary_button(ui, t!(app.locale, "Refresh"), palette) {
                app.load_products();
            }
        });
    });
    ui.add_space(12.0);

    super::scan::draw_scan_bar(app, ui, palette);
    ui.add_space(10.0);

    draw_search(app, ui, palette);
    ui.add_space(10.0);

    if let Some(error) = app.products.error.clone() {
        error_banner(ui, &error, palette);
        ui.add_space(10.0);
    }

    if app.products.loading && app.products.data.is_none() {
        super::centered_spinner(ui, palette);
    } else if app
        .products
        .data
        .as_ref()
        .is_none_or(|items| items.is_empty())
    {
        empty_state(
            ui,
            Icon::Package,
            t!(app.locale, "No product found"),
            t!(app.locale, "Create your first product to get started."),
            palette,
        );
    } else {
        product_list(app, ui, palette);
    }

    if app.product_editor.is_some() {
        draw_editor(app, ui, palette);
    }
    if app.stock_dialog.is_some() {
        draw_stock_dialog(app, ui, palette);
    }
}

fn draw_search(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            icon(ui, Icon::Search, palette.secondary, 18.0);
            let response = ui.add(
                egui::TextEdit::singleline(&mut app.product_search)
                    .hint_text(t!(app.locale, "Search by name, SKU or barcode"))
                    .margin(egui::Margin::symmetric(10, 8))
                    .desired_width(320.0),
            );
            if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                app.load_products();
            }
            if secondary_button(ui, t!(app.locale, "Search"), palette) {
                app.load_products();
            }
        });
    });
}

fn product_list(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let products = app.products.data.clone().unwrap_or_default();
    for product in products {
        card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                icon(
                    ui,
                    Icon::Package,
                    if product.is_low_stock() {
                        palette.warning
                    } else {
                        palette.accent
                    },
                    20.0,
                );
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(product.name.clone())
                            .color(palette.text)
                            .strong()
                            .size(14.0),
                    );
                    let subtitle = product.subtitle();
                    if !subtitle.is_empty() {
                        ui.label(RichText::new(subtitle).color(palette.dim).size(12.0));
                    }
                });
                // No-wrap scopes: the name/subtitle column is the only
                // flexible element and may wrap; the price/pill/button
                // columns keep their natural width so the button label
                // never collapses to one character per line.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    widgets::no_wrap_scope(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(atlas_core::format::money(
                                    product.price,
                                    app.dashboard
                                        .data
                                        .as_ref()
                                        .map(|d| d.currency())
                                        .unwrap_or("MAD"),
                                ))
                                .color(palette.text)
                                .strong()
                                .size(13.5),
                            );
                            stock_pill(ui, app.locale, palette, &product);
                        });
                        ui.vertical(|ui| {
                            ui.add_space(2.0);
                            if secondary_button(ui, t!(app.locale, "Adjust stock"), palette) {
                                app.stock_dialog = Some(StockDialog::new(&product));
                            }
                        });
                    });
                });
            });
        });
        ui.add_space(8.0);
    }
}

fn stock_pill(
    ui: &mut egui::Ui,
    locale: crate::i18n::Locale,
    palette: &Palette,
    product: &Product,
) {
    if !product.tracks_inventory() {
        pill(
            ui,
            t!(locale, "Not tracked"),
            palette.surface_active,
            palette.secondary,
        );
        return;
    }
    let text = format!(
        "{} {}",
        number(product.stock),
        product.unit.clone().unwrap_or_else(|| "unité".to_owned())
    );
    let (fill, tint) = if product.is_out_of_stock() {
        (palette.danger.gamma_multiply(0.18), palette.danger)
    } else if product.is_low_stock() {
        (palette.warning.gamma_multiply(0.18), palette.warning)
    } else {
        (palette.accent.gamma_multiply(0.16), palette.accent)
    };
    pill(ui, text, fill, tint);
}

// ── Create product dialog ──────────────────────────────────────────────────

fn draw_editor(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    // Poll the save job first, so a result applies this frame.
    if let Some(editor) = app.product_editor.as_mut()
        && let Some(job) = &editor.job
        && let Some(result) = job.poll()
    {
        editor.job = None;
        editor.saving = false;
        match result {
            Ok(_) => {
                app.product_editor = None;
                app.notice(t!(app.locale, "Product created.").into_owned());
                app.load_products();
                return;
            }
            Err(error) => {
                if error.code.as_deref() == Some("DUPLICATE_PRODUCT") {
                    editor.force_duplicate = true;
                    editor.error = Some(
                        t!(
                            app.locale,
                            "A similar product already exists. Save again to create it anyway."
                        )
                        .into_owned(),
                    );
                } else {
                    editor.error = Some(error.message);
                }
            }
        }
    }
    if app.product_editor.is_none() {
        return;
    }

    let screen = ui.ctx().input(|i| i.viewport_rect());
    let rect = egui::Rect::from_center_size(screen.center(), vec2(520.0, 560.0));
    ui.painter()
        .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(140));

    let locale = app.locale;
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_min_size(vec2(476.0, 0.0));
            ui.label(
                RichText::new(t!(locale, "New product"))
                    .color(palette.text)
                    .strong()
                    .size(17.0),
            );
            ui.add_space(10.0);

            let dirty = app
                .product_editor
                .as_ref()
                .is_some_and(ProductEditor::is_dirty);
            let saving = app
                .product_editor
                .as_ref()
                .is_some_and(|editor| editor.saving);
            let name_empty = app
                .product_editor
                .as_ref()
                .is_some_and(|editor| editor.name.trim().is_empty());
            let error = app
                .product_editor
                .as_ref()
                .and_then(|editor| editor.error.clone());

            if let Some(editor) = app.product_editor.as_mut() {
                labelled_field(ui, t!(locale, "Name"), &mut editor.name, palette, false);
                labelled_field(ui, t!(locale, "SKU"), &mut editor.sku, palette, false);
                labelled_field(
                    ui,
                    t!(locale, "Barcode"),
                    &mut editor.barcode,
                    palette,
                    false,
                );
                labelled_field(
                    ui,
                    t!(locale, "Category"),
                    &mut editor.category,
                    palette,
                    false,
                );
                labelled_field(ui, t!(locale, "Unit"), &mut editor.unit, palette, false);
                labelled_field(
                    ui,
                    t!(locale, "Price (MAD)"),
                    &mut editor.price,
                    palette,
                    false,
                );
                labelled_field(
                    ui,
                    t!(locale, "Initial stock"),
                    &mut editor.stock,
                    palette,
                    false,
                );
                labelled_field(
                    ui,
                    t!(locale, "Low stock threshold"),
                    &mut editor.threshold,
                    palette,
                    false,
                );
            }

            if let Some(error) = &error {
                error_banner(ui, error, palette);
            }

            ui.add_space(10.0);
            widgets::dialog_actions(ui, |ui| {
                let save = primary_button(ui, t!(locale, "Save"), palette, !saving && !name_empty);
                let cancel = secondary_button(ui, t!(locale, "Cancel"), palette);
                if cancel {
                    if dirty {
                        app.confirm_dialog = Some(Confirm {
                            title: t!(locale, "Discard changes?").into_owned(),
                            body: t!(locale, "The product information has not been saved.")
                                .into_owned(),
                            confirm_label: t!(locale, "Discard").into_owned(),
                            danger: true,
                            action: ConfirmAction::DiscardEditor,
                        });
                    } else {
                        app.product_editor = None;
                    }
                }
                if save {
                    submit_product(app);
                }
            });
        });
    });
}

impl ProductEditor {
    pub(crate) fn is_dirty(&self) -> bool {
        !self.name.is_empty()
            || !self.sku.is_empty()
            || !self.barcode.is_empty()
            || !self.category.is_empty()
            // The form seeds Unit with its default "unité"; only a deviation
            // (not any non-empty value) counts as an unsaved edit.
            || self.unit.trim() != "unité"
            || !self.price.is_empty()
            || !self.stock.is_empty()
            || !self.threshold.trim().is_empty()
    }

    fn parse(&self) -> Result<atlas_api::models::CreateProduct, String> {
        let parse_amount = |value: &str, field: &str| -> Result<f64, String> {
            value
                .trim()
                .replace(',', ".")
                .parse::<f64>()
                .map_err(|_| format!("{field} : valeur invalide."))
        };
        Ok(atlas_api::models::CreateProduct {
            name: self.name.trim().to_owned(),
            sku: self.sku.trim().to_owned(),
            barcode: optional(&self.barcode),
            category: optional(&self.category),
            unit: if self.unit.trim().is_empty() {
                "unité".to_owned()
            } else {
                self.unit.trim().to_owned()
            },
            price: parse_amount(&self.price, "Prix")?,
            stock_quantity: parse_amount(&self.stock, "Stock initial")?,
            low_stock_threshold: parse_amount(&self.threshold, "Seuil de stock bas").unwrap_or(0.0),
            description: optional(&self.description),
        })
    }
}

fn optional(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn submit_product(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let Some(editor) = app.product_editor.as_mut() else {
        return;
    };
    let product = match editor.parse() {
        Ok(product) => product,
        Err(message) => {
            editor.error = Some(message);
            return;
        }
    };
    let force = editor.force_duplicate;
    let job = app
        .runner
        .spawn(async move { client.create_product(&product, force).await });
    editor.saving = true;
    editor.error = None;
    editor.job = Some(job);
}

// ── Stock adjustment dialog ─────────────────────────────────────────────────

fn draw_stock_dialog(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    if let Some(dialog) = app.stock_dialog.as_mut()
        && let Some(job) = &dialog.job
        && let Some(result) = job.poll()
    {
        dialog.job = None;
        dialog.saving = false;
        match result {
            Ok(_) => {
                app.stock_dialog = None;
                app.notice(t!(app.locale, "Stock updated.").into_owned());
                app.load_products();
                return;
            }
            Err(error) => dialog.error = Some(error.message),
        }
    }
    if app.stock_dialog.is_none() {
        return;
    }

    let screen = ui.ctx().input(|i| i.viewport_rect());
    let rect = egui::Rect::from_center_size(screen.center(), vec2(460.0, 360.0));
    ui.painter()
        .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(140));

    let locale = app.locale;
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_min_size(vec2(416.0, 0.0));
            ui.label(
                RichText::new(t!(locale, "Adjust stock"))
                    .color(palette.text)
                    .strong()
                    .size(17.0),
            );
            ui.add_space(6.0);
            let product_name = app
                .stock_dialog
                .as_ref()
                .map(|dialog| dialog.product_name.clone())
                .unwrap_or_default();
            ui.label(
                RichText::new(product_name)
                    .color(palette.secondary)
                    .size(13.0),
            );
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                for kind in [StockKind::In, StockKind::Out, StockKind::Adjustment] {
                    let selected = app
                        .stock_dialog
                        .as_ref()
                        .is_some_and(|dialog| dialog.kind == kind);
                    let button = egui::Button::new(
                        RichText::new(kind.label(locale))
                            .color(if selected {
                                palette.on_accent
                            } else {
                                palette.text
                            })
                            .size(13.0),
                    )
                    .wrap_mode(egui::TextWrapMode::Extend)
                    .fill(if selected {
                        palette.accent
                    } else {
                        palette.surface
                    })
                    .corner_radius(8.0);
                    if ui.add(button).clicked()
                        && let Some(dialog) = app.stock_dialog.as_mut()
                    {
                        dialog.kind = kind;
                    }
                }
            });

            ui.add_space(8.0);
            if let Some(dialog) = app.stock_dialog.as_mut() {
                labelled_field(
                    ui,
                    t!(locale, "Quantity"),
                    &mut dialog.quantity,
                    palette,
                    false,
                );
                labelled_field(
                    ui,
                    t!(locale, "Reason (optional)"),
                    &mut dialog.reason,
                    palette,
                    false,
                );
            }

            let error = app
                .stock_dialog
                .as_ref()
                .and_then(|dialog| dialog.error.clone());
            if let Some(error) = &error {
                error_banner(ui, error, palette);
            }

            let saving = app
                .stock_dialog
                .as_ref()
                .is_some_and(|dialog| dialog.saving);
            let quantity_empty = app
                .stock_dialog
                .as_ref()
                .is_some_and(|dialog| dialog.quantity.trim().is_empty());

            ui.add_space(10.0);
            widgets::dialog_actions(ui, |ui| {
                let confirm =
                    primary_button(ui, t!(locale, "Apply"), palette, !saving && !quantity_empty);
                let cancel = secondary_button(ui, t!(locale, "Cancel"), palette);
                if cancel {
                    app.stock_dialog = None;
                }
                if confirm {
                    submit_adjust(app);
                }
            });
        });
    });
}

fn submit_adjust(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let Some(dialog) = app.stock_dialog.as_mut() else {
        return;
    };
    let Ok(quantity) = dialog.quantity.trim().replace(',', ".").parse::<f64>() else {
        dialog.error = Some("Quantité invalide.".to_owned());
        return;
    };
    let body = StockAdjust {
        quantity: quantity.abs(),
        kind: dialog.kind.worker_value().to_owned(),
        reason: optional(&dialog.reason),
    };
    let id = dialog.product_id;
    let job = app
        .runner
        .spawn(async move { client.adjust_stock(id, &body).await });
    dialog.saving = true;
    dialog.error = None;
    dialog.job = Some(job);
}
