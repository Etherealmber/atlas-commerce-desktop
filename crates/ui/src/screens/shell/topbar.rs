//! Title bar, search, profile and notification chrome.
use super::{TITLEBAR_HEIGHT, route_meta};
use crate::dialogs::{Confirm, ConfirmAction};
use crate::icons::Icon;
use crate::state::{AtlasApp, Route, Screen};
use crate::theme::Palette;
use crate::{t, widgets};
use egui::{Align, Layout, RichText, Sense, Vec2, vec2};

const PROFILE_MENU_WIDTH: f32 = 296.0;

#[derive(Clone, Copy, Default)]
struct DockScroll {
    offset: f32,
    travel: f32,
    compact: bool,
    expanded_until: f64,
}

fn route_scroll_id(route: Route) -> egui::Id {
    egui::Id::new(("shell-dock-scroll", format!("{route:?}")))
}

fn update_dock_scroll(mut state: DockScroll, offset: f32, locked: bool, now: f64) -> DockScroll {
    let offset = offset.max(0.0);
    let delta = offset - state.offset;
    state.offset = offset;
    if offset <= 60.0 {
        state.travel = 0.0;
        state.compact = false;
        return state;
    }
    if locked || now < state.expanded_until {
        state.travel = 0.0;
        return state;
    }
    if delta > 0.0 {
        state.travel = if state.travel > 0.0 {
            state.travel + delta
        } else {
            delta
        };
        if offset > 140.0 && state.travel >= 48.0 {
            state.compact = true;
            state.travel = 0.0;
        }
    } else if delta < 0.0 {
        state.travel = if state.travel < 0.0 {
            state.travel + delta
        } else {
            delta
        };
        if state.compact && state.travel <= -48.0 {
            state.compact = false;
            state.travel = 0.0;
        }
    }
    state
}

pub(super) fn record_scroll(app: &AtlasApp, ctx: &egui::Context, offset: f32) {
    let Screen::Shell(route) = app.screen else {
        return;
    };
    let id = route_scroll_id(route);
    let menu_open = app.profile_menu_open
        || ctx.data(|d| {
            d.get_temp::<bool>(egui::Id::new("notification-popover-open"))
                .unwrap_or(false)
        });
    let (now, pointer) = ctx.input(|input| (input.time, input.pointer.hover_pos()));
    let hover = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("dock-last-rect")))
        .is_some_and(|rect| pointer.is_some_and(|pos| rect.contains(pos)));
    ctx.data_mut(|d| {
        let previous = d.get_temp::<DockScroll>(id).unwrap_or_default();
        d.insert_temp(
            id,
            update_dock_scroll(previous, offset, menu_open || hover, now),
        );
    });
}

/// Shared source for both bell and header. Follow the web's four badge
/// categories and at most six extra alert kinds, avoiding double counting.
fn notification_total(
    badges: Option<&atlas_api::models::NavBadges>,
    alerts: &[atlas_api::models::Alert],
) -> i64 {
    let defaults = atlas_api::models::NavBadges::default();
    let badges = badges.unwrap_or(&defaults);
    let extra = alerts
        .iter()
        .filter(|alert| {
            let kind = alert.kind.to_ascii_lowercase();
            !((kind == "mobile_actions" && badges.actions_pending > 0)
                || (kind == "low_stock" && badges.products_low > 0)
                || (kind == "unpaid_invoices" && badges.invoices_unpaid > 0)
                || (kind == "pay_due" && badges.employees_pay_due > 0))
        })
        .take(6)
        .count() as i64;
    badges
        .actions_pending
        .max(0)
        .saturating_add(badges.employees_pay_due.max(0))
        .saturating_add(badges.invoices_unpaid.max(0))
        .saturating_add(badges.products_low.max(0))
        .saturating_add(extra)
}

fn app_notification_total(app: &AtlasApp) -> i64 {
    notification_total(
        app.nav_badges.data.as_ref(),
        app.alerts.data.as_deref().unwrap_or(&[]),
    )
}

