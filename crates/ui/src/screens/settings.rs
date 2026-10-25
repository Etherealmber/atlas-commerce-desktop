//! Réglages, organisés en cartes thématiques comme le client de référence :
//! profil, application, apparence, agent IA, clés API, serveur, sécurité et
//! mises à jour. Chaque ligne associe un titre (et une description) à son
//! contrôle droit; les commutateurs sont animés style Android.

use atlas_core::settings::{ThemeMode, UpdateChannel};
use atlas_core::{APP_NAME, SLUG};
use egui::{Context, RichText};
use fastframe_autostart::{Autostart, Start, State as AutostartState};

use crate::dialogs::{Confirm, ConfirmAction};
use crate::i18n::Locale;
use crate::icons::Icon;
use crate::state::{ApiKeyScope, AtlasApp, Screen, SettingsFocus};
use crate::theme::Palette;
use crate::widgets::{
    avatar, card, danger_button, error_banner, icon, info_banner, labelled_field, page_title, pill,
    primary_button, secondary_button, section_header, segmented, setting_row, switch_row,
};
use crate::{t, widgets};

pub(crate) const REPOSITORY_URL: &str = "https://github.com/Etherealmber/atlas-commerce-desktop";
pub(crate) const RELEASES_URL: &str =
    "https://github.com/Etherealmber/atlas-commerce-desktop/releases";
const POLL_CHOICES: [u64; 4] = [30, 60, 120, 300];

/// Readable measure for the settings column; cards no longer stretch across
/// the whole window on large displays.
const SETTINGS_WIDTH: f32 = 880.0;

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, _ctx: &Context, palette: &Palette) {
    // An avatar-menu entry may ask for a specific section; consume the focus
    // once and bring that cursor into the scroll area's view.
    let focus = app.settings_focus;
    app.settings_focus = SettingsFocus::General;

    // Center a capped column instead of stretching the forms edge to edge.
    let available_width = ui.available_width();
    let content_width = available_width.min(SETTINGS_WIDTH);
    let side_margin = ((available_width - content_width) * 0.5).max(0.0);
    ui.horizontal(|ui| {
        ui.add_space(side_margin);
        ui.vertical(|ui| {
            ui.set_min_width(content_width);
            ui.set_max_width(content_width);

            ui.add_space(8.0);
            page_title(ui, t!(app.locale, "Settings"), palette);
            ui.add_space(8.0);

            if focus == SettingsFocus::Profile {
                ui.scroll_to_cursor(Some(egui::Align::Min));
            }
            profile_section(app, ui, palette);
            ui.add_space(18.0);

            application_section(app, ui, palette);
            ui.add_space(18.0);

            appearance_section(app, ui, palette);
            ui.add_space(18.0);

            assist_section(app, ui, palette);
            ui.add_space(18.0);

            if is_admin(app) {
                if focus == SettingsFocus::Integrations {
                    ui.scroll_to_cursor(Some(egui::Align::Min));
                }
                integrations_section(app, ui, palette);
                ui.add_space(18.0);
            }

            servers_section(app, ui, palette);
            ui.add_space(18.0);

            security_section(app, ui, palette);
            ui.add_space(18.0);

            if focus == SettingsFocus::About {
                ui.scroll_to_cursor(Some(egui::Align::Min));
            }
            updates_section(app, ui, palette);
            ui.add_space(24.0);

            if app.created_key.is_some() {
                draw_created_key_modal(app, ui, palette);
            }
        });
    });
}

fn is_admin(app: &AtlasApp) -> bool {
    matches!(
        app.session.as_ref().map(|session| session.role.as_str()),
        Some("owner" | "admin")
    )
}

fn role_label(app: &AtlasApp) -> String {
    match app.session.as_ref().map(|session| session.role.as_str()) {
        Some("owner") => t!(app.locale, "Owner").into_owned(),
        Some("admin") => t!(app.locale, "Administrator").into_owned(),
        _ => t!(app.locale, "User").into_owned(),
    }
}

