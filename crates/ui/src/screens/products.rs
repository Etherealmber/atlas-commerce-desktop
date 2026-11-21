//! Native catalogue, product editor, labels, deletion and stock movements.

use atlas_api::ApiError;
use atlas_api::models::{Product, StockAdjust, StockMovement};
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

/// One undoable deletion at a time. No request is sent before the deadline.
pub(crate) struct ProductDelete {
    id: i64,
    deadline: std::time::Instant,
    job: Option<Job<Result<atlas_api::commerce::ProductDeletion, ApiError>>>,
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Drop for ProductDelete {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
}

pub(crate) struct ProductLabel {
    pub(crate) uri: String,
    name: String,
    job: Option<Job<Result<Vec<u8>, ApiError>>>,
    loaded: bool,
    error: Option<String>,
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Drop for ProductLabel {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
}

/// Run only one bounded request, and cancel the countdown on navigation.
pub(crate) fn poll_product_actions(app: &mut AtlasApp, ctx: &egui::Context) {
    if let Some(uri) = app.product_label_evict.take() {
        ctx.forget_image(&uri);
    }
    if let Some(label) = app.product_label.as_mut() {
        if let Some(result) = label.job.as_ref().and_then(Job::poll) {
            label.job = None;
            label.cancel = None;
            match result {
                Ok(bytes) => {
                    ctx.forget_image(&label.uri);
                    ctx.include_bytes(label.uri.clone(), bytes);
                    label.loaded = true;
                }
                Err(error) => label.error = Some(error.message),
            }
            ctx.request_repaint();
        }
        if label.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    let on_products = matches!(
        app.screen,
        crate::state::Screen::Shell(crate::state::Route::Products)
    );
    if !on_products {
        if let Some(label) = app.product_label.take() {
            ctx.forget_image(&label.uri);
        }
        app.stock_report_open = false;
        app.product_scanner_open = false;
        if app.scan.create_job.is_none() {
            app.scan.quick = None;
        }
    }
    if let Some(pending) = app.product_delete.as_mut() {
        if pending.job.is_none() && !on_products {
            app.product_delete = None;
            return;
        }
        if pending.job.is_none() && pending.deadline <= std::time::Instant::now() {
            if let Some(client) = app.client.clone() {
                let id = pending.id;
                let (cancel, cancelled) = tokio::sync::oneshot::channel();
                pending.cancel = Some(cancel);
                pending.job = Some(app.runner.spawn(async move {
                    tokio::select! {
                        result = client.delete_product(id) => result,
                        _ = cancelled => Err(ApiError::client("Suppression annulée.")),
                    }
                }));
            } else {
                app.product_delete = None;
                return;
            }
        }
        let result = pending.job.as_ref().and_then(Job::poll);
        if let Some(result) = result {
            app.product_delete = None;
            match result {
                Ok(result) => {
                    let fallback = if result.archived {
                        t!(app.locale, "Product archived.")
                    } else {
                        t!(app.locale, "Product deleted.")
                    };
                    app.notice(result.message.unwrap_or_else(|| fallback.into_owned()));
                    app.load_products();
                }
                Err(error) => app.notice(error.message),
            }
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

fn open_product_label(app: &mut AtlasApp, product: &Product) {
    let Some(client) = app.client.clone() else {
        return;
    };
    if let Some(old) = app.product_label.take() {
        app.product_label_evict = Some(old.uri.clone());
    }
    let id = product.id;
    let (cancel, cancelled) = tokio::sync::oneshot::channel();
    app.product_label = Some(ProductLabel {
        uri: format!("bytes://atlas-product-label-{id}.svg"),
        name: product.name.clone(),
        job: Some(app.runner.spawn(async move {
            tokio::select! {
                result = client.product_label(id) => result,
                _ = cancelled => Err(ApiError::client("Chargement annulé.")),
            }
        })),
        cancel: Some(cancel),
        loaded: false,
        error: None,
    });
}

fn draw_product_label(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let Some(label) = app.product_label.as_ref() else {
        return;
    };
    let screen = ui.ctx().input(|i| i.viewport_rect());
    let rect = egui::Rect::from_center_size(
        screen.center(),
        vec2(
            440.0_f32.min((screen.width() - 32.0).max(1.0)),
            300.0_f32.min((screen.height() - 32.0).max(1.0)),
        ),
    );
    if widgets::scrim_clicked(ui, screen, rect, egui::Id::new("product-label-scrim")) {
        if let Some(label) = app.product_label.take() {
            app.product_label_evict = Some(label.uri.clone());
        }
        return;
    }
    let name = label.name.clone();
    let uri = label.uri.clone();
    let error = label.error.clone();
    let loaded = label.loaded;
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(format!("{} · {}", t!(app.locale, "Label"), name))
                    .strong()
                    .color(palette.text),
            );
            ui.add_space(12.0);
            if let Some(error) = error {
                error_banner(ui, &error, palette);
            } else if loaded {
                ui.add(
                    egui::Image::new(uri)
                        .fit_to_exact_size(vec2(320.0_f32.min(ui.available_width()), 180.0)),
                );
            } else {
                super::centered_spinner(ui, palette);
            }
            ui.add_space(12.0);
            if secondary_button(ui, t!(app.locale, "Close"), palette)
                && let Some(label) = app.product_label.take()
            {
                app.product_label_evict = Some(label.uri.clone());
            }
        });
    });
}

struct ProductSaveResult {
    failed_photos: usize,
}

/// The create-product dialog state.
#[derive(Default)]
pub struct ProductEditor {
    pub editing_id: Option<i64>,
    pub step: usize,
    pub name: String,
    pub sku: String,
    pub barcode: String,
    pub category: String,
    pub unit: String,
    pub price: String,
    pub stock: String,
    pub threshold: String,
    pub description: String,
    bulk_available: bool,
    bulk_qty: [String; 2],
    bulk_percent: [String; 2],
    bulk_extra: Vec<atlas_api::models::BulkDiscount>,
    variants: crate::state::Cache<Vec<atlas_api::models::ProductVariant>>,
    variant_job: Option<Job<Result<(), ApiError>>>,
    variant_error: Option<String>,
    variant_confirm: Option<i64>,
    variant_label: String,
    variant_sku: String,
    variant_barcode: String,
    variant_price: String,
    images: crate::state::Cache<Vec<atlas_api::models::ProductImage>>,
    image_job: Option<Job<Result<(), ApiError>>>,
    image_error: Option<String>,
    image_confirm: Option<i64>,
    staged_images: Vec<String>,
    stage_job: Option<Job<Result<String, ApiError>>>,
    pub saving: bool,
    /// A 409 `DUPLICATE_PRODUCT` was answered; the next save forces it.
    pub force_duplicate: bool,
    duplicate_identity: Option<(String, String, String)>,
    pub error: Option<String>,
    job: Option<Job<Result<ProductSaveResult, ApiError>>>,
}

impl ProductEditor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            unit: "unité".to_owned(),
            price: "0".to_owned(),
            stock: "0".to_owned(),
            threshold: "5".to_owned(),
            bulk_available: true,
            ..Self::default()
        }
    }

    pub(crate) fn is_busy(&self) -> bool {
        self.saving
            || self.variant_job.is_some()
            || self.image_job.is_some()
            || self.stage_job.is_some()
    }

    #[must_use]
    pub fn for_product(product: &Product) -> Self {
        let tiers = parse_bulk_from_product(product.bulk_discounts.as_ref());
        let mut qty = [String::new(), String::new()];
        let mut percent = [String::new(), String::new()];
        if let Some(tiers) = tiers.as_ref() {
            for (index, tier) in tiers.iter().take(2).enumerate() {
                qty[index] = tier.min_qty.to_string();
                percent[index] = tier.percent.to_string();
            }
        }
        Self {
            bulk_available: tiers.is_some(),
            bulk_qty: qty,
            bulk_percent: percent,
            bulk_extra: tiers.unwrap_or_default().into_iter().skip(2).collect(),
            editing_id: Some(product.id),
            name: product.name.clone(),
            sku: product.sku.clone().unwrap_or_default(),
            barcode: product.barcode.clone().unwrap_or_default(),
            category: product.category.clone().unwrap_or_default(),
            unit: product.unit.clone().unwrap_or_default(),
            price: product.price.to_string(),
            stock: product.stock.to_string(),
            threshold: product.threshold().to_string(),
            description: product.description.clone().unwrap_or_default(),
            ..Self::new()
        }
    }
}

