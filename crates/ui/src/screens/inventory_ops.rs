//! Inventory locations and consolidated cycle counts, backed by the Worker.
use crate::dialogs::{Confirm, ConfirmAction};
use crate::state::AtlasApp;
use crate::theme::Palette;
use crate::widgets::{
    card, error_banner, labelled_field, page_title, primary_button, secondary_button,
    section_header,
};
use crate::{t, widgets};
use egui::RichText;

#[derive(Default)]
pub(crate) struct InventoryForm {
    pub location_code: String,
    pub location_name: String,
    pub product_id: i64,
    pub quantity: String,
    pub note: String,
}

pub(crate) enum InventoryAction {
    LocationCreated(atlas_api::inventory_ops::StockLocation),
    Counted,
}

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    page_title(ui, t!(app.locale, "Stock operations"), palette);
    ui.add_space(12.0);
    if let Some(error) = &app.inventory_ops.error {
        error_banner(ui, error, palette);
    }
    if let Some(error) = &app.inventory_action.error {
        error_banner(ui, error, palette);
        ui.label(
            RichText::new(t!(
                app.locale,
                "The server may have saved the change. Lists can be cached; verify the result before continuing."
            ))
            .color(palette.secondary),
        );
        if secondary_button(ui, t!(app.locale, "I have verified the result"), palette) {
            app.confirm_dialog = Some(Confirm {
                title: t!(app.locale, "Continue after verification?").into_owned(),
                body: t!(app.locale, "Only continue after checking the actual server state. Retrying an operation that already succeeded can change stock again.").into_owned(),
                confirm_label: t!(app.locale, "I have verified the result").into_owned(),
                danger: true,
                action: ConfirmAction::AcknowledgeInventoryUncertainty,
            });
        }
    }
    if app.inventory_ops.loading && app.inventory_ops.data.is_none() {
        let time = ui.input(|input| input.time);
        widgets::draw_page_skeleton(
            ui,
            palette,
            widgets::PageSkeleton::InventoryOps,
            time,
            app.settings.reduced_motion,
        );
        return;
    }
    let Some(data) = app.inventory_ops.data.clone() else {
        return;
    };
    let ready = !app.inventory_ops.loading
        && app.inventory_ops.error.is_none()
        && !app.inventory_action.loading
        && app.inventory_action.error.is_none();
    section_header(ui, palette, t!(app.locale, "Stock locations"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        for location in &data.locations {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!("{} · {}", location.name, location.code))
                        .strong()
                        .color(palette.text),
                );
                if location.is_default != 0 {
                    widgets::pill(
                        ui,
                        t!(app.locale, "Default"),
                        palette.surface_active,
                        palette.text,
                    );
                }
                if let Some(qty) = location.total_units {
                    ui.label(
                        RichText::new(format!("{} : {qty}", t!(app.locale, "Units")))
                            .color(palette.secondary),
                    );
                }
                if let Some(count) = location.sku_count {
                    ui.label(RichText::new(format!("SKU : {count}")).color(palette.secondary));
                }
            });
            ui.separator();
        }
        labelled_field(
            ui,
            t!(app.locale, "Location code"),
            &mut app.inventory_form.location_code,
            palette,
            false,
        );
        labelled_field(
            ui,
            t!(app.locale, "Location name"),
            &mut app.inventory_form.location_name,
            palette,
            false,
        );
        let valid = !app.inventory_form.location_code.trim().is_empty()
            && app.inventory_form.location_code.trim().chars().count() <= 16
            && !app.inventory_form.location_name.trim().is_empty()
            && app.inventory_form.location_name.trim().chars().count() <= 80;
        if primary_button(
            ui,
            t!(app.locale, "Create location"),
            palette,
            ready && valid,
        ) {
            app.create_inventory_location(
                app.inventory_form.location_code.clone(),
                app.inventory_form.location_name.clone(),
            );
        }
    });
    ui.add_space(14.0);
    section_header(ui, palette, t!(app.locale, "Physical stock count"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(t!(
                app.locale,
                "This count adjusts the product’s consolidated stock, not an individual warehouse."
            ))
            .color(palette.secondary),
        );
        ui.label(
            RichText::new(t!(
                app.locale,
                "The selector shows up to 300 catalogue products."
            ))
            .color(palette.secondary)
            .size(11.0),
        );
        let products: Vec<_> = data
            .products
            .iter()
            .filter(|p| p.tracks_inventory())
            .collect();
        let selected = products
            .iter()
            .find(|p| p.id == app.inventory_form.product_id);
        egui::ComboBox::from_id_salt("inventory-count-product")
            .selected_text(selected.map_or_else(
                || t!(app.locale, "Choose a product").into_owned(),
                |p| p.name.clone(),
            ))
            .show_ui(ui, |ui| {
                for product in &products {
                    ui.selectable_value(
                        &mut app.inventory_form.product_id,
                        product.id,
                        &product.name,
                    );
                }
            });
        if let Some(product) = selected {
            ui.label(
                RichText::new(format!(
                    "{} : {}",
                    t!(app.locale, "Displayed stock (may be cached)"),
                    product.stock
                ))
                .color(palette.secondary),
            );
        }
        labelled_field(
            ui,
            t!(app.locale, "Counted quantity"),
            &mut app.inventory_form.quantity,
            palette,
            false,
        );
        labelled_field(
            ui,
            t!(app.locale, "Note"),
            &mut app.inventory_form.note,
            palette,
            false,
        );
        let quantity = parse_quantity(&app.inventory_form.quantity);
        let valid = selected.is_some()
            && quantity.is_some()
            && app.inventory_form.note.chars().count() <= 200;
        if primary_button(ui, t!(app.locale, "Record count"), palette, ready && valid) {
            if let (Some(product), Some(quantity)) = (selected, quantity) {
                app.confirm_dialog = Some(Confirm {
                    title: t!(app.locale, "Confirm physical count?").into_owned(),
                    body: format!(
                        "{} · {} : {} → {}. {}",
                        product.name,
                        t!(app.locale, "Displayed stock (may be cached)"),
                        product.stock,
                        quantity,
                        t!(app.locale, "This changes consolidated stock. Concurrent stock movements may affect the final quantity.")
                    ),
                    confirm_label: t!(app.locale, "Record count").into_owned(),
                    danger: true,
                    action: ConfirmAction::CountInventory {
                        product_id: product.id,
                        quantity,
                        note: app.inventory_form.note.clone(),
                    },
                });
            }
        }
    });
}

fn parse_quantity(input: &str) -> Option<f64> {
    let value = input.trim().replace(',', ".").parse::<f64>().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::parse_quantity;
    #[test]
    fn physical_count_requires_finite_non_negative_number() {
        assert_eq!(parse_quantity("12,5"), Some(12.5));
        assert_eq!(parse_quantity("0"), Some(0.0));
        for input in ["", "-1", "NaN", "inf"] {
            assert_eq!(parse_quantity(input), None);
        }
    }
}
