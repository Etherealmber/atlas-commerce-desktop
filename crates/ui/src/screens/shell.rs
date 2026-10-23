//! The authenticated shell: custom title bar, left icon rail, top bar and
//! the routed page content.

use egui::{Align, Color32, Layout, RichText, Sense, Vec2, vec2};

use crate::dialogs::{Confirm, ConfirmAction};
use crate::icons::Icon;
use crate::state::{AtlasApp, Route, Screen};
use crate::theme::Palette;
use crate::widgets::{danger_button, icon, secondary_button};
use crate::{t, widgets};

/// Height of the custom caption row.
const TITLEBAR_HEIGHT: f32 = 40.0;
const RAIL_WIDTH: f32 = 212.0;

/// French label + icon of every rail destination.
fn route_meta(route: Route) -> (Icon, &'static str) {
    match route {
        Route::Dashboard => (Icon::Dashboard, "Tableau de bord"),
        Route::Products => (Icon::Package, "Produits"),
        Route::Catalog => (Icon::Boxes, "Catalogue"),
        Route::Clients => (Icon::Users, "Clients"),
        Route::Vendors => (Icon::Store, "Fournisseurs"),
        Route::Expenses => (Icon::Wallet, "Dépenses"),
        Route::DeliveryNotes => (Icon::Truck, "Bons de livraison"),
        Route::Sales => (Icon::Receipt, "Ventes"),
        Route::Agent => (Icon::Bot, "Agent IA"),
        Route::Notifications => (Icon::Bell, "Notifications"),
        Route::Settings => (Icon::Settings, "Réglages"),
    }
}

pub fn draw_shell(app: &mut AtlasApp, ui: &mut egui::Ui, ctx: &egui::Context, palette: &Palette) {
    // Dedicated panels, not a hand-rolled horizontal layout: a `Panel::left`
    // with an exact size reserves its width up front, so the rail can never
    // expand to the full window and starve the central column (the blank
    // shell bug came from Frame + set_min_size, whose minimum width did not
    // stop rows calling available_width() from growing the frame).
    egui::Panel::top(egui::Id::new("shell-titlebar"))
        .exact_size(TITLEBAR_HEIGHT)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(palette.window).inner_margin(egui::Margin::same(0)))
        .show(ui, |ui| {
            draw_titlebar(app, ui, ctx, palette);
        });

    let rail_frame = egui::Frame::NONE
        .fill(palette.panel)
        .stroke(egui::Stroke::new(1.0, palette.outline))
        .inner_margin(egui::Margin::symmetric(10, 14));
    egui::Panel::left(egui::Id::new("shell-rail"))
        .exact_size(RAIL_WIDTH)
        .resizable(false)
        .show_separator_line(false)
        .frame(rail_frame)
        .show(ui, |ui| {
            draw_rail(app, ui, palette);
        });

    egui::CentralPanel::no_frame()
        .frame(egui::Frame::NONE.fill(palette.window).inner_margin(egui::Margin::same(0)))
        .show(ui, |ui| {
            draw_content(app, ui, ctx, palette);
        });

    if app.confirm_dialog.is_some() {
        draw_confirm_dialog(app, ui, palette);
    }
}

// ── Custom title bar ───────────────────────────────────────────────────────

