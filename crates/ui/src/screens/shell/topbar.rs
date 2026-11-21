//! Title bar, search, profile and notification chrome.
use super::{RAIL_COLLAPSE_BELOW, TITLEBAR_HEIGHT, route_meta};
use crate::dialogs::{Confirm, ConfirmAction};
use crate::icons::Icon;
use crate::state::{AtlasApp, Route, Screen};
use crate::theme::Palette;
use crate::widgets::{icon, secondary_button};
use crate::{t, widgets};
use egui::{Align, Layout, RichText, Sense, Vec2, vec2};

const PROFILE_MENU_WIDTH: f32 = 248.0;

pub(super) fn draw_titlebar(
    app: &mut AtlasApp,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    palette: &Palette,
) {
    // The top panel already owns a TITLEBAR_HEIGHT strip; paint into it.
    let rect = ui.available_rect_before_wrap();
    ui.allocate_exact_size(rect.size(), Sense::hover());
    if fastframe_titlebar::enabled() {
        fastframe_titlebar::drag(ui, rect);
    }
    let footprint = fastframe_titlebar::footprint(ctx, TITLEBAR_HEIGHT);
    let left_inset = footprint.left.max(12.0);
    let right_inset = footprint.right.max(120.0);

    // Compact pane toggle stays in the caption row; navigation state is
    // window-local and can always be overridden by the user.
    let brand_width = 220.0_f32.min((rect.width() * 0.30).max(80.0));
    let brand = egui::Rect::from_min_size(
        egui::pos2(rect.left() + left_inset, rect.top()),
        egui::vec2(brand_width, rect.height()),
    );
    widgets::overlay_at(ui, brand, |ui| {
        ui.horizontal_centered(|ui| {
            let toggle = widgets::icon_circle_button(ui, palette, Icon::PanelLeft, true)
                .on_hover_text(t!(app.locale, "Toggle sidebar"));
            if toggle.clicked() {
                let id = egui::Id::new("rail-collapsed");
                let initial =
                    ctx.input(|input| input.viewport_rect().width()) < RAIL_COLLAPSE_BELOW;
                ctx.data_mut(|data| {
                    let collapsed = data.get_temp::<bool>(id).unwrap_or(initial);
                    data.insert_temp(id, !collapsed);
                });
            }
            if let Screen::Shell(route) = app.screen {
                let (_, title) = route_meta(app.locale, route);
                ui.label(RichText::new(title).color(palette.text).strong().size(14.0));
            }
        });
    });
    let strip = egui::Rect::from_min_max(
        egui::pos2(rect.left() + left_inset + brand_width, rect.top()),
        egui::pos2(rect.right() - right_inset - 8.0, rect.bottom()),
    );

    let mut avatar_rect = egui::Rect::NOTHING;
    let mut bell_rect = egui::Rect::NOTHING;
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
            let avatar_url = app.avatar_texture_uri.as_deref();
            // Fallback disk uses the primary accent so the initials stay
            // readable in both themes: paper-white disk with near-black ink
            // in dark mode, the inverse in light mode. The account menu the
            // avatar opens already carries the Settings entry, so no
            // separate gear button is shown here.
            let avatar = widgets::avatar(
                ui,
                avatar_url,
                &initial,
                crate::theme::TARGET_MIN_HEIGHT,
                palette.accent,
                palette.on_accent,
                palette.outline,
            );
            avatar_rect = avatar.rect;
            if avatar.clicked() {
                app.profile_menu_open = !app.profile_menu_open;
            }

            // An available release is visible from every screen: the icon
            // opens the update, and a dot means a copy is already staged and
            // only needs a restart.
            if app.update.progress.is_none()
                && (app.update.last_manifest.is_some() || app.update.ready_version.is_some())
            {
                let update = widgets::icon_circle_button(ui, palette, Icon::Download, true);
                if app.update.ready_version.is_some() {
                    paint_update_dot(ui, &update, palette);
                }
                let tooltip = match &app.update.ready_version {
                    Some(version) => format!(
                        "{} · v{version}",
                        t!(app.locale, "Update ready — restart to install")
                    ),
                    None => format!(
                        "{} · v{}",
                        t!(app.locale, "Update available"),
                        app.update
                            .last_manifest
                            .as_ref()
                            .map(|manifest| manifest.version.as_str())
                            .unwrap_or("—")
                    ),
                };
                let update = update.on_hover_text(tooltip);
                if update.clicked() {
                    // Reuses the staged copy when one exists; otherwise this
                    // starts the verified download and then the hand-off.
                    app.start_update_download();
                }
            }

            if app.role_policy().allows_route(Route::Notifications) {
                let bell = widgets::icon_circle_button(ui, palette, Icon::Bell, true);
                bell_rect = bell.rect;
                let alert_count = app.nav_badges.data.as_ref().map_or(0, |badges| {
                    badges.products_low + badges.invoices_unpaid + badges.vr_open
                });
                if alert_count > 0 {
                    paint_titlebar_badge(ui, &bell, palette, alert_count);
                }
                if bell.clicked() {
                    let id = egui::Id::new("notification-popover-open");
                    let open = ctx.data_mut(|data| {
                        let next = !data.get_temp::<bool>(id).unwrap_or(false);
                        data.insert_temp(id, next);
                        next
                    });
                    app.profile_menu_open = false;
                    if open {
                        let interval =
                            std::time::Duration::from_secs(app.settings.poll_interval_secs.max(15));
                        if !app.alerts.loading && app.alerts.stale_for(interval) {
                            app.load_alerts();
                        }
                        if !app.nav_badges.loading && app.nav_badges.stale_for(interval) {
                            app.load_nav_badges();
                        }
                    }
                }
            }
            let theme_button = widgets::icon_circle_button(
                ui,
                palette,
                if palette.dark { Icon::Sun } else { Icon::Moon },
                true,
            )
            .on_hover_text(t!(app.locale, "Toggle theme"));
            if theme_button.clicked() {
                app.settings.theme = if palette.dark {
                    atlas_core::settings::ThemeMode::Light
                } else {
                    atlas_core::settings::ThemeMode::Dark
                };
                app.save_settings();
                ctx.request_repaint();
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
            let search_width = ui.available_width().clamp(80.0, 260.0);
            draw_search_pill(app, ui, palette, search_width);
        });
    });

    if app.profile_menu_open {
        draw_profile_menu(app, ctx, palette, &avatar_rect);
    }
    if ctx.data(|data| {
        data.get_temp::<bool>(egui::Id::new("notification-popover-open"))
            .unwrap_or(false)
    }) && bell_rect != egui::Rect::NOTHING
    {
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

/// Recognize page shortcuts; other text starts a business search.
pub(super) fn search_destination(query: &str) -> Option<Route> {
    match crate::shortcuts::normalize(query.trim()).as_str() {
        "facture" | "factures" | "invoice" | "invoices" | "ventes" | "sales" => Some(Route::Sales),
        "situation" | "client" | "clients" => Some(Route::Clients),
        "produit" | "produits" | "products" => Some(Route::Products),
        "depense" | "depenses" | "expenses" => Some(Route::Expenses),
        "employe" | "employes" | "employees" | "equipe" | "rh" => Some(Route::Employees),
        "parametres" | "reglages" | "settings" => Some(Route::Settings),
        "agent" => Some(Route::Agent),
        _ => None,
    }
}

/// Search offers page shortcuts and searches business data on Enter.
fn draw_search_pill(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette, width: f32) {
    egui::Frame::NONE
        .fill(palette.surface)
        .stroke(egui::Stroke::new(1.0, palette.outline))
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(14, 0))
        .show(ui, |ui| {
            ui.set_height(crate::theme::TARGET_MIN_HEIGHT);
            ui.allocate_ui_with_layout(
                vec2(width, crate::theme::TARGET_MIN_HEIGHT),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    widgets::icon(ui, Icon::Search, palette.secondary, 15.0);
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut app.top_search)
                            .id(egui::Id::new("titlebar-search"))
                            .frame(egui::Frame::NONE)
                            .hint_text(t!(app.locale, "Search pages or business data…"))
                            .desired_width(width - 42.0)
                            .clip_text(true),
                    );
                    let enter = response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    if enter && !app.top_search.trim().is_empty() {
                        let query = app.top_search.trim().to_owned();
                        if let Some(route) = search_destination(&query) {
                            app.go(route);
                        } else {
                            app.search_business(query);
                        }
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
    // Counters are the one sanctioned saffron surface (Theme.kt badge).
    let galley =
        ui.painter()
            .layout_no_wrap(text, egui::FontId::proportional(10.5), palette.on_badge);
    let width = (galley.size().x + 8.0).max(15.0);
    let rect = egui::Rect::from_min_size(
        egui::pos2(button.rect.right() - width + 2.0, button.rect.top() - 2.0),
        vec2(width, 15.0),
    );
    ui.painter().rect_filled(rect, 999.0, palette.badge);
    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        palette.on_badge,
    );
}

/// A small accent dot on the update icon: a verified copy is staged and the
/// next restart installs it.
fn paint_update_dot(ui: &egui::Ui, response: &egui::Response, palette: &Palette) {
    let rect = response.rect;
    let center = egui::pos2(rect.right() - 3.0, rect.top() + 3.0);
    ui.painter().circle_filled(center, 4.0, palette.accent);
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
            ui.set_height(crate::theme::TARGET_MIN_HEIGHT);
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

/// Fetch alerts only when opened, never as a background fan-out.
fn draw_notifications_popover(
    app: &mut AtlasApp,
    ctx: &egui::Context,
    palette: &Palette,
    anchor: egui::Rect,
) {
    let id = egui::Id::new("notification-popover-open");
    let area = egui::Area::new(egui::Id::new("notification-popover"))
        .fixed_pos(egui::pos2(anchor.right() - 320.0, anchor.bottom() + 8.0))
        .order(egui::Order::Foreground)
        .constrain(true)
        .show(ctx, |ui| {
            widgets::pop_card(palette)
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.set_width(292.0);
                    ui.label(
                        RichText::new(t!(app.locale, "Notifications"))
                            .color(palette.text)
                            .strong()
                            .size(15.0),
                    );
                    ui.separator();
                    if app.alerts.loading && app.alerts.data.is_none() {
                        super::super::centered_spinner(ui, palette);
                    } else if let Some(error) = &app.alerts.error {
                        widgets::error_banner(ui, error, palette);
                        if secondary_button(ui, t!(app.locale, "Refresh"), palette) {
                            app.load_alerts();
                        }
                    } else if let Some(alerts) = app.alerts.data.as_ref() {
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
                                ui.label(RichText::new(&alert.label).color(palette.text));
                                ui.separator();
                            }
                        }
                    }
                    if secondary_button(ui, t!(app.locale, "View all"), palette) {
                        ctx.data_mut(|data| data.insert_temp(id, false));
                        app.go(Route::Notifications);
                    }
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
                                        session.role,
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
                        app.go(Route::Settings);
                    }
                    let settings_click =
                        menu_row(ui, palette, Icon::Settings, t!(app.locale, "Settings")).clicked();
                    if settings_click {
                        app.profile_menu_open = false;
                        app.settings_focus = crate::state::SettingsFocus::General;
                        app.go(Route::Settings);
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
                        t!(app.locale, "What's new"),
                    )
                    .clicked();
                    if whats_new {
                        app.profile_menu_open = false;
                        app.settings_focus = crate::state::SettingsFocus::About;
                        app.go(Route::Settings);
                        if app.update.release_notes.is_none() && !app.update.checking {
                            app.check_for_updates(false);
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
