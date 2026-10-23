//! Vendors (purchasing): searchable list with bill balances, a detail
//! panel and a create/edit editor. Wire contract: the `/api/vendors`
//! block of the worker; validation mirrors `validateVendorWrite`.

use atlas_api::ApiError;
use atlas_api::models::{Vendor, VendorId, VendorWrite};
use atlas_core::format::money;
use atlas_core::jobs::Job;
use egui::{RichText, Sense, vec2};

use crate::icons::Icon;
use crate::state::AtlasApp;
use crate::theme::Palette;
use crate::widgets::{
    card, empty_state, error_banner, icon, info_banner, labelled_field, page_title,
    primary_button, secondary_button,
};
use crate::{t, widgets};

/// Create/edit form. `editing_id` is `None` for creation.
#[derive(Default)]
pub struct VendorEditor {
    pub editing_id: Option<i64>,
    pub name: String,
    pub ice: String,
    pub if_number: String,
    pub address: String,
    pub city: String,
    pub phone: String,
    pub email: String,
    pub saving: bool,
    pub error: Option<String>,
    pub job: Option<Job<Result<Vendor, ApiError>>>,
    /// The update endpoint answers `{ id }` instead of the full row; the
    /// job type is normalised in `submit_editor`.
    pub update_job: Option<Job<Result<VendorId, ApiError>>>,
}

impl VendorEditor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn for_vendor(vendor: &Vendor) -> Self {
        Self {
            editing_id: Some(vendor.id),
            name: vendor.name.clone(),
            ice: vendor.ice.clone(),
            if_number: vendor.if_number.clone(),
            address: vendor.address.clone(),
            city: vendor.city.clone(),
            phone: vendor.phone.clone(),
            email: vendor.email.clone(),
            ..Self::default()
        }
    }

    /// Worker rules: name 1..=200, optional valid email.
    fn validate(&self) -> Option<&'static str> {
        if self.name.trim().is_empty() {
            return Some("The name is required.");
        }
        if self.name.chars().count() > 200 {
            return Some("The name must be 200 characters or fewer.");
        }
        let email = self.email.trim();
        if !email.is_empty() && (!email.contains('@') || !email.contains('.')) {
            return Some("The e-mail address is invalid.");
        }
        None
    }

    fn to_write(&self) -> VendorWrite {
        VendorWrite {
            name: self.name.trim().to_owned(),
            ice: self.ice.trim().to_owned(),
            if_number: self.if_number.trim().to_owned(),
            address: self.address.trim().to_owned(),
            city: self.city.trim().to_owned(),
            phone: self.phone.trim().to_owned(),
            email: self.email.trim().to_owned(),
        }
    }
}

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    poll_editor(app);

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        page_title(ui, t!(app.locale, "Vendors"), palette);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if primary_button(ui, t!(app.locale, "New vendor"), palette, true) {
                app.vendor_editor = Some(VendorEditor::new());
            }
            if secondary_button(ui, t!(app.locale, "Refresh"), palette) {
                app.load_vendors();
            }
        });
    });
    ui.add_space(12.0);

    if let Some(error) = app.vendors.error.clone() {
        error_banner(ui, &error, palette);
        ui.add_space(10.0);
    }

    ui.horizontal_top(|ui| {
        card(palette).show(ui, |ui| {
            ui.set_width((ui.available_width() * 0.55).max(300.0));
            vendor_list(app, ui, palette);
        });
        card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            vendor_detail(app, ui, palette);
        });
    });

    if app.vendor_editor.is_some() {
        draw_editor(app, ui, palette);
    }
}

