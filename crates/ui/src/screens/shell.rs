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
        .frame(
            egui::Frame::NONE
                .fill(palette.window)
                .inner_margin(egui::Margin::same(0)),
        )
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
        .frame(
            egui::Frame::NONE
                .fill(palette.window)
                .inner_margin(egui::Margin::same(0)),
        )
        .show(ui, |ui| {
            draw_content(app, ui, ctx, palette);
        });

    if app.confirm_dialog.is_some() {
        draw_confirm_dialog(app, ui, palette);
    }
}

// ── Custom title bar ───────────────────────────────────────────────────────

const PROFILE_MENU_WIDTH: f32 = 248.0;

fn draw_titlebar(app: &mut AtlasApp, ui: &mut egui::Ui, ctx: &egui::Context, palette: &Palette) {
    // The top panel already owns a TITLEBAR_HEIGHT strip; paint into it.
    let rect = ui.available_rect_before_wrap();
    ui.allocate_exact_size(rect.size(), Sense::hover());
    if fastframe_titlebar::enabled() {
        fastframe_titlebar::drag(ui, rect);
    }
    let footprint = fastframe_titlebar::footprint(ctx, TITLEBAR_HEIGHT);
    let left_inset = footprint.left.max(12.0);
    let right_inset = footprint.right.max(120.0);

    let strip = egui::Rect::from_min_max(
        egui::pos2(rect.left() + left_inset, rect.top()),
        egui::pos2(rect.right() - right_inset - 8.0, rect.bottom()),
    );

    let mut avatar_rect = egui::Rect::NOTHING;
    widgets::overlay_at(ui, strip, |ui| {
        // Right-to-left layout: the action cluster is placed against the
        // caption buttons first, the search pill fills whatever remains.
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            let initial = app
                .session
                .as_ref()
                .and_then(|session| {
                    if session.display_name.is_empty() {
                        session.username.chars().next()
                    } else {
                        session.display_name.chars().next()
                    }
                })
                .unwrap_or('A')
                .to_uppercase()
                .collect::<String>();
            let avatar =
                widgets::avatar_circle(ui, palette, &initial, 30.0, palette.surface_active);
            avatar_rect = avatar.rect;
            if avatar.clicked() {
                app.profile_menu_open = !app.profile_menu_open;
            }

            if widgets::icon_circle_button(ui, palette, Icon::Settings, true).clicked() {
                app.profile_menu_open = false;
                app.go(Route::Settings);
            }

            let bell = widgets::icon_circle_button(ui, palette, Icon::Bell, true);
            let alert_count = app.nav_badges.data.as_ref().map_or(0, |badges| {
                badges.products_low + badges.invoices_unpaid + badges.vr_open
            });
            if alert_count > 0 {
                paint_titlebar_badge(ui, &bell, palette, alert_count);
            }
            if bell.clicked() {
                app.go(Route::Notifications);
                if app.alerts.data.is_none() {
                    app.load_alerts();
                }
            }

            // While a release is downloading, surface a live progress pill
            // next to the bell instead of hiding the transfer on Settings.
            if let Some((downloaded, total)) = app.update.progress {
                draw_update_pill(ui, app, palette, downloaded, total);
                ui.add_space(6.0);
            }

            ui.add_space(8.0);

            // The pill takes the room left by the action cluster and the
            // navigation arrows, clamped to a readable range.
            let search_width = (ui.available_width() - 2.0 * 30.0 - 18.0).clamp(120.0, 420.0);
            draw_search_pill(app, ui, palette, search_width);

            ui.add_space(8.0);
            // The bundled Lucide set ships chevrons but no arrow-right, so the
            // navigation pair uses the matching chevron set.
            let forward = widgets::icon_circle_button(
                ui,
                palette,
                Icon::ChevronRight,
                !app.nav_forward.is_empty(),
            );
            if forward.clicked() {
                app.go_forward();
            }
            let back = widgets::icon_circle_button(
                ui,
                palette,
                Icon::ChevronLeft,
                !app.nav_back.is_empty(),
            );
            if back.clicked() {
                app.go_back();
            }
        });
    });

    if app.profile_menu_open {
        draw_profile_menu(app, ctx, palette, &avatar_rect);
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
                egui::pos2(rect.right() - footprint.right - 4.0, rect.top()),
                vec2(right_inset, TITLEBAR_HEIGHT),
            ),
            |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    fastframe_titlebar::controls(ui, &style, &labels);
                });
            },
        );
    }
}

