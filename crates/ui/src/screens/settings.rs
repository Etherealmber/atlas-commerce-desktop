//! Réglages: appearance, general behaviour, the server list, trusted
//! devices, self-update and the about block.

use atlas_core::settings::{ThemeMode, UpdateChannel};
use atlas_core::{APP_NAME, SLUG};
use egui::{Context, RichText};
use fastframe_autostart::{Autostart, Start, State as AutostartState};

use crate::dialogs::{Confirm, ConfirmAction};
use crate::i18n::Locale;
use crate::icons::Icon;
use crate::state::{AtlasApp, Screen};
use crate::theme::Palette;
use crate::widgets::{
    card, danger_button, icon, info_banner, page_title, pill, primary_button, secondary_button,
};
use crate::{t, widgets};

const REPOSITORY_URL: &str = "https://github.com/Etherealmber/atlas-commerce-desktop";
const POLL_CHOICES: [u64; 4] = [30, 60, 120, 300];

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, ctx: &Context, palette: &Palette) {
    ui.add_space(8.0);
    page_title(ui, t!(app.locale, "Settings"), palette);
    ui.add_space(12.0);

    appearance(app, ui, palette);
    ui.add_space(14.0);
    general(app, ui, palette);
    ui.add_space(14.0);
    servers(app, ui, palette);
    ui.add_space(14.0);
    devices(app, ui, palette);
    ui.add_space(14.0);
    updates(app, ui, palette);
    ui.add_space(14.0);
    about(app, ui, ctx, palette);
}

// ── Appearance ─────────────────────────────────────────────────────────────

fn appearance(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    section(
        ui,
        palette,
        Icon::Palette,
        t!(app.locale, "Appearance").as_ref(),
    );
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(label(ui, t!(app.locale, "Language").as_ref(), palette));
        ui.horizontal(|ui| {
            for locale in [Locale::French, Locale::English] {
                let selected = app.locale == locale;
                let button = egui::Button::new(
                    RichText::new(locale.native_name())
                        .color(if selected {
                            palette.on_accent
                        } else {
                            palette.text
                        })
                        .size(13.0),
                )
                .fill(if selected {
                    palette.accent
                } else {
                    palette.surface
                })
                .corner_radius(8.0);
                if ui.add(button).clicked() {
                    app.settings.language = locale.tag().to_owned();
                    app.locale = locale;
                    app.save_settings();
                }
            }
        });

        ui.add_space(10.0);
        ui.label(label(ui, t!(app.locale, "Theme").as_ref(), palette));
        ui.horizontal(|ui| {
            let themes = [
                (ThemeMode::Dark, "Sombre"),
                (ThemeMode::Light, "Clair"),
                (ThemeMode::System, "Système"),
            ];
            for (mode, name) in themes {
                let selected = app.settings.theme == mode;
                let button = egui::Button::new(
                    RichText::new(name)
                        .color(if selected {
                            palette.on_accent
                        } else {
                            palette.text
                        })
                        .size(13.0),
                )
                .fill(if selected {
                    palette.accent
                } else {
                    palette.surface
                })
                .corner_radius(8.0);
                if ui.add(button).clicked() {
                    app.settings.theme = mode;
                    app.save_settings();
                }
            }
        });

        ui.add_space(10.0);
        if toggle_row(
            ui,
            palette,
            t!(app.locale, "Custom window title bar").as_ref(),
            &mut app.settings.custom_titlebar,
        ) {
            fastframe_titlebar::set_enabled(app.settings.custom_titlebar);
            app.save_settings();
        }
    });
}

// ── General ────────────────────────────────────────────────────────────────