fn detail_back_available(app: &AtlasApp, route: Route) -> bool {
    match route {
        Route::Products => app
            .stock_history
            .as_ref()
            .is_some_and(|history| history.full),
        Route::Sales => app.invoice_detail_id.is_some() && !app.quick_sale_open,
        Route::Clients => app.client_report_id.is_some(),
        Route::Employees => app.employee_situation_id.is_some(),
        Route::Expenses => app.finance_report_open || app.bills_open,
        _ => false,
    }
}

fn back_to_parent(app: &mut AtlasApp, route: Route) {
    match route {
        Route::Products if detail_back_available(app, route) => app.stock_history = None,
        Route::Sales if detail_back_available(app, route) => {
            app.invoice_detail_id = None;
            app.invoice_detail = Default::default();
        }
        Route::Clients if detail_back_available(app, route) => {
            app.client_report_id = None;
            app.client_report = Default::default();
        }
        Route::Employees if detail_back_available(app, route) => {
            app.employee_situation_id = None;
            app.employee_situation = Default::default();
        }
        Route::Expenses if detail_back_available(app, route) => {
            app.finance_report_open = false;
            app.bills_open = false;
        }
        _ => {
            app.go_back();
        }
    }
}

fn dock_title(app: &AtlasApp, route: Route) -> String {
    let source = match route {
        Route::Products if app.stock_history.as_ref().is_some_and(|h| h.full) => {
            Some("Stock history")
        }
        Route::Sales if app.quick_sale_open => Some("Quick sale"),
        Route::Sales if app.quotes_open => Some("Quotes"),
        Route::Sales if app.receipts_open => Some("Receipts"),
        Route::Sales if app.refunds_open => Some("Reimbursements"),
        Route::Sales => Some("Sales invoices"),
        Route::Expenses if app.finance_report_open => Some("Reports and closing"),
        Route::Expenses if app.bills_open => Some("Purchase invoices"),
        _ => None,
    };
    source.map_or_else(
        || route_meta(app.locale, route).1,
        |label| crate::i18n::gettext(app.locale, label).into_owned(),
    )
}

