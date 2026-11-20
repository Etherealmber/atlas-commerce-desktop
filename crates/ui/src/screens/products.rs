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
    pub saving: bool,
    /// A 409 `DUPLICATE_PRODUCT` was answered; the next save forces it.
    pub force_duplicate: bool,
    duplicate_identity: Option<(String, String, String)>,
    pub error: Option<String>,
    pub job: Option<Job<Result<Product, ApiError>>>,
}

impl ProductEditor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            unit: "unité".to_owned(),
            price: "0".to_owned(),
            stock: "0".to_owned(),
            threshold: "5".to_owned(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn for_product(product: &Product) -> Self {
        Self {
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
                        .is_some_and(|editor| editor.saving);
                    let step = app.product_editor.as_ref().map_or(0, |editor| editor.step);
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
                        _ => t!(locale, "Price and stock"),
                    };
                    ui.label(
                        RichText::new(format!(
                            "{} {}/3 · {}",
                            t!(locale, "Step"),
                            step + 1,
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
                            _ => {
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
                        }
                    }

                    if let Some(error) = &error {
                        error_banner(ui, error, palette);
                    }

                    ui.add_space(10.0);
                    widgets::dialog_actions(ui, |ui| {
                        let save = primary_button(
                            ui,
                            if step == 2 {
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
                            if step < 2 {
                                if let Some(editor) = app.product_editor.as_mut() {
                                    editor.step += 1;
                                    editor.error = None;
                                }
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
    }

    fn identity(&self) -> (String, String, String) {
        (
            self.name.trim().to_owned(),
            self.sku.trim().to_owned(),
            self.barcode.trim().to_owned(),
        )
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
    };
    let job = app.runner.spawn(async move {
        if let Some(id) = editing_id {
            client.update_product(id, &update).await
        } else {
            client.create_product(&product, force).await
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