fn general(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    section(
        ui,
        palette,
        Icon::Settings,
        t!(app.locale, "General").as_ref(),
    );
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());

        if toggle_row(
            ui,
            palette,
            t!(
                app.locale,
                "Keep running in the notification area when closed"
            )
            .as_ref(),
            &mut app.settings.close_to_tray,
        ) {
            app.save_settings();
        }

        ui.add_space(8.0);
        let mut autostart = app.settings.autostart;
        let changed = toggle_row(
            ui,
            palette,
            t!(app.locale, "Start at login").as_ref(),
            &mut autostart,
        );
        if changed {
            apply_autostart(autostart, app.settings.start_hidden);
            app.settings.autostart = autostart;
            app.save_settings();
        }
        if autostart {
            if toggle_row(
                ui,
                palette,
                t!(app.locale, "Start hidden in the notification area").as_ref(),
                &mut app.settings.start_hidden,
            ) {
                // Re-register with the new hidden/window mode.
                apply_autostart(true, app.settings.start_hidden);
                app.save_settings();
            }
            if matches!(app.autostart_state, AutostartState::NeedsApproval(_)) {
                ui.add_space(6.0);
                info_banner(
                    ui,
                    t!(
                        app.locale,
                        "Allow Atlas Commerce in the system's Login Items settings."
                    ),
                    palette,
                );
            }
        }

        ui.add_space(8.0);
        if toggle_row(
            ui,
            palette,
            t!(app.locale, "Native notifications for stock alerts").as_ref(),
            &mut app.settings.notifications_enabled,
        ) {
            app.save_settings();
        }

        ui.add_space(10.0);
        ui.label(label(
            ui,
            t!(app.locale, "Refresh interval").as_ref(),
            palette,
        ));
        ui.horizontal(|ui| {
            for seconds in POLL_CHOICES {
                let name = if seconds < 60 {
                    format!("{seconds} s")
                } else {
                    format!("{} min", seconds / 60)
                };
                let selected = app.settings.poll_interval_secs == seconds;
                let button = egui::Button::new(
                    RichText::new(name)
                        .color(if selected {
                            palette.on_accent
                        } else {
                            palette.text
                        })
                        .size(13.0),
                )
                .fill(if selected {
                    palette.accent
                } else {
                    palette.surface
                })
                .corner_radius(8.0);
                if ui.add(button).clicked() {
                    app.settings.poll_interval_secs = seconds;
                    app.save_settings();
                }
            }
        });
    });
}

fn apply_autostart(enabled: bool, hidden: bool) {
    let target = if enabled {
        Some(if hidden { Start::Hidden } else { Start::Window })
    } else {
        None
    };
    // Registry / file-system work: never on the UI thread.
    std::thread::spawn(move || {
        let autostart = Autostart::new(SLUG, APP_NAME).hidden_argument("--start-hidden");
        if let Err(error) = autostart.set(target) {
            log::warn!("autostart registration failed: {error}");
        }
    });
}

// ── Servers ────────────────────────────────────────────────────────────────

fn servers(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    section(
        ui,
        palette,
        Icon::Server,
        t!(app.locale, "Servers").as_ref(),
    );
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let entries = app.server_store.saved.clone();
        for entry in &entries {
            let is_selected = app.server_store.selected.as_deref() == Some(entry.url.as_str());
            ui.horizontal(|ui| {
                icon(
                    ui,
                    Icon::Server,
                    if is_selected {
                        palette.accent
                    } else {
                        palette.secondary
                    },
                    18.0,
                );
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(entry.label.clone())
                            .color(palette.text)
                            .strong()
                            .size(13.5),
                    );
                    ui.label(
                        RichText::new(entry.url.clone())
                            .color(palette.dim)
                            .size(11.5),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if is_selected {
                        pill(
                            ui,
                            t!(app.locale, "Active"),
                            palette.accent.gamma_multiply(0.18),
                            palette.accent,
                        );
                    } else if secondary_button(ui, t!(app.locale, "Use"), palette) {
                        switch_server(app, entry.url.clone());
                    }
                    if danger_button(ui, t!(app.locale, "Forget"), palette, true) {
                        app.confirm_dialog = Some(Confirm {
                            title: t!(app.locale, "Remove server?").into_owned(),
                            body: t!(
                                app.locale,
                                "Saved accounts on this server will be removed from this device."
                            )
                            .into_owned(),
                            confirm_label: t!(app.locale, "Remove").into_owned(),
                            danger: true,
                            action: ConfirmAction::ForgetServer(entry.url.clone()),
                        });
                    }
                });
            });
            ui.separator();
        }

        if app.force_server_setup {
            ui.add_space(8.0);
            super::auth::draw_server_setup(app, ui, palette, true);
            ui.add_space(8.0);
            if secondary_button(ui, t!(app.locale, "Close"), palette) {
                app.force_server_setup = false;
            }
        } else if primary_button(
            ui,
            t!(app.locale, "Add or configure a server"),
            palette,
            true,
        ) {
            app.force_server_setup = true;
        }
    });
}