pub(super) fn draw_titlebar(
    app: &mut AtlasApp,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    palette: &Palette,
) {
    let rect = ui.available_rect_before_wrap();
    ui.allocate_exact_size(rect.size(), Sense::hover());
    if fastframe_titlebar::enabled() {
        fastframe_titlebar::drag(ui, rect);
    }
    let left = rect.left() + 8.0;
    let right = rect.right().max(left);
    let available = (right - left).max(1.0);
    let route = match app.screen {
        Screen::Shell(route) => route,
        _ => Route::Dashboard,
    };
    let title = dock_title(app, route)
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ");
    let title_width = ui
        .painter()
        .layout_no_wrap(title.clone(), crate::fonts::semibold(14.4), palette.text)
        .size()
        .x
        .min(180.0);
    let notifications_open = ctx.data(|d| {
        d.get_temp::<bool>(egui::Id::new("notification-popover-open"))
            .unwrap_or(false)
    });
    let hover = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("dock-last-rect")))
        .is_some_and(|last| {
            ctx.input(|i| i.pointer.hover_pos())
                .is_some_and(|pos| last.contains(pos))
        });
    let scroll = ctx
        .data(|d| d.get_temp::<DockScroll>(route_scroll_id(route)))
        .unwrap_or_default();
    let focus_ids = ctx
        .data(|d| d.get_temp::<Vec<egui::Id>>(egui::Id::new("dock-focus-ids")))
        .unwrap_or_default();
    let focused = ctx.memory(|memory| memory.focused().is_some_and(|id| focus_ids.contains(&id)));
    let compact =
        scroll.compact && !hover && !focused && !app.profile_menu_open && !notifications_open;
    let t = if app.settings.reduced_motion {
        if compact { 1.0 } else { 0.0 }
    } else {
        ctx.animate_bool_with_time_and_easing(
            egui::Id::new("dock-compact-animation"),
            compact,
            crate::motion::duration(crate::motion::FAST, false),
            crate::motion::standard,
        )
    };
    let show_extras = available >= 330.0;
    let show_back = (available >= 280.0 && detail_back_available(app, route))
        || (available >= 430.0 && !app.nav_back.is_empty());
    let right_width = if show_extras { 136.0 } else { 40.0 };
    let back_width = if show_back { 48.0 } else { 0.0 };
    let title_space = title_width + 40.0;
    let full_width = (22.0 + title_space + 12.0 + right_width + back_width).min(available);
    let compact_width = (title_space + 22.0).min(available).max(1.0);
    let width = full_width + (compact_width - full_width) * t;
    let dock = egui::Rect::from_center_size(
        egui::pos2((left + right) * 0.5, rect.center().y),
        vec2(width, 58.0),
    );
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("dock-last-rect"), dock));
    ui.painter().rect_filled(
        dock.translate(vec2(0.0, 2.0)),
        255.0,
        egui::Color32::from_black_alpha(if palette.dark { 35 } else { 16 }),
    );
    ui.painter().rect_filled(
        dock,
        255.0,
        if palette.dark {
            egui::Color32::from_rgb(20, 20, 23)
        } else {
            egui::Color32::WHITE
        },
    );
    ui.painter().rect_stroke(
        dock,
        255.0,
        egui::Stroke::new(1.0, palette.outline),
        egui::StrokeKind::Inside,
    );
    let mut avatar_rect = egui::Rect::NOTHING;
    let mut bell_rect = egui::Rect::NOTHING;
    let mut focus_ids = Vec::new();
    widgets::overlay_at(ui, dock.shrink2(vec2(11.0, 0.0)), |ui| {
        ui.set_clip_rect(ui.clip_rect().intersect(dock));
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if t < 0.25 && show_back {
                let back =
                    dock_icon(ui, palette, Icon::ArrowLeft).on_hover_text(t!(app.locale, "Back"));
                focus_ids.push(back.id);
                if back.clicked() {
                    back_to_parent(app, route);
                }
            }
            let title_button = egui::Button::new(
                RichText::new(title)
                    .font(crate::fonts::semibold(14.4))
                    .color(palette.text),
            )
            .frame(false)
            .min_size(vec2(title_space, 44.0));
            let title_response = ui.add_sized(vec2(title_space, 44.0), title_button);
            focus_ids.push(title_response.id);
            if title_response.clicked() {
                let now = ctx.input(|input| input.time);
                ctx.data_mut(|d| {
                    let id = route_scroll_id(route);
                    let mut state = d.get_temp::<DockScroll>(id).unwrap_or_default();
                    state.compact = false;
                    state.travel = 0.0;
                    state.expanded_until = now + 0.4;
                    d.insert_temp(id, state);
                });
            }
            if t < 0.25 {
                ui.add_space(4.0);
                if show_extras {
                    let theme = dock_icon(
                        ui,
                        palette,
                        if palette.dark { Icon::Sun } else { Icon::Moon },
                    )
                    .on_hover_text(t!(app.locale, "Toggle theme"));
                    focus_ids.push(theme.id);
                    if theme.clicked() {
                        app.settings.theme = if palette.dark {
                            atlas_core::settings::ThemeMode::Light
                        } else {
                            atlas_core::settings::ThemeMode::Dark
                        };
                        app.save_settings();
                        ctx.request_repaint();
                    }
                    if app.role_policy().allows_route(Route::Notifications) {
                        let bell = dock_icon(ui, palette, Icon::Bell);
                        focus_ids.push(bell.id);
                        bell_rect = bell.rect;
                        let count = app_notification_total(app);
                        if count > 0 {
                            paint_titlebar_badge(ui, &bell, count);
                        }
                        if bell.clicked() {
                            let id = egui::Id::new("notification-popover-open");
                            let open = ctx.data_mut(|d| {
                                let next = !d.get_temp::<bool>(id).unwrap_or(false);
                                d.insert_temp(id, next);
                                next
                            });
                            app.profile_menu_open = false;
                            if open {
                                let interval = std::time::Duration::from_secs(
                                    app.settings.poll_interval_secs.max(15),
                                );
                                if !app.alerts.loading && app.alerts.stale_for(interval) {
                                    app.load_alerts();
                                }
                                if !app.nav_badges.loading && app.nav_badges.stale_for(interval) {
                                    app.load_nav_badges();
                                }
                            }
                        }
                    }
                }
                let initial = app
                    .session
                    .as_ref()
                    .and_then(|s| {
                        if s.display_name.is_empty() {
                            s.username.chars().next()
                        } else {
                            s.display_name.chars().next()
                        }
                    })
                    .unwrap_or('A')
                    .to_uppercase()
                    .collect::<String>();
                let avatar = widgets::avatar(
                    ui,
                    app.avatar_texture_uri.as_deref(),
                    &initial,
                    40.0,
                    palette.accent,
                    palette.on_accent,
                    palette.outline,
                );
                focus_ids.push(avatar.id);
                avatar_rect = avatar.rect;
                if avatar.clicked() {
                    app.profile_menu_open = !app.profile_menu_open;
                    ctx.data_mut(|d| {
                        d.insert_temp(egui::Id::new("notification-popover-open"), false)
                    });
                }
            }
        });
    });
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("dock-focus-ids"), focus_ids));
    if app.profile_menu_open && avatar_rect != egui::Rect::NOTHING {
        draw_profile_menu(app, ctx, palette, &avatar_rect);
    }
    if notifications_open && bell_rect != egui::Rect::NOTHING {
        draw_notifications_popover(app, ctx, palette, bell_rect);
    }
    if fastframe_titlebar::enabled() {
        let style = fastframe_titlebar::Style {
            icon: if cfg!(target_os = "linux") {
                palette.text
            } else {
                palette.secondary
            },
            button: palette.surface_hover,
            header_height: TITLEBAR_HEIGHT,
        };
        let minimize = t!(app.locale, "Minimize").into_owned();
        let maximize = t!(app.locale, "Maximize").into_owned();
        let restore = t!(app.locale, "Restore").into_owned();
        let close = t!(app.locale, "Close").into_owned();
        let labels = fastframe_titlebar::Labels {
            minimize: &minimize,
            maximize: &maximize,
            restore: &restore,
            close: &close,
        };
        widgets::overlay_at(
            ui,
            egui::Rect::from_min_size(
                egui::pos2(rect.right() - footprint.right.max(120.0), rect.top()),
                vec2(footprint.right.max(120.0), TITLEBAR_HEIGHT),
            ),
            |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    fastframe_titlebar::controls(ui, &style, &labels);
                });
            },
        );
    }
}