// ── Profile ────────────────────────────────────────────────────────────────

fn profile_section(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    section_header(ui, palette, t!(app.locale, "Profile"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());

        if app.profile.loading && app.profile.data.is_none() {
            super::centered_spinner(ui, palette);
            return;
        }
        if let Some(error) = app.profile.error.clone() {
            error_banner(ui, &error, palette);
            ui.add_space(8.0);
            if secondary_button(ui, t!(app.locale, "Retry"), palette) {
                app.load_profile();
            }
            return;
        }

        ui.horizontal(|ui| {
            let initial = app
                .profile_form
                .display_name
                .chars()
                .next()
                .or_else(|| app.session.as_ref().and_then(|s| s.username.chars().next()))
                .unwrap_or('A')
                .to_uppercase()
                .collect::<String>();
            // Preview the saved picture only: fetching every intermediate
            // URL while typing would spam the HTTP loader (and arbitrary
            // hosts). The new URL appears here once the profile is saved.
            let avatar_url = app
                .session
                .as_ref()
                .map(|session| session.avatar_url.as_str());
            avatar(
                ui,
                avatar_url,
                &initial,
                52.0,
                palette.surface_active,
                palette.on_accent,
                palette.outline,
            );
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(
                        app.session
                            .as_ref()
                            .map(|session| session.username.clone())
                            .unwrap_or_default(),
                    )
                    .color(palette.text)
                    .strong()
                    .size(14.5),
                );
                pill(
                    ui,
                    role_label(app),
                    palette.surface_active,
                    palette.secondary,
                );
            });
        });

        ui.add_space(12.0);
        setting_row(
            ui,
            palette,
            t!(app.locale, "Username"),
            Some(&t!(app.locale, "The identifier cannot be changed.")),
            |ui| {
                ui.label(
                    RichText::new(
                        app.session
                            .as_ref()
                            .map(|session| session.username.clone())
                            .unwrap_or_default(),
                    )
                    .color(palette.dim)
                    .size(13.0),
                );
            },
        );
        ui.add_space(10.0);

        labelled_field(
            ui,
            t!(app.locale, "Display name"),
            &mut app.profile_form.display_name,
            palette,
            false,
        );
        ui.add_space(8.0);
        labelled_field(
            ui,
            t!(app.locale, "Email"),
            &mut app.profile_form.email,
            palette,
            false,
        );
        ui.add_space(8.0);
        labelled_field(
            ui,
            t!(app.locale, "Phone"),
            &mut app.profile_form.phone,
            palette,
            false,
        );
        ui.add_space(8.0);

        ui.label(
            RichText::new(t!(app.locale, "Personal note"))
                .color(palette.secondary)
                .size(12.5),
        );
        ui.add(
            egui::TextEdit::multiline(&mut app.profile_form.notes)
                .desired_rows(2)
                .desired_width(ui.available_width())
                .margin(egui::Margin::symmetric(10, 8))
                .clip_text(true),
        );
        ui.add_space(8.0);
        labelled_field(
            ui,
            t!(app.locale, "Profile picture URL"),
            &mut app.profile_form.avatar_url,
            palette,
            false,
        );

        ui.add_space(14.0);
        ui.label(
            RichText::new(t!(
                app.locale,
                "Password changes are handled by an administrator, not from this screen."
            ))
            .color(palette.dim)
            .size(11.5),
        );
        ui.add_space(10.0);
        let saving = app.profile_save_job.is_some();
        if primary_button(
            ui,
            t!(app.locale, "Save profile"),
            palette,
            !saving && app.profile_form.loaded,
        ) {
            app.save_profile();
        }
    });
}

// ── Application behaviour ──────────────────────────────────────────────────