fn switch_server(app: &mut AtlasApp, url: String) {
    app.server_store.selected = Some(url);
    app.save_servers();
    // A session is bound to one server origin: switching always signs out.
    app.sign_out();
    app.rebuild_client();
    app.screen = Screen::Login;
    app.notice(t!(app.locale, "Server changed.").into_owned());
}

// ── Trusted devices ────────────────────────────────────────────────────────

fn devices(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    section(
        ui,
        palette,
        Icon::ShieldCheck,
        t!(app.locale, "Security").as_ref(),
    );
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(t!(app.locale, "Device fingerprint"))
                .color(palette.secondary)
                .size(12.5),
        );
        ui.horizontal(|ui| {
            let fingerprint = app.identity.fingerprint();
            let short = fingerprint.chars().take(16).collect::<String>();
            ui.monospace(
                RichText::new(format!("{short}…"))
                    .color(palette.text)
                    .size(12.5),
            );
            if secondary_button(ui, t!(app.locale, "Copy"), palette) {
                ui.ctx().copy_text(fingerprint.to_owned());
            }
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!(app.locale, "Trusted devices"))
                    .color(palette.text)
                    .strong()
                    .size(14.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if secondary_button(ui, t!(app.locale, "Load devices"), palette) {
                    app.load_devices();
                }
            });
        });
        ui.add_space(6.0);

        if app.devices.loading && app.devices.data.is_none() {
            super::spinner(ui, palette);
        } else if let Some(devices) = app.devices.data.clone() {
            if devices.is_empty() {
                widgets::empty_state(
                    ui,
                    Icon::Smartphone,
                    t!(app.locale, "No trusted device"),
                    t!(app.locale, "Devices you sign in with are listed here."),
                    palette,
                );
            }
            for device in devices {
                ui.horizontal_top(|ui| {
                    icon(ui, Icon::Smartphone, palette.secondary, 18.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(device.full.clone())
                                .color(palette.text)
                                .size(13.0)
                                .strong(),
                        );
                        if let Some(ip) = &device.last_ip {
                            ui.label(
                                RichText::new(format!("IP : {ip}"))
                                    .color(palette.dim)
                                    .size(11.5),
                            );
                        }
                        if device.current {
                            pill(
                                ui,
                                t!(app.locale, "This device"),
                                palette.accent.gamma_multiply(0.18),
                                palette.accent,
                            );
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if !device.current
                            && danger_button(ui, t!(app.locale, "Revoke"), palette, true)
                        {
                            app.confirm_dialog = Some(Confirm {
                                title: t!(app.locale, "Revoke device?").into_owned(),
                                body: t!(app.locale, "That device will have to sign in again.")
                                    .into_owned(),
                                confirm_label: t!(app.locale, "Revoke").into_owned(),
                                danger: true,
                                action: ConfirmAction::RevokeDevice(device.revoke_id().to_owned()),
                            });
                        }
                    });
                });
                ui.separator();
            }
        }
    });
}

// ── Self-update ────────────────────────────────────────────────────────────