/// Recognize page shortcuts.
#[cfg(test)]
pub(super) fn search_destination(query: &str) -> Option<Route> {
    match crate::shortcuts::normalize(query.trim()).as_str() {
        "facture" | "factures" | "invoice" | "invoices" | "ventes" | "sales" => Some(Route::Sales),
        "situation" | "client" | "clients" => Some(Route::Clients),
        "produit" | "produits" | "products" => Some(Route::Products),
        "depense" | "depenses" | "expenses" => Some(Route::Expenses),
        "employe" | "employes" | "employees" | "equipe" | "rh" => Some(Route::Employees),
        "agent" => Some(Route::Agent),
        _ => None,
    }
}

fn dock_icon(ui: &mut egui::Ui, palette: &Palette, which: Icon) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::click());
    let fill = if response.hovered() {
        palette.surface_hover
    } else if palette.dark {
        egui::Color32::from_rgb(32, 32, 36)
    } else {
        egui::Color32::WHITE
    };
    ui.painter().rect_filled(rect, 255.0, fill);
    ui.painter().rect_stroke(
        rect,
        255.0,
        egui::Stroke::new(1.0, palette.outline),
        egui::StrokeKind::Inside,
    );
    widgets::overlay_at(
        ui,
        egui::Rect::from_center_size(rect.center(), Vec2::splat(18.0)),
        |ui| {
            widgets::icon(ui, which, palette.secondary, 18.0);
        },
    );
    response
}

/// Small accent counter at a title-bar button's top-right corner.
fn paint_titlebar_badge(ui: &mut egui::Ui, button: &egui::Response, count: i64) {
    let text = if count > 99 {
        "99+".to_owned()
    } else {
        count.to_string()
    };
    let galley =
        ui.painter()
            .layout_no_wrap(text, egui::FontId::proportional(10.5), egui::Color32::WHITE);
    let width = (galley.size().x + 8.0).max(15.0);
    let rect = egui::Rect::from_min_size(
        egui::pos2(button.rect.right() - width + 2.0, button.rect.top() - 2.0),
        vec2(width, 15.0),
    );
    ui.painter()
        .rect_filled(rect, 999.0, egui::Color32::from_rgb(239, 68, 68));
    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        egui::Color32::WHITE,
    );
}