fn application_section(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    section_header(ui, palette, t!(app.locale, "Application"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());

        if switch_row(
            ui,
            palette,
            t!(
                app.locale,
                "Keep running in the notification area when closed"
            ),
            Some(&t!(
                app.locale,
                "Closing the window hides the app instead of quitting it."
            )),
            &mut app.settings.close_to_tray,
            app.settings.reduced_motion,
        ) {
            app.save_settings();
        }

        ui.add_space(10.0);
        let mut autostart = app.settings.autostart;
        if switch_row(
            ui,
            palette,
            t!(app.locale, "Start at login"),
            Some(&t!(
                app.locale,
                "Launches Atlas Commerce when the computer starts."
            )),
            &mut autostart,
            app.settings.reduced_motion,
        ) {
            apply_autostart(autostart, app.settings.start_hidden);
            app.settings.autostart = autostart;
            app.save_settings();
        }
        if autostart {
            ui.add_space(8.0);
            if switch_row(
                ui,
                palette,
                t!(app.locale, "Start hidden in the notification area"),
                Some(&t!(
                    app.locale,
                    "No window opens until you click the tray icon."
                )),
                &mut app.settings.start_hidden,
                app.settings.reduced_motion,
            ) {
                apply_autostart(true, app.settings.start_hidden);
                app.save_settings();
            }
            if matches!(app.autostart_state, AutostartState::NeedsApproval(_)) {
                ui.add_space(8.0);
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

        ui.add_space(10.0);
        let notifications_were_enabled = app.settings.notifications_enabled;
        if switch_row(
            ui,
            palette,
            t!(app.locale, "Native notifications for stock alerts"),
            Some(&t!(
                app.locale,
                "High-severity alerts also raise a system notification."
            )),
            &mut app.settings.notifications_enabled,
            app.settings.reduced_motion,
        ) {
            app.save_settings();
            // Enabling toasts immediately needs alert data: start the first
            // fetch now instead of waiting for the Notifications screen.
            if app.settings.notifications_enabled
                && !notifications_were_enabled
                && app.alerts.data.is_none()
                && !app.alerts.loading
            {
                app.load_alerts();
            }
        }

        ui.add_space(10.0);
        if switch_row(
            ui,
            palette,
            t!(app.locale, "Custom window title bar"),
            Some(&t!(
                app.locale,
                "Draws the caption bar inside the application."
            )),
            &mut app.settings.custom_titlebar,
            app.settings.reduced_motion,
        ) {
            fastframe_titlebar::set_enabled(app.settings.custom_titlebar);
            app.save_settings();
        }

        ui.add_space(10.0);
        if switch_row(
            ui,
            palette,
            t!(app.locale, "Reduce animations"),
            Some(&t!(
                app.locale,
                "Remove transition and entrance effects."
            )),
            &mut app.settings.reduced_motion,
            app.settings.reduced_motion,
        ) {
            app.save_settings();
        }

        ui.add_space(12.0);
        ui.label(
            RichText::new(t!(app.locale, "Refresh interval"))
                .color(palette.text)
                .strong()
                .size(13.5),
        );
        ui.add_space(6.0);
        let options: Vec<(String, u64)> = POLL_CHOICES
            .iter()
            .map(|seconds| {
                let name = if *seconds < 60 {
                    format!("{seconds} s")
                } else {
                    format!("{} min", seconds / 60)
                };
                (name, *seconds)
            })
            .collect();
        let refs: Vec<(&str, u64)> = options.iter().map(|(n, v)| (n.as_str(), *v)).collect();
        let current_interval = app.settings.poll_interval_secs;
        if let Some(seconds) = segmented(ui, palette, &refs, Some(current_interval)) {
            app.settings.poll_interval_secs = seconds;
            app.save_settings();
        }
    });
}

// ── Appearance ─────────────────────────────────────────────────────────────

fn appearance_section(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    section_header(ui, palette, t!(app.locale, "Appearance"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());

        setting_row(
            ui,
            palette,
            t!(app.locale, "Language"),
            Some(&t!(
                app.locale,
                "French is the default; the English catalog is built in."
            )),
            |ui| {
                let options = [
                    (Locale::French.native_name(), Locale::French),
                    ("English", Locale::English),
                ];
                if let Some(locale) = segmented(ui, palette, &options, Some(app.locale)) {
                    app.settings.language = locale.tag().to_owned();
                    app.locale = locale;
                    app.save_settings();
                }
            },
        );

        ui.add_space(12.0);
        setting_row(
            ui,
            palette,
            t!(app.locale, "Theme"),
            Some(&t!(
                app.locale,
                "System follows the operating system appearance."
            )),
            |ui| {
                // Bind the translated labels so the borrowed strings outlive
                // the array that references them.
                let dark = t!(app.locale, "Dark");
                let light = t!(app.locale, "Light");
                let system = t!(app.locale, "System");
                let options = [
                    (dark.as_ref(), ThemeMode::Dark),
                    (light.as_ref(), ThemeMode::Light),
                    (system.as_ref(), ThemeMode::System),
                ];
                if let Some(mode) = segmented(ui, palette, &options, Some(app.settings.theme)) {
                    app.settings.theme = mode;
                    app.save_settings();
                }
            },
        );
    });
}

