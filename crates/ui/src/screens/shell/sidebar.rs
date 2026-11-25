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
    ui.spacing_mut().item_spacing.y = 4.0;
    ui.add_space(12.0);
    let brand = ui.vertical_centered(|ui| {
        if collapsed {
            crate::branding::mark(ui, palette, 28.0);
        } else {
            crate::branding::lockup(ui, palette, 26.0);
            let stock = app
                .session
                .as_ref()
                .is_some_and(|session| session.role.eq_ignore_ascii_case("stock"));
            ui.label(
                RichText::new(if stock {
                    t!(app.locale, "Stock · warehouse keeper")
                } else {
                    t!(app.locale, "Stock & invoicing")
                })
                .color(palette.secondary)
                .size(10.0),
            );
        }
    });
    if ui
        .interact(
            brand.response.rect,
            egui::Id::new("sidebar-brand-toggle"),
            Sense::click(),
        )
        .double_clicked()
    {
        app.settings.sidebar_collapsed = !collapsed;
        app.save_settings();
    }
    ui.add_space(8.0);
    ui.separator();
    // Navigation counters are shared with the bell. Cache them across rows
    // and rate-limit retries, including after a failed request.
    if app.role_policy().allows_route(Route::Notifications)
        && app.client.is_some()
        && !app.nav_badges.loading
        && app
            .nav_badges
            .stale_for(std::time::Duration::from_secs(300))
    {
        app.load_nav_badges();
    }
    type RailGroup<'a> = (std::borrow::Cow<'a, str>, &'a [(Route, &'a str)]);
    let groups: [RailGroup<'_>; 6] = [
        (
            t!(app.locale, "Overview"),
            &[(Route::Dashboard, "Dashboard")],
        ),
        (
            t!(app.locale, "Catalog"),
            &[(Route::Products, "Products and stock")],
        ),
        (
            t!(app.locale, "Customer relations"),
            &[(Route::Clients, "Clients")],
        ),
        (
            t!(app.locale, "Team"),
            &[
                (Route::Employees, "Employees"),
                (Route::Attendance, "Attendance"),
                (Route::Sales, "Quotes"),
                (Route::Sales, "Quick sale"),
                (Route::Sales, "Sales invoices"),
                (Route::Sales, "Reimbursements"),
                (Route::Sales, "Receipts"),
            ],
        ),
        (
            t!(app.locale, "Purchases"),
            &[
                (Route::Vendors, "Vendors"),
                (Route::Expenses, "Purchase invoices"),
            ],
        ),
        (
            t!(app.locale, "Finance"),
            &[
                (Route::Expenses, "Balance and income"),
                (Route::CashShifts, "Cash shifts"),
                (Route::Payroll, "Payroll CNSS / AMO"),
                (Route::Expenses, "Reports and closing"),
                (Route::InventoryOps, "Stock operations"),
            ],
        ),
    ];
    let navigation_height = ui.available_height().max(0.0);
    egui::ScrollArea::vertical()
        .max_height(navigation_height)
        .show(ui, |ui| {
            for (heading, entries) in groups {
                if !entries
                    .iter()
                    .any(|(route, _)| app.role_policy().allows_route(*route))
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
                for &(route, source) in entries {
                    if !app.role_policy().allows_route(route)
                        || (source == "Reports and closing"
                            && !app.role_policy().can_view_reports())
                    {
                        continue;
                    }
                    let (default_icon, _) = route_meta(app.locale, route);
                    let which = entry_icon(source, default_icon);
                    let label = crate::i18n::gettext(app.locale, source);
                    let selected = active == Some(route)
                        && match (route, source) {
                            (Route::Products, "Scan barcode") => app.product_scanner_open,
                            (Route::Products, "Products and stock") => !app.product_scanner_open,
                            (Route::Sales, "Quotes") => app.quotes_open,
                            (Route::Sales, "Quick sale") => app.quick_sale_open,
                            (Route::Sales, "Receipts") => app.receipts_open,
                            (Route::Sales, "Reimbursements") => app.refunds_open,
                            (Route::Sales, "Sales invoices") => {
                                !app.quotes_open
                                    && !app.receipts_open
                                    && !app.refunds_open
                                    && !app.quick_sale_open
                            }
                            (Route::Sales, _) => false,
                            (Route::Expenses, "Purchase invoices") => app.bills_open,
                            (Route::Expenses, "Balance and income") => {
                                !app.bills_open && !app.finance_report_open
                            }
                            (Route::Expenses, "Reports and closing") => app.finance_report_open,
                            _ => true,
                        };
                    {
                        let response = rail_row(ui, palette, which, &label, selected, collapsed);
                        if response.clicked() {
                            app.go(route);
                            match (route, source) {
                                (Route::Products, "Scan barcode") => {
                                    app.product_scanner_open = true;
                                }
                                (Route::Products, "Products and stock") => {
                                    app.product_scanner_open = false;
                                }
                                (Route::Sales, _) => {
                                    app.invoice_detail_id = None;
                                    app.invoice_detail = Default::default();
                                    app.quotes_open = source == "Quotes";
                                    app.quick_sale_open = source == "Quick sale";
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
                                    app.finance_report_open = source == "Reports and closing"
                                        && app.role_policy().can_view_reports();
                                    if app.bills_open
                                        && app.bills.data.is_none()
                                        && !app.bills.loading
                                    {
                                        app.load_bills();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        });
}

fn entry_icon(source: &str, fallback: Icon) -> Icon {
    match source {
        "Scan barcode" => Icon::QrCode,
        "Quotes" => Icon::SquarePen,
        "Quick sale" => Icon::TrendingUp,
        "Sales invoices" | "Purchase invoices" => Icon::Receipt,
        "Reimbursements" => Icon::Wallet,
        "Receipts" => Icon::CircleCheck,
        "Reports and closing" => Icon::Dashboard,
        _ => fallback,
    }
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
    let height = if collapsed { 44.0 } else { 40.0 };
    // Reserve the row with a passive hover sense first. The icon and label
    // are painted through overlay child UIs whose widgets register their own
    // hover rectangles and used to swallow presses landing on the text; the
    // click interaction below is therefore registered last, so the full row
    // (icon and label text included) is one clickable surface.
    let (slot, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let rect = egui::Rect::from_center_size(
        slot.center(),
        vec2(
            if collapsed {
                44.0
            } else {
                231.0_f32.min(slot.width())
            },
            height,
        ),
    );
    let hovered = ui.rect_contains_pointer(rect);
    if is_active {
        ui.painter().rect_filled(rect, 12.0, palette.surface_active);
        ui.painter().rect_stroke(
            rect,
            12.0,
            egui::Stroke::new(1.0, palette.outline),
            egui::StrokeKind::Inside,
        );
    } else if hovered {
        ui.painter().rect_filled(rect, 12.0, palette.surface_hover);
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
                vec2((rect.width() - 52.0).max(1.0), rect.height()),
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
                        ui.add(
                            egui::Label::new(
                                RichText::new(label)
                                    .color(if is_active {
                                        palette.text
                                    } else {
                                        palette.secondary
                                    })
                                    .font(font),
                            )
                            .truncate(),
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