/// Fetch alerts only when opened, never as a background fan-out.
/// Map only known Worker alert kinds; unknown kinds remain neutral.
fn notification_tile(kind: &str, palette: &Palette) -> (Icon, egui::Color32, egui::Color32) {
    match kind {
        "rupture" | "stock_bas" | "lot_expire" | "lot_bientot" => (
            Icon::Boxes,
            palette.finance_warning,
            palette.finance_warning_soft,
        ),
        "demande_produit" => (
            Icon::Package,
            palette.finance_info,
            palette.finance_info_soft,
        ),
        "facture_echue" => (
            Icon::Receipt,
            palette.finance_danger,
            palette.finance_danger_soft,
        ),
        _ => (Icon::Bell, palette.secondary, palette.surface_hover),
    }
}

fn draw_notifications_popover(
    app: &mut AtlasApp,
    ctx: &egui::Context,
    palette: &Palette,
    anchor: egui::Rect,
) {
    let id = egui::Id::new("notification-popover-open");
    let area = egui::Area::new(egui::Id::new("notification-popover"))
        .fixed_pos(egui::pos2(anchor.right() - 336.0, anchor.bottom() + 8.0))
        .order(egui::Order::Foreground)
        .constrain(true)
        .show(ctx, |ui| {
            widgets::pop_card(palette)
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.set_width(308.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(t!(app.locale, "Notifications"))
                                .color(palette.text)
                                .strong()
                                .size(15.0),
                        );
                        let count = app_notification_total(app);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            widgets::pill(
                                ui,
                                format!("{count} alertes"),
                                palette.surface_hover,
                                palette.secondary,
                            );
                        });
                    });
                    ui.separator();
                    if app.alerts.loading && app.alerts.data.is_none() {
                        super::super::centered_spinner(ui, palette);
                    } else if let Some(error) = &app.alerts.error {
                        widgets::error_banner(ui, error, palette);
                        if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette) {
                            app.load_alerts();
                        }
                    } else if let Some(alerts) = app.alerts.data.clone() {
                        if alerts.is_empty() {
                            ui.add_space(20.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(t!(app.locale, "All caught up"))
                                        .color(palette.secondary),
                                );
                            });
                            ui.add_space(20.0);
                        } else {
                            for alert in alerts.iter().take(4) {
                                let destination = alert
                                    .href
                                    .as_deref()
                                    .and_then(super::super::notifications::alert_destination)
                                    .filter(|target| {
                                        app.role_policy().allows_route(target.route())
                                    });
                                let (tile_icon, tint, fill) =
                                    notification_tile(&alert.kind, palette);
                                let row = ui.horizontal_centered(|ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.set_min_height(48.0);
                                    let (tile, _) =
                                        ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                                    ui.painter().rect_filled(tile, 9.0, fill);
                                    ui.put(tile.shrink(7.0), tile_icon.image(tint, 18.0));
                                    let label_width = (ui.available_width() - 24.0).max(1.0);
                                    ui.allocate_ui_with_layout(
                                        vec2(label_width, 32.0),
                                        Layout::top_down(Align::Min),
                                        |ui| {
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(&alert.label)
                                                        .color(palette.text)
                                                        .size(13.0),
                                                )
                                                .wrap(),
                                            );
                                            if let Some(count) = alert.count {
                                                ui.label(
                                                    RichText::new(count.to_string())
                                                        .color(palette.secondary)
                                                        .size(11.0),
                                                );
                                            }
                                        },
                                    );
                                    if destination.is_some() {
                                        widgets::icon(ui, Icon::ChevronRight, palette.dim, 16.0);
                                    }
                                });
                                let response = row.response.interact(Sense::click());
                                let clicked = response.clicked();
                                if destination.is_some() {
                                    response.on_hover_cursor(egui::CursorIcon::PointingHand);
                                }
                                if let Some(target) = destination
                                    && clicked
                                {
                                    ctx.data_mut(|data| data.insert_temp(id, false));
                                    super::super::notifications::open_alert(app, target);
                                }
                                ui.separator();
                            }
                        }
                    }
                    ui.vertical_centered(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(t!(app.locale, "View all"))
                                        .color(palette.text)
                                        .strong(),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            ctx.data_mut(|data| data.insert_temp(id, false));
                            app.go(Route::Notifications);
                        }
                    });
                });
        });
    if ctx.input(|input| input.pointer.any_pressed())
        && !area.response.rect.contains(
            ctx.input(|input| input.pointer.interact_pos())
                .unwrap_or_default(),
        )
        && !anchor.contains(
            ctx.input(|input| input.pointer.interact_pos())
                .unwrap_or_default(),
        )
    {
        ctx.data_mut(|data| data.insert_temp(id, false));
    }
}