// ── AI assist provider ─────────────────────────────────────────────────────

fn assist_section(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    section_header(ui, palette, t!(app.locale, "AI Agent"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if app.assist.loading && app.assist.data.is_none() {
            super::centered_spinner(ui, palette);
            return;
        }
        if let Some(error) = app.assist.error.clone() {
            error_banner(ui, &error, palette);
            ui.add_space(8.0);
            if secondary_button(ui, t!(app.locale, "Retry"), palette) {
                app.load_assist();
            }
            return;
        }
        let Some(provider) = app.assist.data.clone() else {
            if secondary_button(ui, t!(app.locale, "Load status"), palette) {
                app.load_assist();
            }
            return;
        };

        let configured = provider.groq_configured;
        let provider_hint = t!(
            app.locale,
            "The language model is configured on the server with a secret key; the desktop client never holds it."
        );
        setting_row(
            ui,
            palette,
            t!(app.locale, "Assistant provider"),
            Some(&provider_hint),
            |ui| {
                if configured {
                    pill(
                        ui,
                        t!(app.locale, "Configured"),
                        palette.finance_positive_soft,
                        palette.finance_positive,
                    );
                } else {
                    pill(
                        ui,
                        t!(app.locale, "Not configured"),
                        palette.finance_warning_soft,
                        palette.finance_warning,
                    );
                }
            },
        );
        ui.add_space(10.0);
        ui.label(
            RichText::new(format!(
                "{} : Groq",
                t!(app.locale, "Provider"),
            ))
            .color(palette.secondary)
            .size(12.5),
        );
    });
}

// ── Public API keys (owner/admin) ──────────────────────────────────────────

fn integrations_section(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    section_header(ui, palette, t!(app.locale, "API keys"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(t!(
                app.locale,
                "Public keys let external tools call the Atlas Commerce API on this shop's behalf."
            ))
            .color(palette.dim)
            .size(12.0),
        );
        ui.add_space(10.0);

        if app.api_keys.loading && app.api_keys.data.is_none() {
            super::centered_spinner(ui, palette);
        } else if let Some(error) = app.api_keys.error.clone() {
            error_banner(ui, &error, palette);
            ui.add_space(8.0);
            if secondary_button(ui, t!(app.locale, "Retry"), palette) {
                app.load_api_keys();
            }
        } else if let Some(keys) = app.api_keys.data.clone() {
            if keys.is_empty() {
                widgets::empty_state(
                    ui,
                    Icon::KeyRound,
                    t!(app.locale, "No API key yet"),
                    t!(app.locale, "Create one below to connect an external tool."),
                    palette,
                );
            }
            for key in &keys {
                api_key_row(app, ui, palette, key);
                ui.separator();
            }
        } else if secondary_button(ui, t!(app.locale, "Load API keys"), palette) {
            app.load_api_keys();
        }

        ui.add_space(10.0);
        if app.key_editor.is_none()
            && primary_button(ui, t!(app.locale, "Create an API key"), palette, true)
        {
            app.key_editor = Some(crate::state::ApiKeyEditor::default());
        }
    });

    if app.key_editor.is_some() {
        ui.add_space(10.0);
        key_editor_card(app, ui, palette);
    }
}

