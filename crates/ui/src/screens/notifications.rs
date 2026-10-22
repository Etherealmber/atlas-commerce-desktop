//! Notifications: the worker's stock and invoice alerts, with French
//! severity labels. Native toasts for high-severity alerts are raised by the
//! application while alerts refresh in the background.

use egui::RichText;

use crate::icons::Icon;
use crate::state::AtlasApp;
use crate::theme::Palette;
use crate::widgets::{
    card, empty_state, error_banner, icon, page_title, pill, primary_button, secondary_button,
};
use crate::{t, widgets};

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        page_title(ui, t!(app.locale, "Notifications"), palette);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if primary_button(
                ui,
                t!(app.locale, "Refresh"),
                palette,
                !app.alerts.loading,
            ) {
                app.load_alerts();
            }
            if secondary_button(
                ui,
                t!(app.locale, "Open settings"),
                palette,
            ) {
                app.go(crate::state::Route::Settings);
            }
        });
    });
    ui.add_space(12.0);

    if let Some(error) = app.alerts.error.clone() {
        error_banner(ui, &error, palette);
        ui.add_space(10.0);
    }

    if app.alerts.loading && app.alerts.data.is_none() {
        super::spinner(ui, palette);
        return;
    }

    let alerts = app.alerts.data.clone().unwrap_or_default();
    if alerts.is_empty() {
        empty_state(
            ui,
            Icon::Bell,
            t!(app.locale, "No alert"),
            t!(app.locale, "Low stock and overdue invoices appear here."),
            palette,
        );
        return;
    }

    for alert in &alerts {
        card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let which = match alert.severity.as_deref() {
                    Some("high") => Icon::TriangleAlert,
                    Some("medium") => Icon::CircleAlert,
                    _ => Icon::Info,
                };
                let tint = match alert.severity.as_deref() {
                    Some("high") => palette.danger,
                    Some("medium") => palette.warning,
                    _ => palette.accent,
                };
                icon(ui, which, tint, 20.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(alert.label.clone())
                            .color(palette.text)
                            .size(14.0)
                            .strong(),
                    );
                    let detail = alert
                        .count
                        .map(|count| format!("{} · {count}", alert.kind))
                        .unwrap_or_else(|| alert.kind.clone());
                    ui.label(RichText::new(detail).color(palette.dim).size(12.0));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    let label = super::severity_label(alert.severity.as_deref());
                    pill(label.to_owned(), tint.gamma_multiply(0.16), tint).show(ui);
                });
            });
        });
        ui.add_space(8.0);
    }

    if !app.settings.notifications_enabled {
        widgets::info_banner(
            ui,
            t!(
                app.locale,
                "Native notifications are off. Enable them in settings."
            ),
            palette,
        );
    }
}