/// Account popover anchored under the title-bar avatar.
fn draw_profile_menu(
    app: &mut AtlasApp,
    ctx: &egui::Context,
    palette: &Palette,
    avatar_rect: &egui::Rect,
) {
    let position = egui::pos2(
        avatar_rect.right() - PROFILE_MENU_WIDTH,
        avatar_rect.bottom() + 8.0,
    );
    let response = egui::Area::new(egui::Id::new("titlebar-profile-menu"))
        .fixed_pos(position)
        .order(egui::Order::Foreground)
        .constrain(true)
        .show(ctx, |ui| {
            widgets::pop_card(palette)
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(PROFILE_MENU_WIDTH - 20.0);
                    if let Some(session) = app.session.clone() {
                        ui.horizontal(|ui| {
                            ui.add_space(2.0);
                            let initial = if session.display_name.is_empty() {
                                session.username.chars().next().unwrap_or('A')
                            } else {
                                session.display_name.chars().next().unwrap_or('A')
                            }
                            .to_string();
                            widgets::avatar(
                                ui,
                                app.avatar_texture_uri.as_deref(),
                                &initial,
                                34.0,
                                palette.accent,
                                palette.on_accent,
                                palette.outline,
                            );
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(if session.display_name.is_empty() {
                                        session.username.clone()
                                    } else {
                                        session.display_name.clone()
                                    })
                                    .color(palette.text)
                                    .strong()
                                    .size(13.5),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {}",
                                        super::super::settings::role_label(app),
                                        atlas_core::ServerUrl::host(&session.server_url)
                                    ))
                                    .color(palette.dim)
                                    .size(11.5),
                                );
                            });
                        });
                        ui.separator();
                    }

                    if menu_row(ui, palette, Icon::User, t!(app.locale, "My profile")).clicked() {
                        app.profile_menu_open = false;
                        app.settings_focus = crate::state::SettingsFocus::Profile;
                        app.open_settings();
                    }
                    let settings_click =
                        menu_row(ui, palette, Icon::Settings, t!(app.locale, "Settings")).clicked();
                    if settings_click {
                        app.profile_menu_open = false;
                        app.settings_focus = crate::state::SettingsFocus::General;
                        app.open_settings();
                    }
                    // The server is fixed by the installer and read-only in
                    // Settings; only a reinstall can point elsewhere.
                    if !app.server_store.is_locked()
                        && menu_row(ui, palette, Icon::Server, t!(app.locale, "Change server"))
                            .clicked()
                    {
                        app.profile_menu_open = false;
                        app.sign_out();
                        app.server_form.verified_selected = None;
                        app.screen = Screen::ServerSetup;
                    }
                    // Release notes are shown in the application; a browser
                    // link remains available in the settings section.
                    let whats_new = menu_row(
                        ui,
                        palette,
                        Icon::ExternalLink,
                        t!(app.locale, "What's new and updates"),
                    )
                    .clicked();
                    if whats_new {
                        app.profile_menu_open = false;
                        app.settings_focus = crate::state::SettingsFocus::About;
                        app.open_settings();
                        if app.update.release_notes.is_none() && !app.update.checking {
                            app.check_for_updates(false);
                        }
                    }
                    let about =
                        menu_row(ui, palette, Icon::Info, t!(app.locale, "About")).clicked();
                    if about {
                        app.profile_menu_open = false;
                        app.settings_focus = crate::state::SettingsFocus::About;
                        app.open_settings();
                    }
                    ui.separator();
                    if menu_row(ui, palette, Icon::LogOut, t!(app.locale, "Sign out")).clicked() {
                        app.profile_menu_open = false;
                        app.confirm_dialog = Some(Confirm {
                            title: t!(app.locale, "Sign out").into_owned(),
                            body: t!(
                                app.locale,
                                "You will need to sign in again to reach this server."
                            )
                            .into_owned(),
                            confirm_label: t!(app.locale, "Sign out").into_owned(),
                            danger: true,
                            action: ConfirmAction::SignOut,
                        });
                    }
                });
        })
        .response;

    // Click-outside dismissal. The avatar owns its own toggle, so a press
    // landing on the avatar or inside the popover must not flip the state.
    let pressed_outside = ctx.input(|input| {
        input.pointer.any_pressed()
            && input
                .pointer
                .interact_pos()
                .is_some_and(|pos| !response.rect.contains(pos) && !avatar_rect.contains(pos))
    });
    if pressed_outside {
        app.profile_menu_open = false;
    }
}

