//! Adaptive navigation rail and its grouped route entries.
use super::route_meta;
use crate::icons::Icon;
use crate::state::{AtlasApp, Route, Screen};
use crate::theme::Palette;
use crate::widgets::icon;
use crate::{t, widgets};
use egui::{RichText, Sense, Vec2, vec2};

pub(super) fn draw_rail(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette, collapsed: bool) {
    let active = match app.screen {
        Screen::Shell(route) => Some(route),
        _ => None,
    };
    // Leave the native macOS traffic lights unobstructed in the top-left.
    #[cfg(target_os = "macos")]
    if fastframe_titlebar::enabled() {
        ui.add_space(super::TITLEBAR_HEIGHT);
    }
    ui.add_space(8.0);
    ui.vertical_centered(|ui| {
        if collapsed {
            crate::branding::mark(ui, palette, 24.0);
        } else {
            crate::branding::lockup(ui, palette, 26.0);
            ui.label(
                RichText::new(t!(app.locale, "Stock & invoicing"))
                    .color(palette.secondary)
                    .size(10.0),
            );
        }
    });
    ui.add_space(8.0);
    ui.separator();
    // Only implemented destinations are actionable. The remaining entries
    // are visible but explicitly unavailable; they never issue API requests.
    type RailGroup<'a> = (std::borrow::Cow<'a, str>, &'a [(Route, &'a str, bool)]);
    let groups: [RailGroup<'_>; 6] = [
        (
            t!(app.locale, "Overview"),
            &[(Route::Dashboard, "Dashboard", true)],
        ),
        (
            t!(app.locale, "Catalog"),
            &[(Route::Products, "Products and stock", true)],
        ),
        (
            t!(app.locale, "Customer relations"),
            &[(Route::Clients, "Clients", true)],
        ),
        (
            t!(app.locale, "Team"),
            &[
                (Route::Employees, "Employees", true),
                (Route::Employees, "Attendance", false),
                (Route::Sales, "Quotes", true),
                (Route::Sales, "Quick sale", true),
                (Route::Sales, "Sales invoices", true),
                (Route::Sales, "Reimbursements", true),
                (Route::Sales, "Receipts", true),
            ],
        ),
        (
            t!(app.locale, "Purchases"),
            &[
                (Route::Vendors, "Vendors", true),
                (Route::Expenses, "Purchase invoices", true),
            ],
        ),
        (
            t!(app.locale, "Finance"),
            &[
                (Route::Expenses, "Balance and income", false),
                (Route::Expenses, "Reports and closing", false),
                (Route::Products, "Stock operations", false),
                (Route::Expenses, "Expenses", true),
                (Route::DeliveryNotes, "Delivery notes", true),
            ],
        ),
    ];
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (heading, entries) in groups {
            if !entries
                .iter()
                .any(|(route, _, _)| app.role_policy().allows_route(*route))
            {
                continue;
            }
            if !collapsed {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(heading.to_uppercase())
                        .color(palette.dim)
                        .size(10.5),
                );
            }
            for &(route, source, implemented) in entries {
                if !app.role_policy().allows_route(route) {
                    continue;
                }
                let (which, _) = route_meta(app.locale, route);
                let label = crate::i18n::gettext(app.locale, source);
                let selected = active == Some(route)
                    && match (route, source) {
                        (Route::Sales, "Quotes") => app.quotes_open,
                        (Route::Sales, "Receipts") => app.receipts_open,
                        (Route::Sales, "Reimbursements") => app.refunds_open,
                        (Route::Sales, "Sales invoices") => {
                            !app.quotes_open && !app.receipts_open && !app.refunds_open
                        }
                        (Route::Sales, _) => false,
                        (Route::Expenses, "Purchase invoices") => app.bills_open,
                        (Route::Expenses, "Expenses") => !app.bills_open,
                        _ => true,
                    };
                if implemented {
                    let response = rail_row(ui, palette, which, &label, selected, collapsed);
                    if response.clicked() {
                        app.go(route);
                        match (route, source) {
                            (Route::Sales, _) => {
                                app.quotes_open = source == "Quotes";
                                app.receipts_open = source == "Receipts";
                                app.refunds_open = source == "Reimbursements";
                                if app.receipts_open
                                    && app.receipts.data.is_none()
                                    && !app.receipts.loading
                                {
                                    app.load_receipts();
                                }
                            }
                            (Route::Expenses, _) => {
                                app.bills_open = source == "Purchase invoices";
                                if app.bills_open && app.bills.data.is_none() && !app.bills.loading
                                {
                                    app.load_bills();
                                }
                            }
                            _ => {}
                        }
                    }
                } else {
                    let unavailable =
                        format!("{} · {}", label, t!(app.locale, "Not available yet"));
                    ui.add_enabled_ui(false, |ui| {
                        rail_row(ui, palette, which, &unavailable, false, collapsed)
                    })
                    .inner
                    .on_hover_text(t!(app.locale, "Not available yet"));
                }
            }
        }
    });
}