fn vendor_list(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.horizontal(|ui| {
        icon(ui, Icon::Search, palette.secondary, 16.0);
        ui.add(
            egui::TextEdit::singleline(&mut app.vendor_search)
                .hint_text(t!(app.locale, "Search by name, city, phone or ICE"))
                .margin(egui::Margin::symmetric(10, 8))
                .desired_width(ui.available_width() - 24.0),
        );
    });
    ui.add_space(10.0);

    if app.vendors.loading && app.vendors.data.is_none() {
        super::spinner(ui, palette);
        return;
    }
    let vendors = app.vendors.data.clone().unwrap_or_default();
    if vendors.is_empty() {
        empty_state(
            ui,
            Icon::Store,
            t!(app.locale, "No vendor yet"),
            t!(app.locale, "Created vendors appear here."),
            palette,
        );
        return;
    }

    let query = app.vendor_search.to_lowercase();
    let currency = app
        .dashboard
        .data
        .as_ref()
        .map(|data| data.currency())
        .unwrap_or("MAD");
    let selected = app.selected_vendor;
    for vendor in vendors
        .iter()
        .filter(|vendor| {
            let q = query.as_str();
            query.is_empty()
                || vendor.name.to_lowercase().contains(q)
                || vendor.city.to_lowercase().contains(q)
                || vendor.phone.contains(q)
                || vendor.ice.to_lowercase().contains(q)
        })
    {
        let is_active = selected == Some(vendor.id);
        let height = 58.0;
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
        if is_active {
            ui.painter().rect_filled(rect, 10.0, palette.surface_active);
        } else if response.hovered() {
            ui.painter().rect_filled(rect, 10.0, palette.surface_hover);
        }
        if response.clicked() {
            app.selected_vendor = Some(vendor.id);
        }
        widgets::overlay_at(
            ui,
            egui::Rect::from_min_size(
                rect.min + vec2(8.0, 0.0),
                rect.size() - vec2(16.0, 0.0),
            ),
            |ui| {
                ui.horizontal_centered(|ui| {
                    icon(ui, Icon::Store, palette.secondary, 18.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(vendor.name.clone())
                                .color(palette.text)
                                .strong()
                                .size(13.5),
                        );
                        let subline = [vendor.city.clone(), vendor.phone.clone()]
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
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            let tint = if vendor.balance_due > 0.001 {
                                palette.text
                            } else {
                                palette.dim
                            };
                            ui.label(
                                RichText::new(money(vendor.balance_due, currency))
                                    .color(tint)
                                    .strong()
                                    .size(12.5),
                            );
                        },
                    );
                });
            },
        );
        ui.separator();
    }
}

fn vendor_detail(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let vendors = app.vendors.data.clone().unwrap_or_default();
    let Some(vendor) = vendors
        .iter()
        .find(|item| Some(item.id) == app.selected_vendor)
    else {
        empty_state(
            ui,
            Icon::Store,
            t!(app.locale, "No vendor selected"),
            t!(app.locale, "Choose a vendor on the left to see its details."),
            palette,
        );
        return;
    };

    let currency = app
        .dashboard
        .data
        .as_ref()
        .map(|data| data.currency())
        .unwrap_or("MAD");

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(vendor.name.clone())
                .color(palette.text)
                .strong()
                .size(17.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if secondary_button(ui, t!(app.locale, "Edit"), palette) {
                app.vendor_editor = Some(VendorEditor::for_vendor(vendor));
            }
        });
    });
    ui.add_space(10.0);

    if vendor.balance_due > 0.001 {
        info_banner(
            ui,
            format!(
                "{} : {}",
                t!(app.locale, "Outstanding balance"),
                money(vendor.balance_due, currency)
            ),
            palette,
        );
        ui.add_space(10.0);
    }

    detail_row(ui, palette, "ICE", blank(&vendor.ice));
    detail_row(ui, palette, t!(app.locale, "Tax ID (IF)"), blank(&vendor.if_number));
    detail_row(ui, palette, t!(app.locale, "Phone"), blank(&vendor.phone));
    detail_row(ui, palette, t!(app.locale, "Email"), blank(&vendor.email));
    detail_row(ui, palette, t!(app.locale, "Address"), blank(&vendor.address));
    detail_row(ui, palette, t!(app.locale, "City"), blank(&vendor.city));
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
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(label.as_ref())
                .color(palette.secondary)
                .size(12.5),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(value.as_ref())
                    .color(palette.text)
                    .size(13.0),
            );
        });
    });
    ui.add_space(6.0);
}