fn menu_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    which: Icon,
    label: impl AsRef<str>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), crate::theme::TARGET_MIN_HEIGHT),
        Sense::click(),
    );
    if response.hovered() {
        ui.painter().rect_filled(rect, 8.0, palette.surface_hover);
    }
    widgets::overlay_at(
        ui,
        egui::Rect::from_center_size(
            egui::pos2(rect.left() + 18.0, rect.center().y),
            Vec2::splat(18.0),
        ),
        |ui| widgets::icon(ui, which, palette.secondary, 16.0),
    );
    widgets::overlay_at(
        ui,
        egui::Rect::from_min_size(
            egui::pos2(rect.left() + 38.0, rect.top()),
            vec2(rect.width() - 44.0, rect.height()),
        ),
        |ui| {
            widgets::no_wrap_scope(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new(label.as_ref())
                            .color(palette.text)
                            .font(crate::fonts::medium(13.5)),
                    );
                });
            });
        },
    );
    response
}

// ── Left rail ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod dock_tests {
    use super::{DockScroll, notification_total, update_dock_scroll};
    use atlas_api::models::{Alert, NavBadges};

    #[test]
    fn compact_requires_directional_travel_and_top_or_upward_travel_expands() {
        let state = update_dock_scroll(DockScroll::default(), 141.0, false, 0.0);
        assert!(state.compact);
        let state = update_dock_scroll(state, 140.0, false, 0.1);
        assert!(state.compact);
        let state = update_dock_scroll(state, 94.0, false, 0.2);
        assert!(state.compact);
        let state = update_dock_scroll(state, 93.0, false, 0.3);
        assert!(!state.compact);
        let state = update_dock_scroll(state, 200.0, false, 0.4);
        assert!(state.compact);
        assert!(!update_dock_scroll(state, 60.0, false, 0.5).compact);
    }

    #[test]
    fn lock_and_manual_expansion_suppress_downward_compaction() {
        let state = update_dock_scroll(DockScroll::default(), 160.0, true, 0.0);
        assert!(!state.compact);
        let state = DockScroll {
            expanded_until: 1.0,
            ..state
        };
        let state = update_dock_scroll(state, 240.0, false, 0.9);
        assert!(!state.compact);
        let state = update_dock_scroll(state, 288.0, false, 1.0);
        assert!(state.compact);
    }

    #[test]
    fn badge_and_header_use_same_web_total_and_cap_extra_kinds() {
        let badges = NavBadges {
            actions_pending: 2,
            products_low: 3,
            invoices_unpaid: 4,
            employees_pay_due: 5,
            ..Default::default()
        };
        let mut alerts = vec![Alert {
            kind: "low_stock".into(),
            ..Default::default()
        }];
        alerts.extend((0..9).map(|_| Alert {
            kind: "cheque".into(),
            ..Default::default()
        }));
        assert_eq!(notification_total(Some(&badges), &alerts), 20);
        assert_eq!(notification_total(None, &alerts), 6);
    }
}