fn rail_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    which: Icon,
    label: &str,
    is_active: bool,
    collapsed: bool,
) -> egui::Response {
    // Web sidebar items are 40 px rows; the desktop parity floor is 44 px
    // touch targets.
    let height = 44.0;
    // Reserve the row with a passive hover sense first. The icon and label
    // are painted through overlay child UIs whose widgets register their own
    // hover rectangles and used to swallow presses landing on the text; the
    // click interaction below is therefore registered last, so the full row
    // (icon and label text included) is one clickable surface.
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let hovered = ui.rect_contains_pointer(rect);
    if is_active {
        ui.painter().rect_filled(rect, 4.0, palette.surface_active);
        // The accent stripe belongs to the labelled rail only; the 72 px
        // collapsed strip communicates selection through tint + surface.
        if !collapsed {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(rect.left() + 2.0, rect.center().y - 10.0),
                    vec2(3.0, 20.0),
                ),
                2.0,
                palette.accent,
            );
        }
    } else if hovered {
        ui.painter().rect_filled(rect, 4.0, palette.surface_hover);
    }
    let tint = if is_active {
        palette.accent
    } else {
        palette.secondary
    };
    if collapsed {
        // Icon-only strip: centre the glyph in the 44 px row and expose the
        // route name as a tooltip for the collapsed navigation.
        widgets::overlay_at(
            ui,
            egui::Rect::from_center_size(rect.center(), Vec2::splat(20.0)),
            |ui| icon(ui, which, tint, 19.0),
        );
    } else {
        widgets::overlay_at(
            ui,
            egui::Rect::from_center_size(
                egui::pos2(rect.left() + 22.0, rect.center().y),
                Vec2::splat(20.0),
            ),
            |ui| icon(ui, which, tint, 19.0),
        );
        widgets::overlay_at(
            ui,
            egui::Rect::from_min_size(
                egui::pos2(rect.left() + 44.0, rect.top()),
                vec2(rect.width() - 52.0, rect.height()),
            ),
            |ui| {
                // Rail entries are one-line chrome labels, even at narrow widths.
                widgets::no_wrap_scope(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        let font = if is_active {
                            crate::fonts::semibold(13.0)
                        } else {
                            crate::fonts::medium(13.0)
                        };
                        ui.label(
                            RichText::new(label)
                                .color(if is_active {
                                    palette.text
                                } else {
                                    palette.secondary
                                })
                                .font(font),
                        );
                    });
                });
            },
        );
    }
    // Last interaction wins for overlapping rectangles, so registering the
    // row click after the overlay widgets keeps the label text clickable.
    let response = ui.interact(rect, egui::Id::new(("rail-row", label)), Sense::click());
    if collapsed {
        response.on_hover_text(label)
    } else {
        response
    }
}