/// The global search pill: Enter jumps to the products page with the query
/// applied, reusing its server-side search.
fn draw_search_pill(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette, width: f32) {
    egui::Frame::NONE
        .fill(palette.surface)
        .stroke(egui::Stroke::new(1.0, palette.outline))
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(12, 0))
        .show(ui, |ui| {
            ui.set_height(30.0);
            ui.allocate_ui_with_layout(
                vec2(width, 30.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    widgets::icon(ui, Icon::Search, palette.secondary, 15.0);
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut app.top_search)
                            .id(egui::Id::new("titlebar-search"))
                            .frame(egui::Frame::NONE)
                            .hint_text(t!(app.locale, "Search a product…"))
                            .desired_width(width - 42.0)
                            .clip_text(true),
                    );
                    let enter = response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    if enter && !app.top_search.trim().is_empty() {
                        let query = app.top_search.trim().to_owned();
                        app.product_search = query;
                        app.go(Route::Products);
                        app.load_products();
                    }
                },
            );
        });
}

/// Small accent counter at a title-bar button's top-right corner.
fn paint_titlebar_badge(ui: &mut egui::Ui, button: &egui::Response, palette: &Palette, count: i64) {
    let text = if count > 9 {
        "9+".to_owned()
    } else {
        count.to_string()
    };
    let galley =
        ui.painter()
            .layout_no_wrap(text, egui::FontId::proportional(10.5), palette.on_accent);
    let width = (galley.size().x + 8.0).max(15.0);
    let rect = egui::Rect::from_min_size(
        egui::pos2(button.rect.right() - width + 2.0, button.rect.top() - 2.0),
        vec2(width, 15.0),
    );
    ui.painter().rect_filled(rect, 999.0, palette.accent);
    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        palette.on_accent,
    );
}

/// Compact live download indicator shown beside the notification bell while
/// a release is being fetched, so the transfer never hides on Settings.
fn draw_update_pill(
    ui: &mut egui::Ui,
    app: &AtlasApp,
    palette: &Palette,
    downloaded: u64,
    total: u64,
) {
    let fraction = if total > 0 {
        (downloaded as f32 / total as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let percent = (fraction * 100.0).round() as u8;
    let mega = |bytes: u64| format!("{:.1} Mo", bytes as f64 / 1_048_576.0);
    let tooltip = format!(
        "{} — {} / {}",
        t!(app.locale, "Downloading update"),
        mega(downloaded),
        mega(total.max(downloaded))
    );
    let response = egui::Frame::NONE
        .fill(palette.surface)
        .stroke(egui::Stroke::new(1.0, palette.outline))
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(10, 0))
        .show(ui, |ui| {
            ui.set_height(30.0);
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 7.0;
                icon(ui, Icon::Download, palette.accent, 14.0);
                ui.label(
                    RichText::new(format!("{percent} %"))
                        .color(palette.text)
                        .strong()
                        .size(12.0),
                );
                let (track, _) = ui.allocate_exact_size(vec2(56.0, 4.0), Sense::hover());
                ui.painter()
                    .rect_filled(track, 999.0, palette.surface_active);
                let fill_rect = egui::Rect::from_min_size(
                    track.min,
                    vec2(track.width() * fraction, track.height()),
                );
                ui.painter().rect_filled(fill_rect, 999.0, palette.accent);
            });
        })
        .response;
    let stage = app.update.stage.trim();
    if stage.is_empty() {
        response.on_hover_text(tooltip);
    } else {
        response.on_hover_text(format!("{tooltip}\n{stage}"));
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
            widgets::card(palette)
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
                            widgets::avatar_circle(
                                ui,
                                palette,
                                &initial,
                                34.0,
                                palette.surface_active,
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
                                    RichText::new(session.username.clone())
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
                        app.go(Route::Settings);
                    }
                    let settings_click =
                        menu_row(ui, palette, Icon::Settings, t!(app.locale, "Settings")).clicked();
                    if settings_click {
                        app.profile_menu_open = false;
                        app.settings_focus = crate::state::SettingsFocus::General;
                        app.go(Route::Settings);
                    }
                    // Web parity (profile dropdown): release notes open on
                    // GitHub in the browser, About jumps to the settings
                    // section that carries version and repository links.
                    let whats_new =
                        menu_row(ui, palette, Icon::ExternalLink, t!(app.locale, "What's new"))
                            .clicked();
                    if whats_new {
                        app.profile_menu_open = false;
                        if let Err(error) =
                            open::that(crate::screens::settings::RELEASES_URL)
                        {
                            log::warn!("could not open releases: {error}");
                        }
                    }
                    let about =
                        menu_row(ui, palette, Icon::Info, t!(app.locale, "About")).clicked();
                    if about {
                        app.profile_menu_open = false;
                        app.settings_focus = crate::state::SettingsFocus::About;
                        app.go(Route::Settings);
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
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
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
            ui.horizontal_centered(|ui| {
                ui.label(RichText::new(label.as_ref()).color(palette.text).size(13.5));
            });
        },
    );
    response
}

