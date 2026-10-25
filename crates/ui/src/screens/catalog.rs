//! Catalog administration: product categories and measurement units.
//! Both tabs follow the same worker contract (GET/POST/PUT, 409 on
//! duplicate names); rows the worker derives from products have no id and
//! are shown read-only. Deletes are password-gated server-side and are
//! handled in a later phase.

use atlas_api::ApiError;
use atlas_api::models::{Category, CategoryWrite, Unit, UnitWrite};
use atlas_core::jobs::Job;
use egui::{RichText, Sense, vec2};

use crate::icons::Icon;
use crate::state::AtlasApp;
use crate::theme::Palette;
use crate::widgets::{
    card, empty_state, error_banner, icon, labelled_field, page_title, primary_button,
    secondary_button,
};
use crate::{t, widgets};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CatalogTab {
    #[default]
    Categories,
    Units,
}

#[derive(Default)]
pub struct CategoryEditor {
    pub editing_id: Option<i64>,
    pub name: String,
    pub description: String,
    pub sort_order: String,
    pub saving: bool,
    pub error: Option<String>,
    pub job: Option<Job<Result<Category, ApiError>>>,
}

impl CategoryEditor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sort_order: "0".to_owned(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn for_category(category: &Category) -> Self {
        Self {
            editing_id: category.id,
            name: category.name.clone(),
            description: category.description.clone(),
            sort_order: category.sort_order.to_string(),
            ..Self::default()
        }
    }

    fn validate(&self) -> Option<&'static str> {
        if self.name.trim().is_empty() {
            return Some("The name is required.");
        }
        if self.sort_order.trim().parse::<i64>().is_err() {
            return Some("The sort order must be a whole number.");
        }
        None
    }

    fn to_write(&self) -> CategoryWrite {
        CategoryWrite {
            name: self.name.trim().to_owned(),
            description: self.description.trim().to_owned(),
            sort_order: self.sort_order.trim().parse().unwrap_or(0),
        }
    }
}

#[derive(Default)]
pub struct UnitEditor {
    pub editing_id: Option<i64>,
    pub name: String,
    pub symbol: String,
    pub sort_order: String,
    pub saving: bool,
    pub error: Option<String>,
    pub job: Option<Job<Result<Unit, ApiError>>>,
}

impl UnitEditor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sort_order: "0".to_owned(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn for_unit(unit: &Unit) -> Self {
        Self {
            editing_id: unit.id,
            name: unit.name.clone(),
            symbol: unit.symbol.clone(),
            sort_order: unit.sort_order.to_string(),
            ..Self::default()
        }
    }

    fn validate(&self) -> Option<&'static str> {
        if self.name.trim().is_empty() {
            return Some("The name is required.");
        }
        if self.sort_order.trim().parse::<i64>().is_err() {
            return Some("The sort order must be a whole number.");
        }
        None
    }

    fn to_write(&self) -> UnitWrite {
        UnitWrite {
            name: self.name.trim().to_owned(),
            symbol: self.symbol.trim().to_owned(),
            sort_order: self.sort_order.trim().parse().unwrap_or(0),
        }
    }
}

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    poll_jobs(app);

    ui.add_space(8.0);
    page_title(ui, t!(app.locale, "Catalog"), palette);
    ui.add_space(12.0);

    tab_bar(app, ui, palette);
    ui.add_space(12.0);

    match app.catalog_tab {
        CatalogTab::Categories => categories_tab(app, ui, palette),
        CatalogTab::Units => units_tab(app, ui, palette),
    }

    if app.category_editor.is_some() {
        category_dialog(app, ui, palette);
    }
    if app.unit_editor.is_some() {
        unit_dialog(app, ui, palette);
    }
}

fn tab_bar(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.horizontal(|ui| {
        for (tab, label, which) in [
            (
                CatalogTab::Categories,
                t!(app.locale, "Categories"),
                Icon::Boxes,
            ),
            (CatalogTab::Units, t!(app.locale, "Units"), Icon::SquarePen),
        ] {
            let active = app.catalog_tab == tab;
            let frame = egui::Frame::NONE
                .fill(if active {
                    palette.surface_active
                } else {
                    palette.surface
                })
                .stroke(egui::Stroke::new(1.0, palette.outline))
                .corner_radius(crate::theme::RADIUS_CARD as f32)
                .inner_margin(egui::Margin::symmetric(14, 8));
            let response = frame
                .show(ui, |ui| {
                    // Tabs are chrome: icon + one-line label, never a wrapped
                    // block when the window gets narrow.
                    widgets::no_wrap_scope(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            icon(
                                ui,
                                which,
                                if active {
                                    palette.accent
                                } else {
                                    palette.secondary
                                },
                                15.0,
                            );
                            ui.label(
                                RichText::new(label)
                                    .color(if active {
                                        palette.text
                                    } else {
                                        palette.secondary
                                    })
                                    .strong()
                                    .size(13.0),
                            );
                        });
                    });
                })
                .response;
            if response.clicked() {
                app.catalog_tab = tab;
            }
        }
    });
}