/// An absent, malformed or unexpected response is not rewritten on save.
fn parse_bulk_from_product(
    raw: Option<&serde_json::Value>,
) -> Option<Vec<atlas_api::models::BulkDiscount>> {
    let raw = raw?;
    let value = if let Some(text) = raw.as_str() {
        serde_json::from_str(text).ok()?
    } else {
        raw.clone()
    };
    let mut tiers: Vec<atlas_api::models::BulkDiscount> = serde_json::from_value(value).ok()?;
    if tiers.iter().any(|tier| {
        !tier.min_qty.is_finite()
            || tier.min_qty <= 0.0
            || !tier.percent.is_finite()
            || tier.percent <= 0.0
            || tier.percent > 100.0
    }) {
        return None;
    }
    tiers.sort_by(|a, b| a.min_qty.total_cmp(&b.min_qty));
    Some(tiers)
}

/// The stock-adjustment dialog for one product.
#[allow(clippy::type_complexity)]
pub struct StockDialog {
    pub product_id: i64,
    pub product_name: String,
    pub current_stock: f64,
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
            current_stock: product.stock,
            quantity: "1".to_owned(),
            kind: StockKind::In,
            reason: String::new(),
            saving: bool::default(),
            error: None,
            job: None,
        }
    }
}

/// Only the requested product history is fetched; each page is bounded.
#[allow(clippy::type_complexity)]
pub(crate) struct StockHistory {
    pub product_id: i64,
    pub product_name: String,
    pub full: bool,
    pub offset: u32,
    pub rows: Vec<StockMovement>,
    pub loaded: bool,
    pub error: Option<String>,
    pub job: Option<Job<Result<Vec<StockMovement>, ApiError>>>,
}

fn load_history(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let Some(history) = app.stock_history.as_mut() else {
        return;
    };
    if history.job.is_some() {
        return;
    }
    let (id, full, offset) = (history.product_id, history.full, history.offset);
    history.error = None;
    history.loaded = false;
    history.rows.clear();
    history.job = Some(
        app.runner
            .spawn(async move { client.product_movements(id, full, offset).await }),
    );
}

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    if app
        .stock_history
        .as_ref()
        .is_some_and(|history| history.full)
    {
        draw_history_page(app, ui, palette);
        return;
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        page_title(ui, t!(app.locale, "Products"), palette);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if app.role_policy().can_edit_prices()
                && primary_button(ui, t!(app.locale, "New product"), palette, true)
            {
                app.product_editor = Some(ProductEditor::new());
            }
            if secondary_button(ui, t!(app.locale, "Refresh"), palette) {
                app.load_products();
            }
        });
    });
    ui.add_space(12.0);

    // Keep these tools available on demand, without embedding separate
    // scanner and report blocks above the catalogue.
    ui.horizontal_wrapped(|ui| {
        if secondary_button(ui, t!(app.locale, "Scan barcode"), palette) {
            app.product_scanner_open = true;
        }
        if app.role_policy().can_view_reports()
            && secondary_button(ui, t!(app.locale, "Stock valuation report"), palette)
        {
            app.load_stock_report();
        }
    });
    ui.add_space(10.0);

    draw_search(app, ui, palette);
    ui.add_space(10.0);

    if let Some(error) = app.products.error.clone() {
        error_banner(ui, &error, palette);
        ui.add_space(10.0);
    }

    if app.products.loading && app.products.data.is_none() {
        super::list_skeleton(ui, palette, app.locale, app.settings.reduced_motion);
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
    if app.stock_history.is_some() {
        draw_history_dialog(app, ui, palette);
    }
    if app.product_scanner_open {
        draw_product_tool(app, ui, palette, false);
    }
    if app.stock_report_open {
        draw_product_tool(app, ui, palette, true);
    }
    if app.product_label.is_some() {
        draw_product_label(app, ui, palette);
    }
}

/// Scanner/report are on-demand native overlays, rather than permanent blocks.
fn draw_product_tool(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette, report: bool) {
    let screen = ui.ctx().input(|i| i.viewport_rect());
    let rect = egui::Rect::from_center_size(
        screen.center(),
        vec2(
            760.0_f32.min((screen.width() - 32.0).max(1.0)),
            600.0_f32.min((screen.height() - 32.0).max(1.0)),
        ),
    );
    let id = egui::Id::new(if report {
        "product-report-scrim"
    } else {
        "product-scan-scrim"
    });
    if widgets::scrim_clicked(ui, screen, rect, id) {
        if report {
            app.stock_report_open = false;
        } else {
            app.product_scanner_open = false;
        }
        return;
    }
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if report {
                        t!(app.locale, "Stock valuation report")
                    } else {
                        t!(app.locale, "Scan barcode")
                    })
                    .strong()
                    .color(palette.text)
                    .size(17.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if secondary_button(ui, t!(app.locale, "Close"), palette) {
                        if report {
                            app.stock_report_open = false;
                        } else {
                            app.product_scanner_open = false;
                        }
                    }
                });
            });
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .max_height((rect.height() - 100.0).max(1.0))
                .show(ui, |ui| {
                    if report {
                        draw_stock_report(app, ui, palette);
                    } else {
                        super::scan::draw_scan_bar(app, ui, palette);
                    }
                });
        });
    });
}