// ── Left rail ──────────────────────────────────────────────────────────────

fn draw_rail(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let active = match app.screen {
        Screen::Shell(route) => Some(route),
        _ => None,
    };
    // The Panel owns the fixed width and frame; contents just fill it.
    ui.add_space(6.0);
    for route in Route::ALL {
        // The bell (notifications) and gear (settings) already live in the
        // title bar; never duplicate them in the rail. The account card used
        // to be pinned here as well — the avatar now owns that surface.
        if matches!(route, Route::Notifications | Route::Settings) {
            continue;
        }
        let (which, label) = route_meta(route);
        let is_active = active == Some(route);
        let response = rail_row(ui, palette, which, label, is_active);
        if response.clicked() {
            app.go(route);
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
}

fn rail_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    which: Icon,
    label: &str,
    is_active: bool,
) -> egui::Response {
    let height = 42.0;
    // Reserve the row with a passive hover sense first. The icon and label
    // are painted through overlay child UIs whose widgets register their own
    // hover rectangles and used to swallow presses landing on the text; the
    // click interaction below is therefore registered last, so the full row
    // (icon and label text included) is one clickable surface.
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let hovered = ui.rect_contains_pointer(rect);
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
    } else if hovered {
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
    // Last interaction wins for overlapping rectangles, so registering the
    // row click after the overlay widgets keeps the label text clickable.
    ui.interact(
        rect,
        egui::Id::new(("rail-row", label)),
        Sense::click(),
    )
}

// ── Routed content ─────────────────────────────────────────────────────────

fn draw_content(app: &mut AtlasApp, ui: &mut egui::Ui, ctx: &egui::Context, palette: &Palette) {
    // Android-style entrance: each navigation replays a quick cubic-out
    // fade while the content rises ~18 px into place. The id carries the
    // animation token, so a fresh navigation starts from zero. The very
    // first shell entry (token 0) stays static: a fade from opacity zero
    // on a cold window reads as launch flicker, not as navigation.
    let entrance = if app.anim_token == 0 {
        1.0
    } else {
        ctx.animate_bool_with_time_and_easing(
            egui::Id::new(("shell-route-enter", app.anim_token)),
            true,
            0.24,
            egui::emath::easing::cubic_out,
        )
    };
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_opacity(entrance);
            ui.add_space(4.0 + (1.0 - entrance) * 18.0);
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
        ConfirmAction::RevokeApiKey(id) => app.revoke_api_key(id),
    }
}