fn api_key_row(
    app: &mut AtlasApp,
    ui: &mut egui::Ui,
    palette: &Palette,
    key: &atlas_api::models::ApiKey,
) {
    ui.horizontal_top(|ui| {
        icon(ui, Icon::Key, palette.secondary, 18.0);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(key.name.clone())
                    .color(palette.text)
                    .strong()
                    .size(13.5),
            );
            ui.label(
                RichText::new(format!("{}…", key.key_prefix))
                    .color(palette.dim)
                    .size(11.5),
            );
            let mut meta = vec![format!("{} : {}", t!(app.locale, "Scopes"), key.scopes)];
            if let Some(created) = &key.created_at {
                meta.push(format!(
                    "{} : {}",
                    t!(app.locale, "Created"),
                    short_date(created)
                ));
            }
            if let Some(last) = &key.last_used_at {
                meta.push(format!(
                    "{} : {}",
                    t!(app.locale, "Last use"),
                    short_date(last)
                ));
            } else {
                meta.push(t!(app.locale, "Never used").into_owned());
            }
            ui.label(
                RichText::new(meta.join("  ·  "))
                    .color(palette.dim)
                    .size(11.0),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if danger_button(ui, t!(app.locale, "Revoke"), palette, true) {
                app.confirm_dialog = Some(Confirm {
                    title: t!(app.locale, "Revoke API key?").into_owned(),
                    body: t!(
                        app.locale,
                        "Tools using this key will immediately lose access."
                    )
                    .into_owned(),
                    confirm_label: t!(app.locale, "Revoke").into_owned(),
                    danger: true,
                    action: ConfirmAction::RevokeApiKey(key.id),
                });
            }
        });
    });
}

fn key_editor_card(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let mut editor = app.key_editor.clone().unwrap_or_default();
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(t!(app.locale, "New API key"))
                .color(palette.text)
                .strong()
                .size(14.0),
        );
        ui.add_space(8.0);
        labelled_field(
            ui,
            t!(app.locale, "Key name"),
            &mut editor.name,
            palette,
            false,
        );
        ui.add_space(10.0);
        ui.label(
            RichText::new(t!(app.locale, "Scopes"))
                .color(palette.text)
                .strong()
                .size(13.5),
        );
        ui.add_space(6.0);
        let read_only = t!(app.locale, "Read only");
        let read_write = t!(app.locale, "Read and write");
        let options = [
            (read_only.as_ref(), ApiKeyScope::Read),
            (read_write.as_ref(), ApiKeyScope::ReadWrite),
        ];
        if let Some(scope) = segmented(ui, palette, &options, editor.scopes) {
            editor.scopes = Some(scope);
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(t!(
                app.locale,
                "Choose a scope explicitly; none is applied by default."
            ))
            .color(palette.dim)
            .size(11.0),
        );

        ui.add_space(12.0);
        let name_ok = !editor.name.trim().is_empty();
        let scope_ok = editor.scopes.is_some();
        let busy = app.key_create_job.is_some();
        ui.horizontal(|ui| {
            if primary_button(
                ui,
                t!(app.locale, "Generate key"),
                palette,
                name_ok && scope_ok && !busy,
            ) {
                app.key_editor = Some(editor.clone());
                app.create_api_key();
            }
            if secondary_button(ui, t!(app.locale, "Cancel"), palette) {
                app.key_editor = None;
            }
        });
        if !name_ok || !scope_ok {
            ui.add_space(6.0);
            ui.label(
                RichText::new(t!(
                    app.locale,
                    "A name and an explicit scope are required before generating."
                ))
                .color(palette.warning)
                .size(11.5),
            );
        }
    });
    // Cancel clears the editor inside the card; do not bring it back. While
    // the editor stays open, persist the locally edited fields.
    if app.key_editor.is_some() {
        app.key_editor = Some(editor);
    }
}