fn updates(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    section(
        ui,
        palette,
        Icon::Download,
        t!(app.locale, "Updates").as_ref(),
    );
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if toggle_row(
            ui,
            palette,
            t!(app.locale, "Check for updates on launch").as_ref(),
            &mut app.settings.update_check_on_start,
        ) {
            app.save_settings();
        }

        ui.add_space(8.0);
        ui.label(label(ui, t!(app.locale, "Channel").as_ref(), palette));
        ui.horizontal(|ui| {
            channel_button(
                ui,
                app,
                palette,
                UpdateChannel::Stable,
                t!(app.locale, "Stable").as_ref(),
            );
            channel_button(
                ui,
                app,
                palette,
                UpdateChannel::Prerelease,
                t!(app.locale, "Pre-release").as_ref(),
            );
        });

        ui.add_space(10.0);
        if app.update.checking {
            super::spinner(ui, palette);
        } else if primary_button(ui, t!(app.locale, "Check for updates now"), palette, true) {
            app.check_for_updates(false);
        }

        if let Some(error) = app.update.error.clone() {
            ui.add_space(8.0);
            widgets::error_banner(ui, &error, palette);
        }

        if let Some(manifest) = app.update.last_manifest.clone() {
            ui.add_space(10.0);
            info_banner(
                ui,
                format!(
                    "{} : v{} — {}",
                    t!(app.locale, "Latest version"),
                    manifest.version,
                    manifest.notes
                ),
                palette,
            );
            if app.update.progress.is_none()
                && primary_button(ui, t!(app.locale, "Download and install"), palette, true)
            {
                app.start_update_download();
            }
        }

        if let Some((received, total)) = app.update.progress {
            ui.add_space(8.0);
            let percent = if total > 0 {
                (received as f64 / total as f64 * 100.0).round() as u8
            } else {
                0
            };
            ui.label(
                RichText::new(format!(
                    "{} : {percent} %",
                    t!(app.locale, "Downloading update")
                ))
                .color(palette.secondary)
                .size(12.5),
            );
            ui.add(egui::ProgressBar::new(f32::from(percent) / 100.0));
        }
    });
}

fn channel_button(
    ui: &mut egui::Ui,
    app: &mut AtlasApp,
    palette: &Palette,
    channel: UpdateChannel,
    name: &str,
) {
    let selected = app.settings.update_channel == channel;
    let button = egui::Button::new(
        RichText::new(name)
            .color(if selected {
                palette.on_accent
            } else {
                palette.text
            })
            .size(13.0),
    )
    .fill(if selected {
        palette.accent
    } else {
        palette.surface
    })
    .corner_radius(8.0);
    if ui.add(button).clicked() {
        app.settings.update_channel = channel;
        app.save_settings();
        app.check_for_updates(false);
    }
}

// ── About ──────────────────────────────────────────────────────────────────

fn about(app: &AtlasApp, ui: &mut egui::Ui, _ctx: &Context, palette: &Palette) {
    section(ui, palette, Icon::Info, t!(app.locale, "About").as_ref());
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            crate::brand(ui, palette, 34.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(t!(app.locale, "Atlas Commerce Desktop"))
                        .color(palette.text)
                        .strong()
                        .size(15.0),
                );
                ui.label(
                    RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .color(palette.dim)
                        .size(12.0),
                );
            });
        });
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Client natif Rust pour la plateforme Atlas Commerce. Aucune télémétrie.",
            )
            .color(palette.secondary)
            .size(12.5),
        );
        ui.add_space(8.0);
        if secondary_button(ui, t!(app.locale, "Open project repository"), palette)
            && let Err(error) = open::that(REPOSITORY_URL)
        {
            log::warn!("could not open repository: {error}");
        }
    });
}

// ── Small shared bits ──────────────────────────────────────────────────────

fn section(ui: &mut egui::Ui, palette: &Palette, which: Icon, title: impl AsRef<str>) {
    ui.horizontal(|ui| {
        icon(ui, which, palette.accent, 18.0);
        ui.label(
            RichText::new(title.as_ref())
                .color(palette.text)
                .strong()
                .size(15.5),
        );
    });
    ui.add_space(6.0);
}

fn label(_ui: &egui::Ui, text: impl AsRef<str>, palette: &Palette) -> RichText {
    RichText::new(text.as_ref())
        .color(palette.secondary)
        .size(12.5)
}

/// A label with a right-aligned checkbox. Returns whether the value changed.
fn toggle_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    text: impl AsRef<str>,
    value: &mut bool,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(text.as_ref()).color(palette.text).size(13.5));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let response = ui.checkbox(value, "");
            if response.changed() {
                changed = true;
            }
        });
    });
    changed
}