/// Values and inventory rows are supplied by the worker, not local caches.
fn draw_stock_report(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(t!(app.locale, "Stock valuation report")).strong().size(16.0).color(palette.text));
        ui.add_space(8.0);
        let locations = app.stock_locations.data.clone().unwrap_or_default();
        let selected = app.stock_report_location
            .and_then(|id| locations.iter().find(|location| location.id == id))
            .map(|location| location.label.clone())
            .unwrap_or_else(|| t!(app.locale, "All locations").into_owned());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t!(app.locale, "Warehouse")).color(palette.secondary));
            let before = app.stock_report_location;
            egui::ComboBox::from_id_salt("stock-report-location")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.stock_report_location, None, t!(app.locale, "All locations"));
                    for location in &locations {
                        ui.selectable_value(&mut app.stock_report_location, Some(location.id), &location.label);
                    }
                });
            if before != app.stock_report_location {
                app.load_stock_report();
            }
            if secondary_button(ui, t!(app.locale, "Refresh"), palette) {
                app.load_stock_report();
            }
            let printable = !app.stock_report.loading && app.stock_report.error.is_none() && app.stock_report.data.is_some() && app.report_job.is_none();
            ui.add_enabled_ui(printable, |ui| {
                if secondary_button(ui, t!(app.locale, "Print / PDF"), palette)
                    && let Ok(location) = crate::client_report::stock_location_param(app.stock_report_location)
                {
                    app.open_printable("/api/reports/stock", &[("location_id", location)]);
                }
            });
            if secondary_button(ui, t!(app.locale, "Close"), palette) {
                app.stock_report_open = false;
                app.stock_report = Default::default();
                app.stock_locations = Default::default();
                app.stock_report_location = None;
            }
        });
        if app.stock_locations.loading {
            ui.label(RichText::new(t!(app.locale, "Loading warehouses…")).color(palette.secondary).size(12.0));
        } else if let Some(error) = app.stock_locations.error.clone() {
            error_banner(ui, &error, palette);
            if secondary_button(ui, t!(app.locale, "Retry warehouses"), palette) {
                app.load_stock_locations();
            }
        }
        ui.add_space(8.0);
        ui.label(RichText::new(t!(app.locale, "Values are provided by the server. Print / PDF opens the official report in your browser.")).color(palette.secondary).size(12.0));
        if app.stock_report.loading {
            super::centered_spinner(ui, palette);
        } else if let Some(error) = &app.stock_report.error {
            error_banner(ui, error, palette);
        } else if let Some(report) = &app.stock_report.data {
            super::clients::draw_report_blocks(ui, palette, report);
        }
    });
}

/// Stable identifier for the catalogue search, including keyboard shortcuts.
pub(crate) fn product_search_id() -> egui::Id {
    egui::Id::new("product-search")
}

fn draw_search(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            icon(ui, Icon::Search, palette.secondary, 18.0);
            let field_width = (ui.available_width() - 130.0).max(40.0);
            let response = ui.add_sized(
                [field_width, crate::theme::TARGET_MIN_HEIGHT],
                egui::TextEdit::singleline(&mut app.product_search)
                    .id(product_search_id())
                    .hint_text(t!(app.locale, "Search by name, SKU or barcode"))
                    .margin(egui::Margin::symmetric(14, 8))
                    .clip_text(true),
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
            let wide = ui.available_width() >= 1120.0;
            ui.horizontal_centered(|ui| {
                let source = product.image_url.as_deref().unwrap_or("");
                let visible = ui.is_rect_visible(egui::Rect::from_min_size(
                    ui.next_widget_position(),
                    vec2(36.0, 36.0),
                ));
                let image = if visible && !source.is_empty() {
                    app.client
                        .as_ref()
                        .and_then(|client| app.product_images.get(source, client, &app.runner))
                } else {
                    None
                };
                if let Some(uri) = image {
                    ui.add(egui::Image::new(uri).fit_to_exact_size(vec2(36.0, 36.0)));
                } else {
                    icon(ui, Icon::Package, palette.accent, 20.0);
                }
                let identity_width = if wide {
                    (ui.available_width() - 780.0).max(120.0)
                } else {
                    ui.available_width().max(40.0)
                };
                ui.allocate_ui_with_layout(
                    vec2(identity_width, 40.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&product.name)
                                    .color(palette.text)
                                    .strong()
                                    .size(14.0),
                            )
                            .truncate(),
                        );
                        let subtitle = product.subtitle();
                        if !subtitle.is_empty() {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(subtitle).color(palette.dim).size(12.0),
                                )
                                .truncate(),
                            );
                        }
                    },
                );
                if wide {
                    product_actions(app, ui, palette, &product);
                }
            });
            if !wide {
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| product_actions(app, ui, palette, &product));
            }
        });
        ui.add_space(8.0);
    }
}

fn product_actions(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette, product: &Product) {
    if app.role_policy().show_prices() {
        let currency = app
            .dashboard
            .data
            .as_ref()
            .map(|d| d.currency())
            .unwrap_or("MAD");
        ui.label(
            RichText::new(atlas_core::format::money(product.price, currency))
                .color(palette.text)
                .strong()
                .size(13.5),
        );
    }
    stock_pill(ui, app.locale, palette, product);
    if product.tracks_inventory()
        && app.role_policy().can_manage_stock()
        && secondary_button(ui, t!(app.locale, "Adjust stock"), palette)
    {
        app.stock_dialog = Some(StockDialog::new(product));
    }
    if secondary_button(ui, t!(app.locale, "History"), palette) {
        app.stock_history = Some(StockHistory {
            product_id: product.id,
            product_name: product.name.clone(),
            full: false,
            offset: 0,
            rows: Vec::new(),
            loaded: false,
            error: None,
            job: None,
        });
        load_history(app);
    }
    if app.role_policy().can_edit_prices() && secondary_button(ui, t!(app.locale, "Edit"), palette)
    {
        app.product_editor = Some(ProductEditor::for_product(product));
    }
    // The worker's label includes a price, which stock-only users must not see.
    if app.role_policy().show_prices() && secondary_button(ui, t!(app.locale, "Label"), palette) {
        open_product_label(app, product);
    }
    if app.role_policy().can_delete_products() {
        let current = app
            .product_delete
            .as_ref()
            .filter(|pending| pending.id == product.id);
        if let Some(pending) = current {
            if pending.job.is_some() {
                ui.label(RichText::new(t!(app.locale, "Deleting…")).color(palette.secondary));
            } else {
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
                    app.product_delete = None;
                }
            }
        } else {
            ui.add_enabled_ui(app.product_delete.is_none(), |ui| {
                if secondary_button(ui, t!(app.locale, "Delete"), palette) {
                    app.product_delete = Some(ProductDelete {
                        id: product.id,
                        deadline: std::time::Instant::now() + std::time::Duration::from_secs(10),
                        job: None,
                        cancel: None,
                    });
                    ui.ctx().request_repaint();
                }
            });
        }
    }
}