fn categories_tab(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.horizontal(|ui| {
        if primary_button(ui, t!(app.locale, "New category"), palette, true) {
            app.category_editor = Some(CategoryEditor::new());
        }
        if secondary_button(ui, t!(app.locale, "Refresh"), palette) {
            app.load_categories();
        }
    });
    ui.add_space(10.0);

    if let Some(error) = app.categories.error.clone() {
        error_banner(ui, &error, palette);
        ui.add_space(10.0);
    }
    if app.categories.loading && app.categories.data.is_none() {
        super::centered_spinner(ui, palette);
        return;
    }
    let categories = app.categories.data.clone().unwrap_or_default();
    if categories.is_empty() {
        empty_state(
            ui,
            Icon::Boxes,
            t!(app.locale, "No category yet"),
            t!(app.locale, "Created categories appear here."),
            palette,
        );
        return;
    }
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let virtual_label = t!(app.locale, "from products").to_string();
        for category in &categories {
            let subtitle =
                (!category.description.is_empty()).then_some(category.description.as_str());
            let response = list_row(
                ui,
                palette,
                &category.name,
                subtitle,
                category.id.is_none(),
                &virtual_label,
            );
            if response.clicked() && category.id.is_some() {
                app.category_editor = Some(CategoryEditor::for_category(category));
            }
            ui.separator();
        }
    });
}

fn units_tab(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.horizontal(|ui| {
        if primary_button(ui, t!(app.locale, "New unit"), palette, true) {
            app.unit_editor = Some(UnitEditor::new());
        }
        if secondary_button(ui, t!(app.locale, "Refresh"), palette) {
            app.load_units();
        }
    });
    ui.add_space(10.0);

    if let Some(error) = app.units.error.clone() {
        error_banner(ui, &error, palette);
        ui.add_space(10.0);
    }
    if app.units.loading && app.units.data.is_none() {
        super::centered_spinner(ui, palette);
        return;
    }
    let units = app.units.data.clone().unwrap_or_default();
    if units.is_empty() {
        empty_state(
            ui,
            Icon::SquarePen,
            t!(app.locale, "No unit yet"),
            t!(app.locale, "Created units appear here."),
            palette,
        );
        return;
    }
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let virtual_label = t!(app.locale, "from products").to_string();
        for unit in &units {
            let subtitle = (!unit.symbol.is_empty()).then_some(unit.symbol.as_str());
            let response = list_row(
                ui,
                palette,
                &unit.name,
                subtitle,
                unit.id.is_none(),
                &virtual_label,
            );
            if response.clicked() && unit.id.is_some() {
                app.unit_editor = Some(UnitEditor::for_unit(unit));
            }
            ui.separator();
        }
    });
}

fn list_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    title: &str,
    subtitle: Option<&str>,
    virtual_row: bool,
    virtual_label: &str,
) -> egui::Response {
    let height = 44.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 10.0, palette.surface_hover);
    }
    widgets::overlay_at(
        ui,
        egui::Rect::from_min_size(rect.min + vec2(10.0, 0.0), rect.size() - vec2(20.0, 0.0)),
        |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(RichText::new(title).color(palette.text).strong().size(13.5));
                if virtual_row {
                    widgets::pill(ui, virtual_label, palette.outline, palette.secondary);
                }
                if let Some(subtitle) = subtitle {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(subtitle).color(palette.dim).size(12.0));
                    });
                }
            });
        },
    );
    response
}

fn category_dialog(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let screen = ui.ctx().input(|input| input.viewport_rect());
    let rect = egui::Rect::from_center_size(screen.center(), vec2(480.0, 380.0));
    if widgets::scrim_clicked(ui, screen, rect, egui::Id::new("category-scrim")) {
        app.category_editor = None;
        return;
    }
    let locale = app.locale;
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_min_size(vec2(436.0, 0.0));
            let title = if app
                .category_editor
                .as_ref()
                .is_some_and(|editor| editor.editing_id.is_some())
            {
                t!(locale, "Edit category")
            } else {
                t!(locale, "New category")
            };
            ui.label(
                RichText::new(title)
                    .color(palette.text)
                    .font(crate::fonts::semibold(17.0)),
            );
            ui.add_space(10.0);
            if let Some(editor) = app.category_editor.as_mut() {
                labelled_field(ui, t!(locale, "Name"), &mut editor.name, palette, false);
                ui.add_space(8.0);
                labelled_field(
                    ui,
                    t!(locale, "Description"),
                    &mut editor.description,
                    palette,
                    false,
                );
                ui.add_space(8.0);
                labelled_field(
                    ui,
                    t!(locale, "Sort order"),
                    &mut editor.sort_order,
                    palette,
                    false,
                );
            }
            if let Some(error) = app
                .category_editor
                .as_ref()
                .and_then(|editor| editor.error.clone())
            {
                ui.add_space(8.0);
                error_banner(ui, &error, palette);
            }
            ui.add_space(12.0);
            widgets::dialog_actions(ui, |ui| {
                let saving = app
                    .category_editor
                    .as_ref()
                    .is_some_and(|editor| editor.saving);
                if secondary_button(ui, t!(locale, "Cancel"), palette) {
                    app.category_editor = None;
                }
                if primary_button(ui, t!(locale, "Save"), palette, !saving) {
                    submit_category(app);
                }
            });
        });
    });
}

