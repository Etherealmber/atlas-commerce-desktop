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
            if primary_button(ui, t!(app.locale, "Refresh"), palette, !app.alerts.loading) {
                app.load_alerts();
            }
            if secondary_button(ui, t!(app.locale, "Open settings"), palette) {
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
        // The worker sends a web `href` (/products, /invoices/…); map the
        // known prefixes onto desktop routes so an alert is one click away.
        let route = alert.href.as_deref().and_then(alert_route);
        let inner = card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                icon(ui, which, tint, 20.0);
                ui.add_space(4.0);
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
                    pill(ui, label, tint.gamma_multiply(0.16), tint);
                });
            });
            // Thin severity rail on the card's left edge.
            let rect = ui.min_rect();
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(rect.left() + 1.0, rect.top() + 8.0),
                    egui::vec2(3.0, (rect.height() - 16.0).max(0.0)),
                ),
                2.0,
                tint,
            );
        });
        let response = inner.response.interact(egui::Sense::click());
        // `on_hover_cursor` consumes the response, so capture the click
        // state before handing it over.
        let clicked = response.clicked();
        if route.is_some() {
            response.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        if let Some(route) = route
            && clicked
        {
            app.go(route);
            if route == crate::state::Route::Sales && app.invoices.data.is_none() {
                app.load_invoices();
            }
            if route == crate::state::Route::Products && app.products.data.is_none() {
                app.load_products();
            }
        }
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

/// Maps a worker alert `href` prefix to a desktop route, when known.
fn alert_route(href: &str) -> Option<crate::state::Route> {
    use crate::state::Route;
    if href.starts_with("/products") {
        Some(Route::Products)
    } else if href.starts_with("/invoices") {
        Some(Route::Sales)
    } else {
        None
    }
}