/// One-time reveal of the freshly generated secret.
fn draw_created_key_modal(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let Some(created) = app.created_key.clone() else {
        return;
    };
    let screen = ui.ctx().input(|input| input.viewport_rect());
    let width = 480.0;
    let height = 250.0;
    let rect = egui::Rect::from_center_size(screen.center(), egui::vec2(width, height));
    if widgets::scrim_clicked(
        ui, screen, rect, egui::Id::new("created-key-scrim")
    ) {
        app.created_key = None;
        return;
    }
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_min_size(egui::vec2(width - 44.0, 0.0));
            ui.label(
                RichText::new(t!(app.locale, "Copy this key now"))
                    .color(palette.text)
                    .font(crate::fonts::semibold(17.0)),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(t!(
                    app.locale,
                    "It will not be shown again after this dialog closes."
                ))
                .color(palette.secondary)
                .size(13.0),
            );
            ui.add_space(12.0);
            egui::Frame::NONE
                .fill(palette.window)
                .stroke(egui::Stroke::new(1.0, palette.outline))
                .corner_radius(crate::theme::RADIUS_CONTROL as f32)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.monospace(
                        RichText::new(created.key.clone())
                            .color(palette.text)
                            .size(12.5),
                    );
                });
            ui.add_space(14.0);
            widgets::dialog_actions(ui, |ui| {
                if primary_button(ui, t!(app.locale, "Copy and close"), palette, true) {
                    ui.ctx().copy_text(created.key.clone());
                    app.created_key = None;
                }
                if secondary_button(ui, t!(app.locale, "Close"), palette) {
                    app.created_key = None;
                }
            });
        });
    });
}

// ── Servers ────────────────────────────────────────────────────────────────