fn unit_dialog(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let screen = ui.ctx().input(|input| input.viewport_rect());
    let rect = egui::Rect::from_center_size(screen.center(), vec2(480.0, 360.0));
    if widgets::scrim_clicked(ui, screen, rect, egui::Id::new("unit-scrim")) {
        app.unit_editor = None;
        return;
    }
    let locale = app.locale;
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_min_size(vec2(436.0, 0.0));
            let title = if app
                .unit_editor
                .as_ref()
                .is_some_and(|editor| editor.editing_id.is_some())
            {
                t!(locale, "Edit unit")
            } else {
                t!(locale, "New unit")
            };
            ui.label(
                RichText::new(title)
                    .color(palette.text)
                    .font(crate::fonts::semibold(17.0)),
            );
            ui.add_space(10.0);
            if let Some(editor) = app.unit_editor.as_mut() {
                labelled_field(ui, t!(locale, "Name"), &mut editor.name, palette, false);
                ui.add_space(8.0);
                labelled_field(ui, t!(locale, "Symbol"), &mut editor.symbol, palette, false);
                ui.add_space(8.0);
                labelled_field(
                    ui,
                    t!(locale, "Sort order"),
                    &mut editor.sort_order,
                    palette,
                    false,
                );
            }
            if let Some(error) = app
                .unit_editor
                .as_ref()
                .and_then(|editor| editor.error.clone())
            {
                ui.add_space(8.0);
                error_banner(ui, &error, palette);
            }
            ui.add_space(12.0);
            widgets::dialog_actions(ui, |ui| {
                let saving = app.unit_editor.as_ref().is_some_and(|editor| editor.saving);
                if secondary_button(ui, t!(locale, "Cancel"), palette) {
                    app.unit_editor = None;
                }
                if primary_button(ui, t!(locale, "Save"), palette, !saving) {
                    submit_unit(app);
                }
            });
        });
    });
}

fn poll_jobs(app: &mut AtlasApp) {
    if let Some(editor) = app.category_editor.as_mut()
        && let Some(job) = &editor.job
        && let Some(result) = job.poll()
    {
        editor.job = None;
        editor.saving = false;
        match result {
            Ok(_) => {
                let was_edit = editor.editing_id.is_some();
                app.category_editor = None;
                app.notice(if was_edit {
                    t!(app.locale, "Category updated.").into_owned()
                } else {
                    t!(app.locale, "Category created.").into_owned()
                });
                app.load_categories();
            }
            Err(error) => {
                if let Some(editor) = app.category_editor.as_mut() {
                    editor.error = Some(error.message);
                }
            }
        }
    }
    if let Some(editor) = app.unit_editor.as_mut()
        && let Some(job) = &editor.job
        && let Some(result) = job.poll()
    {
        editor.job = None;
        editor.saving = false;
        match result {
            Ok(_) => {
                let was_edit = editor.editing_id.is_some();
                app.unit_editor = None;
                app.notice(if was_edit {
                    t!(app.locale, "Unit updated.").into_owned()
                } else {
                    t!(app.locale, "Unit created.").into_owned()
                });
                app.load_units();
            }
            Err(error) => {
                if let Some(editor) = app.unit_editor.as_mut() {
                    editor.error = Some(error.message);
                }
            }
        }
    }
}

fn submit_category(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let Some(editor) = app.category_editor.as_mut() else {
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
            Some(id) => client.update_category(id, &body).await,
            None => client.create_category(&body).await,
        }
    });
    editor.saving = true;
    editor.error = None;
    editor.job = Some(job);
}

fn submit_unit(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let Some(editor) = app.unit_editor.as_mut() else {
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
            Some(id) => client.update_unit(id, &body).await,
            None => client.create_unit(&body).await,
        }
    });
    editor.saving = true;
    editor.error = None;
    editor.job = Some(job);
}