fn draw_titlebar(app: &AtlasApp, ui: &mut egui::Ui, ctx: &egui::Context, palette: &Palette) {
    // The top panel already owns a TITLEBAR_HEIGHT strip; paint into it.
    let rect = ui.available_rect_before_wrap();
    ui.allocate_exact_size(rect.size(), Sense::hover());
    if fastframe_titlebar::enabled() {
        fastframe_titlebar::drag(ui, rect);
    }
    let footprint = fastframe_titlebar::footprint(ctx, TITLEBAR_HEIGHT);

    widgets::overlay_at(
        ui,
        egui::Rect::from_min_size(
            rect.min + vec2(footprint.left.max(12.0), 0.0),
            vec2(
                rect.width() - footprint.left - footprint.right - 24.0,
                TITLEBAR_HEIGHT,
            ),
        ),
        |ui| {
            ui.horizontal_centered(|ui| {
                crate::brand(ui, palette, 20.0);
                ui.label(
                    RichText::new(t!(app.locale, "Atlas Commerce Desktop"))
                        .color(palette.secondary)
                        .size(13.0),
                );
            });
        },
    );

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
                egui::pos2(rect.right() - footprint.right - 4.0, rect.top()),
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

// ── Left rail ──────────────────────────────────────────────────────────────

fn draw_rail(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let active = match app.screen {
        Screen::Shell(route) => Some(route),
        _ => None,
    };
    // The worker's own badge counters drive the rail: low stock, unpaid
    // invoices and open quick-sale tickets.
    let alert_count = app.nav_badges.data.as_ref().map_or(0, |badges| {
        badges.products_low + badges.invoices_unpaid + badges.vr_open
    });

    // The Panel owns the fixed width and frame; contents just fill it.
    ui.add_space(6.0);
    for route in Route::ALL {
        let (which, label) = route_meta(route);
        let is_active = active == Some(route);
        let response = rail_row(ui, palette, which, label, is_active);
        if route == Route::Notifications && alert_count > 0 {
            badge(ui, &response, palette, alert_count);
        }
        if response.clicked() {
            app.go(route);
            if route == Route::Notifications && app.alerts.data.is_none() {
                app.load_alerts();
            }
            if route == Route::Clients && app.clients.data.is_none() {
                app.load_clients();
            }
            if route == Route::Vendors && app.vendors.data.is_none() {
                app.load_vendors();
            }
            if route == Route::Expenses && app.expenses.data.is_none() {
                app.load_expenses();
            }
            if route == Route::DeliveryNotes && app.delivery_notes.data.is_none() {
                app.load_delivery_notes();
            }
            if route == Route::Catalog {
                if app.categories.data.is_none() {
                    app.load_categories();
                }
                if app.units.data.is_none() {
                    app.load_units();
                }
            }
            if route == Route::Sales && app.invoices.data.is_none() {
                app.load_invoices();
            }
        }
    }

    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        draw_user_card(app, ui, palette);
    });
}

fn rail_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    which: Icon,
    label: &str,
    is_active: bool,
) -> egui::Response {
    let height = 42.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    if is_active {
        ui.painter().rect_filled(rect, 10.0, palette.surface_active);
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.left() + 2.0, rect.center().y - 10.0),
                vec2(3.0, 20.0),
            ),
            2.0,
            palette.accent,
        );
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 10.0, palette.surface_hover);
    }
    let tint = if is_active {
        palette.accent
    } else {
        palette.secondary
    };
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
            ui.horizontal_centered(|ui| {
                ui.label(
                    RichText::new(label)
                        .color(if is_active {
                            palette.text
                        } else {
                            palette.secondary
                        })
                        .size(14.0)
                        .strong(),
                );
            });
        },
    );
    response
}

fn badge(ui: &egui::Ui, row: &egui::Response, palette: &Palette, count: i64) {
    let text = if count > 99 {
        "99+".to_owned()
    } else {
        count.to_string()
    };
    let galley =
        ui.painter()
            .layout_no_wrap(text, egui::FontId::proportional(11.5), palette.on_accent);
    let width = (galley.size().x + 12.0).max(20.0);
    let rect = egui::Rect::from_min_size(
        egui::pos2(row.rect.right() - width - 8.0, row.rect.center().y - 9.0),
        vec2(width, 18.0),
    );
    ui.painter().rect_filled(rect, 9.0, palette.accent);
    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        palette.on_accent,
    );
}

fn draw_user_card(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let Some(session) = app.session.clone() else {
        return;
    };
    let host = app
        .server_store
        .selected_url()
        .map(atlas_core::ServerUrl::host)
        .unwrap_or_else(|| "—".to_owned());
    widgets::card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            icon(ui, Icon::User, palette.accent, 20.0);
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
                ui.label(RichText::new(host).color(palette.dim).size(11.5));
            });
        });
        ui.add_space(6.0);
        if secondary_button(ui, t!(app.locale, "Sign out"), palette) {
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
}