fn poll_history(app: &mut AtlasApp, ctx: &egui::Context) {
    if let Some(history) = app.stock_history.as_mut() {
        if let Some(result) = history.job.as_ref().and_then(Job::poll) {
            history.job = None;
            history.loaded = true;
            match result {
                Ok(rows) => history.rows = rows,
                Err(error) => history.error = Some(error.message),
            }
        }
        if history.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

fn history_contents(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    poll_history(app, ui.ctx());
    let Some(history) = app.stock_history.as_ref() else {
        return;
    };
    ui.label(
        RichText::new(format!(
            "{} · {}",
            history.product_name,
            if history.full {
                t!(app.locale, "All history")
            } else {
                t!(app.locale, "Last 90 days")
            }
        ))
        .color(palette.secondary),
    );
    if !history.full {
        ui.label(
            RichText::new(t!(app.locale, "Showing up to 50 recent movements."))
                .color(palette.secondary)
                .size(12.0),
        );
    }
    ui.add_space(8.0);
    if history.job.is_some() {
        super::list_skeleton(ui, palette, app.locale, app.settings.reduced_motion);
        return;
    }
    if let Some(error) = &history.error {
        error_banner(ui, error, palette);
        if secondary_button(ui, t!(app.locale, "Retry"), palette) {
            load_history(app);
        }
        return;
    }
    if history.loaded && history.rows.is_empty() {
        ui.label(RichText::new(t!(app.locale, "No stock movements.")).color(palette.secondary));
    }
    for movement in &history.rows {
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(movement.created_at.as_deref().unwrap_or("—"))
                        .color(palette.text)
                        .strong(),
                );
                ui.label(RichText::new(&movement.kind).color(palette.secondary));
                for (label, value) in [
                    (t!(app.locale, "Quantity"), movement.quantity),
                    (t!(app.locale, "Before"), movement.previous_quantity),
                    (t!(app.locale, "After"), movement.new_quantity),
                ] {
                    ui.label(
                        RichText::new(format!(
                            "{}: {}",
                            label,
                            value
                                .filter(|n| n.is_finite())
                                .map(number)
                                .unwrap_or_else(|| "—".into())
                        ))
                        .color(palette.text),
                    );
                }
            });
            if let Some(reason) = movement.reason.as_deref().filter(|v| !v.is_empty()) {
                ui.label(RichText::new(reason).color(palette.secondary));
            }
            if let Some(reference) = movement.reference.as_deref().filter(|v| !v.is_empty()) {
                ui.label(RichText::new(reference).color(palette.secondary));
            }
        });
    }
    let full = history.full;
    let offset = history.offset;
    let next = history.rows.len() == 50;
    if full {
        ui.horizontal(|ui| {
            if offset > 0 && secondary_button(ui, t!(app.locale, "Previous"), palette) {
                if let Some(history) = app.stock_history.as_mut() {
                    history.offset = offset.saturating_sub(50);
                }
                load_history(app);
            }
            if next && secondary_button(ui, t!(app.locale, "Next"), palette) {
                if let Some(history) = app.stock_history.as_mut() {
                    history.offset = offset.saturating_add(50);
                }
                load_history(app);
            }
        });
    }
}

fn draw_history_page(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.horizontal(|ui| {
        page_title(ui, t!(app.locale, "Stock history"), palette);
        if secondary_button(ui, t!(app.locale, "Back to products"), palette) {
            app.stock_history = None;
        }
        if app.stock_history.is_some() && secondary_button(ui, t!(app.locale, "Refresh"), palette) {
            load_history(app);
        }
    });
    if app.stock_history.is_some() {
        history_contents(app, ui, palette);
    }
}

fn draw_history_dialog(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let screen = ui.ctx().input(|i| i.viewport_rect());
    let rect = egui::Rect::from_center_size(
        screen.center(),
        vec2(
            800.0_f32.min((screen.width() - 32.0).max(1.0)),
            580.0_f32.min((screen.height() - 32.0).max(1.0)),
        ),
    );
    if widgets::scrim_clicked(ui, screen, rect, egui::Id::new("stock-history-scrim")) {
        app.stock_history = None;
        return;
    }
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(t!(app.locale, "Movement history"))
                    .color(palette.text)
                    .strong()
                    .size(18.0),
            );
            egui::ScrollArea::vertical()
                .max_height((rect.height() - 120.0).max(1.0))
                .show(ui, |ui| history_contents(app, ui, palette));
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                if app
                    .stock_history
                    .as_ref()
                    .is_some_and(|history| history.job.is_none())
                    && secondary_button(ui, t!(app.locale, "View full history"), palette)
                {
                    if let Some(history) = app.stock_history.as_mut() {
                        history.full = true;
                        history.offset = 0;
                        history.job = None;
                    }
                    load_history(app);
                }
                if secondary_button(ui, t!(app.locale, "Close"), palette) {
                    app.stock_history = None;
                }
            });
        });
    });
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
    // Stock is a finance-style status: the Android semantic pairs carry
    // the out-of-stock / low / healthy states (soft fill + strong text).
    let (fill, tint) = if product.is_out_of_stock() {
        (palette.finance_danger_soft, palette.finance_danger)
    } else if product.is_low_stock() {
        (palette.finance_warning_soft, palette.finance_warning)
    } else {
        (palette.finance_positive_soft, palette.finance_positive)
    };
    pill(ui, text, fill, tint);
}

fn load_images(app: &mut AtlasApp) {
    let (Some(api), Some(editor)) = (app.client.clone(), app.product_editor.as_mut()) else {
        return;
    };
    let Some(id) = editor.editing_id else {
        return;
    };
    if editor.images.loading {
        return;
    }
    editor.images.start(
        app.runner
            .spawn(async move { api.product_images(id).await }),
    );
}