fn draw_editor(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let screen = ui.ctx().input(|input| input.viewport_rect());
    let rect = egui::Rect::from_center_size(screen.center(), vec2(520.0, 560.0));
    ui.painter()
        .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(140));

    let locale = app.locale;
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_min_size(vec2(476.0, 0.0));
            let title = if app
                .vendor_editor
                .as_ref()
                .is_some_and(|editor| editor.editing_id.is_some())
            {
                t!(locale, "Edit vendor")
            } else {
                t!(locale, "New vendor")
            };
            ui.label(
                RichText::new(title)
                    .color(palette.text)
                    .strong()
                    .size(17.0),
            );
            ui.add_space(10.0);

            if let Some(editor) = app.vendor_editor.as_mut() {
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
                labelled_field(ui, t!(locale, "Address"), &mut editor.address, palette, false);
                ui.add_space(8.0);
                labelled_field(ui, t!(locale, "City"), &mut editor.city, palette, false);
            }

            if let Some(error) = app
                .vendor_editor
                .as_ref()
                .and_then(|editor| editor.error.clone())
            {
                ui.add_space(8.0);
                error_banner(ui, &error, palette);
            }

            ui.add_space(12.0);
            widgets::dialog_actions(ui, |ui| {
                let saving = app
                    .vendor_editor
                    .as_ref()
                    .is_some_and(|editor| editor.saving);
                let save = primary_button(ui, t!(locale, "Save"), palette, !saving);
                let cancel = secondary_button(ui, t!(locale, "Cancel"), palette);
                if cancel {
                    app.vendor_editor = None;
                }
                if save {
                    submit_editor(app);
                }
            });
        });
    });
}

fn poll_editor(app: &mut AtlasApp) {
    // Create job: the worker answers with the stored vendor row.
    if let Some(editor) = app.vendor_editor.as_mut()
        && let Some(job) = &editor.job
        && let Some(result) = job.poll()
    {
        editor.job = None;
        editor.saving = false;
        handle_result(app, result.map(|vendor| vendor.id));
    }
    // Update job: the worker answers with `{ id }`.
    if let Some(editor) = app.vendor_editor.as_mut()
        && let Some(job) = &editor.update_job
        && let Some(result) = job.poll()
    {
        editor.update_job = None;
        editor.saving = false;
        handle_result(app, result.map(|VendorId { id }| id));
    }
}

fn handle_result(app: &mut AtlasApp, result: Result<i64, ApiError>) {
    match result {
        Ok(id) => {
            let was_edit = app
                .vendor_editor
                .as_ref()
                .is_some_and(|editor| editor.editing_id.is_some());
            app.vendor_editor = None;
            app.selected_vendor = Some(id);
            app.notice(if was_edit {
                t!(app.locale, "Vendor updated.").into_owned()
            } else {
                t!(app.locale, "Vendor created.").into_owned()
            });
            app.load_vendors();
        }
        Err(error) => {
            if let Some(editor) = app.vendor_editor.as_mut() {
                editor.error = Some(error.message);
            }
        }
    }
}

fn submit_editor(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let Some(editor) = app.vendor_editor.as_mut() else {
        return;
    };
    if let Some(validation) = editor.validate() {
        editor.error = Some(crate::i18n::gettext(app.locale, validation).into_owned());
        return;
    }
    let body = editor.to_write();
    let editing_id = editor.editing_id;
    editor.saving = true;
    editor.error = None;
    match editing_id {
        Some(id) => {
            let job = app
                .runner
                .spawn(async move { client.update_vendor(id, &body).await });
            editor.update_job = Some(job);
        }
        None => {
            let job = app
                .runner
                .spawn(async move { client.create_vendor(&body).await });
            editor.job = Some(job);
        }
    }
}