// ── Routed content ─────────────────────────────────────────────────────────

fn draw_content(app: &mut AtlasApp, ui: &mut egui::Ui, ctx: &egui::Context, palette: &Palette) {
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(4.0);
            match app.screen {
                Screen::Shell(Route::Dashboard) => {
                    super::dashboard::draw(app, ui, palette);
                }
                Screen::Shell(Route::Products) => {
                    super::products::draw(app, ui, palette);
                }
                Screen::Shell(Route::Clients) => {
                    super::clients::draw(app, ui, palette);
                }
                Screen::Shell(Route::Catalog) => {
                    super::catalog::draw(app, ui, palette);
                }
                Screen::Shell(Route::Vendors) => {
                    super::vendors::draw(app, ui, palette);
                }
                Screen::Shell(Route::Expenses) => {
                    super::expenses::draw(app, ui, palette);
                }
                Screen::Shell(Route::DeliveryNotes) => {
                    super::delivery_notes::draw(app, ui, palette);
                }
                Screen::Shell(Route::Sales) => {
                    super::sales::draw(app, ui, palette);
                }
                Screen::Shell(Route::Agent) => {
                    super::agent::draw(app, ui, palette);
                }
                Screen::Shell(Route::Notifications) => {
                    super::notifications::draw(app, ui, palette);
                }
                Screen::Shell(Route::Settings) => {
                    super::settings::draw(app, ui, ctx, palette);
                }
                _ => {}
            }
        });
}

// ── Confirm dialog ─────────────────────────────────────────────────────────

fn draw_confirm_dialog(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let Some(dialog) = app.confirm_dialog.clone() else {
        return;
    };
    let screen = ui.ctx().input(|i| i.viewport_rect());
    let width = 420.0;
    let height = 210.0;
    let rect = egui::Rect::from_center_size(screen.center(), vec2(width, height));

    ui.painter()
        .rect_filled(screen, 0.0, Color32::from_black_alpha(140));
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_min_size(vec2(width - 44.0, 0.0));
            ui.label(
                RichText::new(dialog.title.clone())
                    .color(palette.text)
                    .strong()
                    .size(17.0),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(dialog.body.clone())
                    .color(palette.secondary)
                    .size(13.5),
            );
            ui.add_space(16.0);
            widgets::dialog_actions(ui, |ui| {
                let confirmed = if dialog.danger {
                    danger_button(ui, &dialog.confirm_label, palette, true)
                } else {
                    crate::widgets::primary_button(ui, &dialog.confirm_label, palette, true)
                };
                let dismissed = secondary_button(ui, t!(app.locale, "Cancel"), palette);
                if dismissed {
                    app.confirm_dialog = None;
                }
                if confirmed {
                    run_confirm_action(app, dialog.action);
                    app.confirm_dialog = None;
                }
            });
        });
    });
}

fn run_confirm_action(app: &mut AtlasApp, action: ConfirmAction) {
    match action {
        ConfirmAction::SignOut => app.sign_out(),
        ConfirmAction::DiscardEditor => {
            app.product_editor = None;
        }
        ConfirmAction::ForgetServer(url) => {
            app.server_store.forget(&url);
            app.account_store.clear_server(&url);
            app.save_servers();
            app.save_accounts();
            app.notice(t!(app.locale, "Server removed.").into_owned());
            if app.server_store.selected_url().is_none() {
                app.sign_out();
                app.screen = Screen::ServerSetup;
            }
        }
        ConfirmAction::RevokeDevice(fingerprint) => {
            let Some(client) = app.client.clone() else {
                return;
            };
            // Revoke, then reload the list in the same job slot so the
            // removed device disappears on the next poll.
            let job = app.runner.spawn(async move {
                client.revoke_device(&fingerprint).await?;
                client.devices().await
            });
            app.devices.start(job);
            app.notice(t!(app.locale, "Device revoked.").into_owned());
        }
    }
}
