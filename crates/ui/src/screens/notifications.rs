//! Notifications: the worker's stock and invoice alerts, with French
//! severity labels. Native toasts for high-severity alerts are raised by the
//! application while alerts refresh in the background.

use egui::RichText;

use crate::icons::Icon;
use crate::state::AtlasApp;
use crate::theme::Palette;
use crate::widgets::{card, empty_state, error_banner, icon, page_title, pill, secondary_button};
use crate::{t, widgets};

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        page_title(ui, t!(app.locale, "Notifications"), palette);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::buttons::primary_refresh_button(
                ui,
                t!(app.locale, "Refresh"),
                palette,
                !app.alerts.loading,
            ) {
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
        super::list_skeleton(ui, palette, app.locale, app.settings.reduced_motion);
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
        // Alert severities are status semantics: the Android finance
        // danger/warning/info tokens, never brand chrome colours.
        let tint = match alert.severity.as_deref() {
            Some("high") => palette.finance_danger,
            Some("medium") => palette.finance_warning,
            _ => palette.finance_info,
        };
        // The worker sends a web `href` (/products, /invoices/…); map the
        // known prefixes onto desktop routes so an alert is one click away.
        let destination = alert
            .href
            .as_deref()
            .and_then(alert_destination)
            .filter(|destination| app.role_policy().allows_route(destination.route()));
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
                    let soft = match alert.severity.as_deref() {
                        Some("high") => palette.finance_danger_soft,
                        Some("medium") => palette.finance_warning_soft,
                        _ => palette.finance_info_soft,
                    };
                    pill(ui, label, soft, tint);
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
        if destination.is_some() {
            response.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        if let Some(destination) = destination
            && clicked
        {
            open_alert(app, destination);
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

/// Only routes emitted by the Worker alerts feed are recognized. A server
/// supplied href never becomes an external navigation or a filesystem path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AlertDestination {
    Products,
    Invoices(Option<i64>),
}

impl AlertDestination {
    pub(super) fn route(self) -> crate::state::Route {
        match self {
            Self::Products => crate::state::Route::Products,
            Self::Invoices(_) => crate::state::Route::Sales,
        }
    }
}

pub(super) fn alert_destination(href: &str) -> Option<AlertDestination> {
    let path = href.split(['?', '#']).next()?;
    match path {
        "/products" | "/products/" => Some(AlertDestination::Products),
        "/invoices" | "/invoices/" => Some(AlertDestination::Invoices(None)),
        _ => {
            let raw = path.strip_prefix("/invoices/")?;
            if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            let id = raw.parse::<i64>().ok().filter(|id| *id > 0)?;
            Some(AlertDestination::Invoices(Some(id)))
        }
    }
}

pub(super) fn open_alert(app: &mut AtlasApp, destination: AlertDestination) {
    let route = destination.route();
    if !app.role_policy().allows_route(route) {
        return;
    }
    match destination {
        AlertDestination::Products => app.stock_history = None,
        AlertDestination::Invoices(_) => {
            app.quotes_open = false;
            app.quick_sale_open = false;
            app.receipts_open = false;
            app.refunds_open = false;
            app.invoice_detail_id = None;
            app.invoice_detail = Default::default();
        }
    }
    app.go(route);
    if let AlertDestination::Invoices(Some(id)) = destination {
        app.open_invoice_detail(id);
    }
}

#[cfg(test)]
mod tests {
    use super::{AlertDestination, alert_destination};

    #[test]
    fn parses_only_known_internal_alert_links() {
        assert_eq!(
            alert_destination("/products"),
            Some(AlertDestination::Products)
        );
        assert_eq!(
            alert_destination("/invoices/42?source=alerts"),
            Some(AlertDestination::Invoices(Some(42)))
        );
        assert_eq!(
            alert_destination("/invoices"),
            Some(AlertDestination::Invoices(None))
        );
        for href in [
            "https://evil.test/products",
            "//evil.test/invoices/3",
            "/products-other",
            "/invoices/0",
            "/invoices/-2",
            "/invoices/2/other",
            "/invoices/99999999999999999999999",
        ] {
            assert_eq!(alert_destination(href), None);
        }
    }
}