fn poll_images(app: &mut AtlasApp, ctx: &egui::Context) {
    let Some(editor) = app.product_editor.as_mut() else {
        return;
    };
    if editor.images.poll() {
        ctx.request_repaint();
    }
    if let Some(result) = editor.stage_job.as_ref().and_then(Job::poll) {
        editor.stage_job = None;
        match result {
            Ok(data) => {
                editor.staged_images.push(data);
                editor.image_error = None;
            }
            Err(error) => editor.image_error = Some(error.message),
        }
        ctx.request_repaint();
    }
    if let Some(result) = editor.image_job.as_ref().and_then(Job::poll) {
        editor.image_job = None;
        match result {
            Ok(()) => {
                editor.image_error = None;
                editor.image_confirm = None;
                load_images(app);
                app.load_products();
            }
            Err(error) => editor.image_error = Some(error.message),
        }
        ctx.request_repaint();
    }
    if app.product_editor.as_ref().is_some_and(|editor| {
        editor.images.loading || editor.image_job.is_some() || editor.stage_job.is_some()
    }) {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// Read only the file explicitly dropped by the user, on a worker thread.
/// Bound both the raw file and the encoded request; paths never enter logs.
fn dropped_image_data(
    path: Option<std::path::PathBuf>,
    bytes: Option<Vec<u8>>,
) -> Result<String, ApiError> {
    use base64::Engine as _;
    use std::io::Read as _;
    const MAX: usize = 4 * 1024 * 1024;
    let bytes = if let Some(bytes) = bytes {
        bytes
    } else {
        let path = path.ok_or_else(|| ApiError::client("Fichier image indisponible."))?;
        let metadata = std::fs::metadata(&path)
            .map_err(|_| ApiError::client("Impossible de lire l’image."))?;
        if !metadata.is_file() || metadata.len() > MAX as u64 {
            return Err(ApiError::client(
                "L’image doit être un fichier de 4 Mo maximum.",
            ));
        }
        let file = std::fs::File::open(path)
            .map_err(|_| ApiError::client("Impossible de lire l’image."))?;
        let metadata = file
            .metadata()
            .map_err(|_| ApiError::client("Impossible de lire l’image."))?;
        if !metadata.is_file() || metadata.len() > MAX as u64 {
            return Err(ApiError::client(
                "L’image doit être un fichier de 4 Mo maximum.",
            ));
        }
        let mut data = Vec::new();
        file.take((MAX + 1) as u64)
            .read_to_end(&mut data)
            .map_err(|_| ApiError::client("Impossible de lire l’image."))?;
        data
    };
    if bytes.is_empty() || bytes.len() > MAX {
        return Err(ApiError::client("L’image doit faire au maximum 4 Mo."));
    }
    let mime = match image::guess_format(&bytes).ok() {
        Some(image::ImageFormat::Png) => "image/png",
        Some(image::ImageFormat::Jpeg) => "image/jpeg",
        Some(image::ImageFormat::WebP) => "image/webp",
        Some(image::ImageFormat::Gif) => "image/gif",
        _ => return Err(ApiError::client("Format image non pris en charge.")),
    };
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn draw_staged_images(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let locale = app.locale;
    let dropped = ui
        .ctx()
        .input(|input| input.raw.dropped_files.first().cloned());
    let Some(editor) = app.product_editor.as_mut() else {
        return;
    };
    if let Some(file) = dropped
        && editor.stage_job.is_none()
    {
        if editor.staged_images.len() >= 4 {
            editor.image_error =
                Some(t!(locale, "A maximum of four photos can be staged.").into_owned());
        } else {
            let path = file.path().to_path_buf();
            editor.stage_job = Some(
                app.runner
                    .spawn(async move { dropped_image_data(Some(path), None) }),
            );
            editor.image_error = None;
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    ui.label(RichText::new(t!(locale, "Drop up to four JPEG, PNG, WebP or GIF files, one at a time (4 MB each). They upload after the product is created.")).color(palette.secondary));
    if editor.stage_job.is_some() {
        super::centered_spinner(ui, palette);
    }
    if let Some(error) = &editor.image_error {
        error_banner(ui, error, palette);
    }
    let mut remove = None;
    for index in 0..editor.staged_images.len() {
        ui.horizontal(|ui| {
            ui.label(format!("{} {}", t!(locale, "Photo"), index + 1));
            if secondary_button(ui, t!(locale, "Remove"), palette) {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        editor.staged_images.remove(index);
    }
}

fn draw_images(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let locale = app.locale;
    let dropped = ui
        .ctx()
        .input(|input| input.raw.dropped_files.first().cloned());
    if let Some(file) = dropped
        && let (Some(api), Some(editor)) = (app.client.clone(), app.product_editor.as_mut())
        && let Some(id) = editor.editing_id
        && editor.image_job.is_none()
    {
        if editor.images.loading || editor.images.data.is_none() {
            editor.image_error =
                Some(t!(locale, "Load the image list before adding a photo.").into_owned());
        } else {
            let path = file.path().to_path_buf();
            let is_primary = editor.images.data.as_ref().is_some_and(Vec::is_empty);
            editor.image_error = None;
            editor.image_job = Some(app.runner.spawn(async move {
                let data_url = dropped_image_data(Some(path), None)?;
                api.add_product_image(
                    id,
                    &atlas_api::models::AddProductImage {
                        data_url,
                        is_primary,
                    },
                )
                .await
                .map(|_| ())
            }));
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    let Some(editor) = app.product_editor.as_mut() else {
        return;
    };
    ui.label(
        RichText::new(t!(
            locale,
            "Drop a JPEG, PNG, WebP or GIF here (4 MB maximum). Images are saved immediately."
        ))
        .color(palette.secondary),
    );
    if editor.images.loading {
        super::centered_spinner(ui, palette);
    }
    if let Some(error) = &editor.images.error {
        error_banner(ui, error, palette);
    }
    if let Some(error) = &editor.image_error {
        error_banner(ui, error, palette);
    }
    let images = editor.images.data.clone().unwrap_or_default();
    let id = editor.editing_id.unwrap_or_default();
    for photo in &images {
        ui.horizontal_wrapped(|ui| {
            let texture = app
                .client
                .as_ref()
                .and_then(|api| app.product_images.get(&photo.url, api, &app.runner));
            if let Some(uri) = texture {
                ui.add(egui::Image::new(uri).fit_to_exact_size(vec2(48.0, 48.0)));
            } else {
                icon(ui, Icon::Package, palette.secondary, 20.0);
            }
            if photo.is_primary != 0 {
                ui.label(t!(locale, "Primary image"));
            } else if editor.image_job.is_none()
                && secondary_button(ui, t!(locale, "Set as primary"), palette)
                && let Some(api) = app.client.clone()
            {
                let image_id = photo.id;
                editor.image_job = Some(app.runner.spawn(async move {
                    api.set_primary_product_image(id, image_id)
                        .await
                        .map(|_| ())
                }));
            }
            if editor.image_confirm == Some(photo.id) {
                if editor.image_job.is_none()
                    && secondary_button(ui, t!(locale, "Confirm deletion"), palette)
                    && let Some(api) = app.client.clone()
                {
                    let image_id = photo.id;
                    editor.image_job = Some(
                        app.runner
                            .spawn(async move { api.delete_product_image(id, image_id).await }),
                    );
                }
                if secondary_button(ui, t!(locale, "Cancel"), palette) {
                    editor.image_confirm = None;
                }
            } else if secondary_button(ui, t!(locale, "Delete"), palette) {
                editor.image_confirm = Some(photo.id);
            }
        });
    }
    if secondary_button(ui, t!(locale, "Refresh"), palette) {
        load_images(app);
    }
}

fn load_variants(app: &mut AtlasApp) {
    let (Some(api), Some(editor)) = (app.client.clone(), app.product_editor.as_mut()) else {
        return;
    };
    let Some(id) = editor.editing_id else {
        return;
    };
    if editor.variants.loading {
        return;
    }
    editor.variants.start(
        app.runner
            .spawn(async move { api.product_variants(id).await }),
    );
}

fn poll_variants(app: &mut AtlasApp, ctx: &egui::Context) {
    let Some(editor) = app.product_editor.as_mut() else {
        return;
    };
    if editor.variants.poll() {
        ctx.request_repaint();
    }
    if let Some(result) = editor.variant_job.as_ref().and_then(Job::poll) {
        editor.variant_job = None;
        match result {
            Ok(()) => {
                editor.variant_error = None;
                editor.variant_confirm = None;
                editor.variant_label.clear();
                editor.variant_sku.clear();
                editor.variant_barcode.clear();
                editor.variant_price.clear();
                load_variants(app);
            }
            Err(error) => editor.variant_error = Some(error.message),
        }
        ctx.request_repaint();
    }
    if app
        .product_editor
        .as_ref()
        .is_some_and(|editor| editor.variants.loading || editor.variant_job.is_some())
    {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

fn draw_variants(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let locale = app.locale;
    let can_adjust = app.role_policy().can_manage_stock();
    let Some(editor) = app.product_editor.as_mut() else {
        return;
    };
    ui.label(
        RichText::new(t!(locale, "Variant changes are saved immediately."))
            .color(palette.secondary),
    );
    if editor.variants.loading {
        super::centered_spinner(ui, palette);
    }
    if let Some(error) = &editor.variants.error {
        error_banner(ui, error, palette);
    }
    if let Some(error) = &editor.variant_error {
        error_banner(ui, error, palette);
    }
    let variants = editor.variants.data.clone().unwrap_or_default();
    for variant in variants.iter().filter(|variant| variant.is_active != 0) {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(variant.variant_label.as_deref().unwrap_or(&variant.name))
                    .strong()
                    .color(palette.text),
            );
            if let Some(sku) = variant.sku.as_deref().filter(|value| !value.is_empty()) {
                ui.label(sku);
            }
            ui.label(format!(
                "{}: {}",
                t!(locale, "Stock"),
                number(variant.stock)
            ));
            if variant.track_inventory != 0
                && can_adjust
                && secondary_button(ui, t!(locale, "Adjust stock"), palette)
            {
                app.stock_dialog = Some(StockDialog::new(&Product {
                    id: variant.id,
                    name: variant.name.clone(),
                    stock: variant.stock,
                    track_inventory: variant.track_inventory,
                    ..Product::default()
                }));
            }
            if editor.variant_confirm == Some(variant.id) {
                if secondary_button(ui, t!(locale, "Confirm deactivation"), palette)
                    && editor.variant_job.is_none()
                    && let Some(api) = app.client.clone()
                {
                    let id = variant.id;
                    editor.variant_job = Some(
                        app.runner
                            .spawn(async move { api.deactivate_variant(id).await }),
                    );
                }
                if secondary_button(ui, t!(locale, "Cancel"), palette) {
                    editor.variant_confirm = None;
                }
            } else if secondary_button(ui, t!(locale, "Deactivate"), palette) {
                editor.variant_confirm = Some(variant.id);
            }
        });
    }
    if secondary_button(ui, t!(locale, "Refresh"), palette) {
        load_variants(app);
    }
    let Some(editor) = app.product_editor.as_mut() else {
        return;
    };
    ui.separator();
    labelled_field(
        ui,
        t!(locale, "Variant label"),
        &mut editor.variant_label,
        palette,
        false,
    );
    labelled_field(
        ui,
        t!(locale, "SKU"),
        &mut editor.variant_sku,
        palette,
        false,
    );
    labelled_field(
        ui,
        t!(locale, "Barcode"),
        &mut editor.variant_barcode,
        palette,
        false,
    );
    labelled_field(
        ui,
        t!(locale, "Price (optional)"),
        &mut editor.variant_price,
        palette,
        false,
    );
    ui.label(
        RichText::new(t!(
            locale,
            "New variants start at zero stock; adjust stock after creation."
        ))
        .color(palette.secondary),
    );
    if primary_button(
        ui,
        t!(locale, "Add variant"),
        palette,
        editor.variant_job.is_none() && !editor.variant_label.trim().is_empty(),
    ) {
        let parsed_price = if editor.variant_price.trim().is_empty() {
            Some(None)
        } else {
            editor
                .variant_price
                .trim()
                .replace(',', ".")
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1e12)
                .map(Some)
        };
        if let Some(price) = parsed_price {
            if let (Some(api), Some(id)) = (app.client.clone(), editor.editing_id) {
                let body = atlas_api::models::CreateVariant {
                    variant_label: editor.variant_label.trim().to_owned(),
                    sku: optional(&editor.variant_sku),
                    barcode: optional(&editor.variant_barcode),
                    price,
                    stock_quantity: 0.0,
                };
                editor.variant_error = None;
                editor.variant_job = Some(
                    app.runner
                        .spawn(async move { api.create_variant(id, &body).await.map(|_| ()) }),
                );
            }
        } else {
            editor.variant_error = Some(t!(locale, "Invalid price.").into_owned());
        }
    }
}

// ── Create product dialog ──────────────────────────────────────────────────

fn draw_editor(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    poll_variants(app, ui.ctx());
    poll_images(app, ui.ctx());
    // Poll the save job first, so a result applies this frame.
    if let Some(editor) = app.product_editor.as_mut()
        && let Some(job) = &editor.job
        && let Some(result) = job.poll()
    {
        editor.job = None;
        editor.saving = false;
        match result {
            Ok(result) => {
                let editing = editor.editing_id.is_some();
                app.product_editor = None;
                app.notice(
                    if editing {
                        t!(app.locale, "Product updated.")
                    } else {
                        t!(app.locale, "Product created.")
                    }
                    .into_owned(),
                );
                if result.failed_photos > 0 {
                    app.notice(format!(
                        "{} : {}",
                        t!(
                            app.locale,
                            "Product created, but some photos could not be added"
                        ),
                        result.failed_photos
                    ));
                }
                app.load_products();
                return;
            }
            Err(error) => {
                if error.code.as_deref() == Some("DUPLICATE_PRODUCT") {
                    editor.force_duplicate = true;
                    editor.duplicate_identity = Some(editor.identity());
                    editor.error = Some(
                        if editor.editing_id.is_some() {
                            t!(
                                app.locale,
                                "A similar product already exists. Save again to update it anyway."
                            )
                        } else {
                            t!(
                                app.locale,
                                "A similar product already exists. Save again to create it anyway."
                            )
                        }
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
    let rect = egui::Rect::from_center_size(
        screen.center(),
        vec2(520.0, 560.0_f32.min((screen.height() - 32.0).max(1.0))),
    );
    let locale = app.locale;
    if widgets::scrim_clicked(ui, screen, rect, egui::Id::new("product-editor-scrim")) {
        if app
            .product_editor
            .as_ref()
            .is_some_and(ProductEditor::is_busy)
        {
            return;
        }
        // Same dirty-draft decision as the Cancel button and Escape: an
        // incidental outside click must not silently lose unsaved input.
        let dirty = app
            .product_editor
            .as_ref()
            .is_some_and(ProductEditor::is_dirty);
        if dirty {
            app.confirm_dialog = Some(Confirm {
                title: t!(locale, "Discard changes?").into_owned(),
                body: t!(locale, "The product information has not been saved.").into_owned(),
                confirm_label: t!(locale, "Discard").into_owned(),
                danger: true,
                action: ConfirmAction::DiscardEditor,
            });
        } else {
            app.product_editor = None;
        }
        return;
    }

    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height((rect.height() - 46.0).max(1.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(
                            if app
                                .product_editor
                                .as_ref()
                                .is_some_and(|editor| editor.editing_id.is_some())
                            {
                                t!(locale, "Edit product")
                            } else {
                                t!(locale, "New product")
                            },
                        )
                        .color(palette.text)
                        .font(crate::fonts::semibold(17.0)),
                    );
                    ui.add_space(10.0);

                    let dirty = app
                        .product_editor
                        .as_ref()
                        .is_some_and(ProductEditor::is_dirty);
                    let saving = app
                        .product_editor
                        .as_ref()
                        .is_some_and(ProductEditor::is_busy);
                    let step = app.product_editor.as_ref().map_or(0, |editor| editor.step);
                    let editing = app.product_editor.as_ref().is_some_and(|editor| editor.editing_id.is_some());
                    let total = if editing { 6 } else { 5 };
                    let name_empty = app
                        .product_editor
                        .as_ref()
                        .is_some_and(|editor| editor.name.trim().is_empty());
                    let error = app
                        .product_editor
                        .as_ref()
                        .and_then(|editor| editor.error.clone());
                    let section = match step {
                        0 => t!(locale, "Identity"),
                        1 => t!(locale, "Classification"),
                        2 => t!(locale, "Price and stock"),
                        3 => t!(locale, "Volume discounts"),
                        4 if editing => t!(locale, "Variants"),
                        _ => t!(locale, "Photos"),
                    };
                    ui.label(
                        RichText::new(format!(
                            "{} {}/{} · {}",
                            t!(locale, "Step"),
                            step + 1,
                            total,
                            section
                        ))
                        .color(palette.secondary),
                    );
                    ui.add_space(8.0);
                    if let Some(editor) = app.product_editor.as_mut() {
                        match step {
                            0 => {
                                labelled_field(
                                    ui,
                                    t!(locale, "Name"),
                                    &mut editor.name,
                                    palette,
                                    false,
                                );
                                labelled_field(
                                    ui,
                                    t!(locale, "SKU"),
                                    &mut editor.sku,
                                    palette,
                                    false,
                                );
                                labelled_field(
                                    ui,
                                    t!(locale, "Description"),
                                    &mut editor.description,
                                    palette,
                                    false,
                                );
                            }
                            1 => {
                                labelled_field(
                                    ui,
                                    t!(locale, "Category"),
                                    &mut editor.category,
                                    palette,
                                    false,
                                );
                                labelled_field(
                                    ui,
                                    t!(locale, "Unit"),
                                    &mut editor.unit,
                                    palette,
                                    false,
                                );
                                labelled_field(
                                    ui,
                                    t!(locale, "Barcode"),
                                    &mut editor.barcode,
                                    palette,
                                    false,
                                );
                            }
                            2 => {
                                labelled_field(
                                    ui,
                                    t!(locale, "Price (MAD)"),
                                    &mut editor.price,
                                    palette,
                                    false,
                                );
                                if editor.editing_id.is_none() {
                                    labelled_field(
                                        ui,
                                        t!(locale, "Initial stock"),
                                        &mut editor.stock,
                                        palette,
                                        false,
                                    );
                                }
                                labelled_field(
                                    ui,
                                    t!(locale, "Low stock threshold"),
                                    &mut editor.threshold,
                                    palette,
                                    false,
                                );
                            }
                            3 => {
                                if editor.bulk_available {
                                    ui.label(RichText::new(t!(locale, "Discounts apply at the quantity threshold when no manual promotional price is set.")).color(palette.secondary));
                                    for index in 0..2 {
                                        ui.label(RichText::new(format!("{} {}", t!(locale, "Tier"), index + 1)).strong().color(palette.text));
                                        labelled_field(ui, t!(locale, "Minimum quantity"), &mut editor.bulk_qty[index], palette, false);
                                        labelled_field(ui, t!(locale, "Discount (%)"), &mut editor.bulk_percent[index], palette, false);
                                    }
                                    if !editor.bulk_extra.is_empty() {
                                        ui.label(RichText::new(t!(locale, "Additional server tiers will be preserved.")).color(palette.secondary));
                                    }
                                } else {
                                    ui.label(RichText::new(t!(locale, "Discount data is unavailable; existing server values will be preserved.")).color(palette.secondary));
                                }
                            }
                            _ => {}
                        }
                    }
                    if step == 4 && editing { draw_variants(app, ui, palette); }
                    if step == 4 && !editing { draw_staged_images(app, ui, palette); }
                    if step == 5 { draw_images(app, ui, palette); }

                    if let Some(error) = &error {
                        error_banner(ui, error, palette);
                    }

                    ui.add_space(10.0);
                    widgets::dialog_actions(ui, |ui| {
                        let save = primary_button(
                            ui,
                            if step + 1 == total {
                                t!(locale, "Save")
                            } else {
                                t!(locale, "Next")
                            },
                            palette,
                            !saving && !name_empty,
                        );
                        if step > 0
                            && !saving
                            && secondary_button(ui, t!(locale, "Previous"), palette)
                            && let Some(editor) = app.product_editor.as_mut()
                        {
                            editor.step -= 1;
                            editor.error = None;
                        }
                        let cancel = !saving && secondary_button(ui, t!(locale, "Cancel"), palette);
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
                            if step + 1 < total {
                                if let Some(editor) = app.product_editor.as_mut() {
                                    editor.step += 1;
                                    editor.error = None;
                                }
                                if editing && step + 1 == 4 { load_variants(app); }
                                if editing && step + 1 == 5 { load_images(app); }
                            } else {
                                submit_product(app);
                            }
                        }
                    });
                });
        });
    });
}

impl ProductEditor {
    pub(crate) fn is_dirty(&self) -> bool {
        self.editing_id.is_some()
            || !self.name.is_empty()
            || !self.sku.is_empty()
            || !self.barcode.is_empty()
            || !self.category.is_empty()
            // The form seeds Unit with its default "unité"; only a deviation
            // (not any non-empty value) counts as an unsaved edit.
            || self.unit.trim() != "unité"
            || self.price.trim() != "0"
            || self.stock.trim() != "0"
            || self.threshold.trim() != "5"
            || !self.description.trim().is_empty()
            || self.bulk_qty.iter().any(|v| !v.trim().is_empty())
            || self.bulk_percent.iter().any(|v| !v.trim().is_empty())
            || !self.staged_images.is_empty()
    }

    fn identity(&self) -> (String, String, String) {
        (
            self.name.trim().to_owned(),
            self.sku.trim().to_owned(),
            self.barcode.trim().to_owned(),
        )
    }

    fn parse_bulk(&self) -> Result<Option<Vec<atlas_api::models::BulkDiscount>>, String> {
        if !self.bulk_available {
            return Ok(None);
        }
        let mut tiers = self.bulk_extra.clone();
        for index in 0..2 {
            let qty = self.bulk_qty[index].trim();
            let pct = self.bulk_percent[index].trim();
            if qty.is_empty() && pct.is_empty() {
                continue;
            }
            let qty = qty
                .replace(',', ".")
                .parse::<f64>()
                .map_err(|_| "Quantité minimale invalide.".to_owned())?;
            let pct = pct
                .replace(',', ".")
                .parse::<f64>()
                .map_err(|_| "Remise invalide.".to_owned())?;
            if !qty.is_finite() || qty <= 0.0 || !pct.is_finite() || pct <= 0.0 || pct > 100.0 {
                return Err(
                    "La quantité doit être positive et la remise comprise entre 0 et 100 %."
                        .to_owned(),
                );
            }
            tiers.push(atlas_api::models::BulkDiscount {
                min_qty: qty,
                percent: pct,
            });
        }
        Ok(Some(tiers))
    }

    fn parse(&self) -> Result<atlas_api::models::CreateProduct, String> {
        let parse_amount = |value: &str, field: &str| -> Result<f64, String> {
            let amount = value
                .trim()
                .replace(',', ".")
                .parse::<f64>()
                .map_err(|_| format!("{field} : valeur invalide."))?;
            if !amount.is_finite() || amount < 0.0 {
                return Err(format!("{field} : valeur invalide."));
            }
            Ok(amount)
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
            stock_quantity: if self.editing_id.is_some() {
                0.0
            } else {
                parse_amount(&self.stock, "Stock initial")?
            },
            low_stock_threshold: if self.threshold.trim().is_empty() {
                0.0
            } else {
                parse_amount(&self.threshold, "Seuil de stock bas")?
            },
            description: optional(&self.description),
            bulk_discounts: self.parse_bulk()?,
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
    let force =
        editor.force_duplicate && editor.duplicate_identity.as_ref() == Some(&editor.identity());
    let editing_id = editor.editing_id;
    let update = atlas_api::models::UpdateProduct {
        name: product.name.clone(),
        sku: product.sku.clone(),
        barcode: product.barcode.clone().unwrap_or_default(),
        category: product.category.clone().unwrap_or_default(),
        unit: product.unit.clone(),
        price: product.price,
        low_stock_threshold: product.low_stock_threshold,
        description: product.description.clone().unwrap_or_default(),
        force_duplicate: force,
        bulk_discounts: product.bulk_discounts.clone(),
    };
    let photos = editor.staged_images.clone();
    let job = app.runner.spawn(async move {
        if let Some(id) = editing_id {
            client.update_product(id, &update).await?;
            Ok(ProductSaveResult { failed_photos: 0 })
        } else {
            let created = client.create_product(&product, force).await?;
            let mut failed_photos = 0;
            let mut uploaded = 0;
            for data_url in photos {
                if created.id <= 0 {
                    failed_photos += 1;
                    continue;
                }
                let image = atlas_api::models::AddProductImage {
                    data_url,
                    is_primary: uploaded == 0,
                };
                if client.add_product_image(created.id, &image).await.is_ok() {
                    uploaded += 1;
                } else {
                    failed_photos += 1;
                }
            }
            Ok(ProductSaveResult { failed_photos })
        }
    });
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
                if app
                    .product_editor
                    .as_ref()
                    .is_some_and(|editor| editor.editing_id.is_some())
                {
                    load_variants(app);
                }
                return;
            }
            Err(error) => dialog.error = Some(error.message),
        }
    }
    if app.stock_dialog.is_none() {
        return;
    }

    let screen = ui.ctx().input(|i| i.viewport_rect());
    let rect = egui::Rect::from_center_size(
        screen.center(),
        vec2(
            460.0_f32.min((screen.width() - 32.0).max(1.0)),
            460.0_f32.min((screen.height() - 32.0).max(1.0)),
        ),
    );
    if widgets::scrim_clicked(ui, screen, rect, egui::Id::new("stock-scrim")) {
        app.stock_dialog = None;
        return;
    }

    let locale = app.locale;
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height((rect.height() - 46.0).max(1.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(t!(locale, "Adjust stock"))
                            .color(palette.text)
                            .font(crate::fonts::semibold(17.0)),
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
                    if let Some(dialog) = app.stock_dialog.as_ref() {
                        ui.label(
                            RichText::new(format!(
                                "{}: {}",
                                t!(locale, "Current stock"),
                                number(dialog.current_stock)
                            ))
                            .color(palette.secondary),
                        );
                    }
                    ui.add_space(10.0);

                    ui.horizontal_wrapped(|ui| {
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
                            .min_size(egui::vec2(0.0, crate::theme::TARGET_MIN_HEIGHT))
                            .fill(if selected {
                                palette.accent
                            } else {
                                palette.surface
                            })
                            .corner_radius(crate::theme::RADIUS_CONTROL as f32);
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
                        let confirm = primary_button(
                            ui,
                            t!(locale, "Apply"),
                            palette,
                            !saving && !quantity_empty,
                        );
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
    if !quantity.is_finite() || quantity <= 0.0 {
        dialog.error = Some("La quantité doit être supérieure à zéro.".to_owned());
        return;
    }
    let body = StockAdjust {
        quantity,
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

#[cfg(test)]
mod product_editor_tests {
    use super::ProductEditor;

    #[test]
    fn fresh_defaults_are_clean_and_wire_stock_is_not_used_on_edit() {
        let mut editor = ProductEditor::new();
        assert!(!editor.is_dirty());
        editor.name = "Produit".into();
        assert!(editor.is_dirty());
        assert_eq!(editor.parse().unwrap().stock_quantity, 0.0);
        editor.editing_id = Some(42);
        editor.stock = "-12".into();
        assert_eq!(editor.parse().unwrap().stock_quantity, 0.0);
    }

    #[test]
    fn staged_photos_mark_new_editor_dirty() {
        let mut editor = ProductEditor::new();
        assert!(!editor.is_dirty());
        editor
            .staged_images
            .push("data:image/png;base64,AA==".into());
        assert!(editor.is_dirty());
        editor.staged_images.clear();
        assert!(!editor.is_dirty());
    }

    #[test]
    fn discounts_keep_extra_tiers_and_never_overwrite_missing_data() {
        use super::parse_bulk_from_product;
        use atlas_api::models::Product;
        let raw = serde_json::json!(
            "[{\"min_qty\":50,\"percent\":10},{\"min_qty\":10,\"percent\":5},{\"min_qty\":100,\"percent\":15}]"
        );
        let product = Product {
            bulk_discounts: Some(raw),
            ..Default::default()
        };
        let mut editor = ProductEditor::for_product(&product);
        assert_eq!(editor.bulk_qty[0], "10");
        assert_eq!(editor.bulk_qty[1], "50");
        editor.bulk_percent[0] = "6".into();
        let tiers = editor.parse_bulk().unwrap().unwrap();
        assert_eq!(tiers.len(), 3);
        assert!(
            tiers
                .iter()
                .any(|tier| tier.min_qty == 100.0 && tier.percent == 15.0)
        );
        assert!(
            tiers
                .iter()
                .any(|tier| tier.min_qty == 10.0 && tier.percent == 6.0)
        );
        assert!(parse_bulk_from_product(None).is_none());
        assert!(parse_bulk_from_product(Some(&serde_json::json!("invalid"))).is_none());
        let missing = ProductEditor::for_product(&Product::default());
        assert!(missing.parse_bulk().unwrap().is_none());
        editor.bulk_percent[0] = "101".into();
        assert!(editor.parse_bulk().is_err());
    }

    #[test]
    fn dropped_image_checks_format_and_size_before_encoding() {
        use super::dropped_image_data;
        let png_header = vec![137, 80, 78, 71, 13, 10, 26, 10];
        let data = dropped_image_data(None, Some(png_header)).unwrap();
        assert!(data.starts_with("data:image/png;base64,"));
        assert!(dropped_image_data(None, Some(vec![0; 32])).is_err());
        assert!(dropped_image_data(None, Some(vec![0; 4 * 1024 * 1024 + 1])).is_err());
        assert!(dropped_image_data(None, None).is_err());
    }

    #[test]
    fn rejects_non_finite_and_negative_amounts() {
        let mut editor = ProductEditor::new();
        editor.name = "Produit".into();
        for value in ["-1", "NaN", "inf"] {
            editor.price = value.into();
            assert!(editor.parse().is_err());
        }
        editor.price = "1,5".into();
        assert_eq!(editor.parse().unwrap().price, 1.5);
    }
}