fn servers_section(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    section_header(ui, palette, t!(app.locale, "Servers"));
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

fn security_section(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    section_header(ui, palette, t!(app.locale, "Security"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        setting_row(
            ui,
            palette,
            t!(app.locale, "Device fingerprint"),
            None,
            |ui| {
                let fingerprint = app.identity.fingerprint();
                let short = fingerprint.chars().take(16).collect::<String>();
                ui.horizontal(|ui| {
                    ui.monospace(
                        RichText::new(format!("{short}…"))
                            .color(palette.text)
                            .size(12.5),
                    );
                    if secondary_button(ui, t!(app.locale, "Copy"), palette) {
                        ui.ctx().copy_text(fingerprint.to_owned());
                    }
                });
            },
        );

        ui.add_space(12.0);
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
            super::centered_spinner(ui, palette);
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

// ── Self-update and about ──────────────────────────────────────────────────

fn updates_section(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    section_header(ui, palette, t!(app.locale, "Updates and about"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());

        ui.horizontal(|ui| {
            crate::brand(ui, palette, 32.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(t!(app.locale, "Atlas Commerce Desktop"))
                        .color(palette.text)
                        .strong()
                        .size(14.5),
                );
                ui.label(
                    RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .color(palette.dim)
                        .size(12.0),
                );
            });
        });
        ui.add_space(10.0);

        // Explicit check status, whatever the answer was.
        if app.update.checking {
            ui.horizontal(|ui| {
                super::spinner(ui, palette);
                ui.label(
                    RichText::new(t!(app.locale, "Checking for updates…"))
                        .color(palette.secondary)
                        .size(13.0),
                );
            });
        } else if let Some(manifest) = app.update.last_manifest.clone() {
            card(palette)
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        icon(ui, Icon::Download, palette.accent, 18.0);
                        ui.label(
                            RichText::new(format!(
                                "{} v{}",
                                t!(app.locale, "Update available"),
                                manifest.version
                            ))
                            .color(palette.text)
                            .strong()
                            .size(13.5),
                        );
                    });
                    ui.add_space(8.0);
                    crate::widgets::release_notes(ui, &manifest.notes, palette, 180.0);
                });
        } else if app.update.up_to_date_at.is_some() {
            ui.horizontal(|ui| {
                icon(ui, Icon::CircleCheck, palette.accent, 18.0);
                ui.label(
                    RichText::new(t!(app.locale, "You are on the latest version."))
                        .color(palette.text)
                        .size(13.0),
                );
            });
        }
        if let Some(error) = app.update.error.clone() {
            ui.add_space(8.0);
            error_banner(ui, &error, palette);
        }

        ui.add_space(12.0);
        if switch_row(
            ui,
            palette,
            t!(app.locale, "Check for updates on launch"),
            Some(&t!(
                app.locale,
                "GitHub Releases are the only update source."
            )),
            &mut app.settings.update_check_on_start,
            app.settings.reduced_motion,
        ) {
            app.save_settings();
        }

        ui.add_space(10.0);
        setting_row(ui, palette, t!(app.locale, "Channel"), None, |ui| {
            let stable = t!(app.locale, "Stable");
            let prerelease = t!(app.locale, "Pre-release");
            let options = [
                (stable.as_ref(), UpdateChannel::Stable),
                (prerelease.as_ref(), UpdateChannel::Prerelease),
            ];
            if let Some(channel) =
                segmented(ui, palette, &options, Some(app.settings.update_channel))
            {
                app.settings.update_channel = channel;
                app.save_settings();
                app.check_for_updates(false);
            }
        });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if secondary_button(ui, t!(app.locale, "Check for updates now"), palette) {
                app.check_for_updates(false);
            }
            if secondary_button(ui, t!(app.locale, "Release notes"), palette)
                && let Err(error) = open::that(RELEASES_URL)
            {
                log::warn!("could not open releases: {error}");
            }
            if secondary_button(ui, t!(app.locale, "Source repository"), palette)
                && let Err(error) = open::that(REPOSITORY_URL)
            {
                log::warn!("could not open repository: {error}");
            }
        });

        // Per-run logs (fastframe-log): the current run and the previous one,
        // which fastframe-log keeps beside it after a forced restart so a
        // frozen/killed run still has its log for a bug report.
        ui.add_space(14.0);
        section_header(ui, palette, t!(app.locale, "Logs"));
        card(palette).show(ui, |ui| {
            ui.label(
                RichText::new(t!(
                    app.locale,
                    "Plain-text per-run logs, useful when reporting a problem."
                ))
                .color(palette.secondary)
                .size(12.5),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                widgets::no_wrap_scope(ui, |ui| {
                    if secondary_button(ui, t!(app.locale, "Open the current log"), palette) {
                        let path = app.paths.log_file();
                        if let Err(error) = open::that(if path.exists() {
                            path
                        } else {
                            app.paths.logs_dir()
                        }) {
                            log::warn!("could not open the log file: {error}");
                        }
                    }
                    let previous = app.paths.previous_log_file();
                    if previous.exists()
                        && secondary_button(
                            ui,
                            t!(app.locale, "Open the previous run's log"),
                            palette,
                        )
                        && let Err(error) = open::that(previous)
                    {
                        log::warn!("could not open the previous log: {error}");
                    }
                });
            });
        });

        if app.update.last_manifest.is_some()
            && app.update.progress.is_none()
            && primary_button(ui, t!(app.locale, "Download and install"), palette, true)
        {
            app.start_update_download();
        }

        if let Some((received, total)) = app.update.progress {
            ui.add_space(10.0);
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

        ui.add_space(12.0);
        ui.label(
            RichText::new(t!(
                app.locale,
                "Native Rust client for the Atlas Commerce platform. No telemetry."
            ))
            .color(palette.dim)
            .size(11.5),
        );
    });
}

// ── Shared bits ────────────────────────────────────────────────────────────

/// Keeps the YYYY-MM-DD part of the worker's datetime strings.
fn short_date(value: &str) -> String {
    value.chars().take(10).collect()
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
