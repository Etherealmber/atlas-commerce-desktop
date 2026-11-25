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
    avatar, card, danger_button, error_banner, icon, info_banner, labelled_field, pill,
    primary_button, secondary_button, section_header, segmented, setting_row, switch_row,
};
use crate::{t, widgets};

pub(crate) const REPOSITORY_URL: &str = "https://github.com/Etherealmber/atlas-commerce-desktop";
pub(crate) const RELEASES_URL: &str =
    "https://github.com/Etherealmber/atlas-commerce-desktop/releases";
const POLL_CHOICES: [u64; 4] = [30, 60, 120, 300];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsPanel {
    Profile,
    #[default]
    General,
    Appearance,
    Agent,
    Attendance,
    Server,
    Integrations,
    Security,
    Updates,
}

const PANEL_ID: &str = "settings-active-panel";
const SEARCH_ID: &str = "settings-navigation-search";
const SEARCH_TARGET_ID: &str = "settings-search-target";
const SEARCH_SCROLL_ID: &str = "settings-search-scroll-pending";
pub(super) const SETTINGS_OPEN_ID: &str = "settings-dialog-session-open";

// Only index controls that exist and have a corresponding scroll anchor.
// This avoids presenting search results for settings the app cannot edit.
const SEARCHABLE_SETTINGS: &[(SettingsPanel, &str)] = &[
    (
        SettingsPanel::General,
        "Keep running in the notification area when closed",
    ),
    (SettingsPanel::General, "Start at login"),
    (
        SettingsPanel::General,
        "Native notifications for stock alerts",
    ),
    (SettingsPanel::General, "Reduce animations"),
    (SettingsPanel::General, "Refresh interval"),
    (SettingsPanel::Appearance, "Interface size"),
    (SettingsPanel::Appearance, "Language"),
    (SettingsPanel::Appearance, "Theme"),
    (SettingsPanel::Attendance, "Enable attendance"),
    (SettingsPanel::Updates, "Check for updates automatically"),
    (SettingsPanel::Updates, "Channel"),
    (SettingsPanel::Updates, "Version history"),
];

fn settings_search_matches(needle: &str, source: &str, translated: &str) -> bool {
    let needle = crate::shortcuts::normalize(needle.trim());
    !needle.is_empty()
        && (crate::shortcuts::normalize(source).contains(&needle)
            || crate::shortcuts::normalize(translated).contains(&needle))
}

fn search_anchor(ui: &mut egui::Ui, palette: &Palette, locale: Locale, key: &'static str) {
    let target = ui
        .ctx()
        .data(|data| data.get_temp::<String>(egui::Id::new(SEARCH_TARGET_ID)));
    if target.as_deref() == Some(key) {
        let response = ui.label(
            RichText::new(crate::i18n::gettext(locale, key))
                .strong()
                .color(palette.copper),
        );
        let should_scroll = ui.ctx().data(|data| {
            data.get_temp::<bool>(egui::Id::new(SEARCH_SCROLL_ID))
                .unwrap_or(false)
        });
        if should_scroll {
            response.scroll_to_me(Some(egui::Align::Center));
            ui.ctx()
                .data_mut(|data| data.insert_temp(egui::Id::new(SEARCH_SCROLL_ID), false));
        }
    }
}

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, ctx: &Context, palette: &Palette) {
    let mut active = ctx
        .data(|data| data.get_temp::<SettingsPanel>(egui::Id::new(PANEL_ID)))
        .unwrap_or_default();
    let previous_panel = active;
    let first_frame = !ctx.data(|data| {
        data.get_temp::<bool>(egui::Id::new(SETTINGS_OPEN_ID))
            .unwrap_or(false)
    });
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(SETTINGS_OPEN_ID), true));
    match app.settings_focus {
        SettingsFocus::Profile => active = SettingsPanel::Profile,
        SettingsFocus::Integrations if is_admin(app) => active = SettingsPanel::Integrations,
        SettingsFocus::Attendance if is_admin(app) => active = SettingsPanel::Attendance,
        SettingsFocus::About => active = SettingsPanel::Updates,
        _ => {}
    }
    app.settings_focus = SettingsFocus::General;
    if matches!(
        active,
        SettingsPanel::Integrations | SettingsPanel::Attendance
    ) && !is_admin(app)
    {
        active = SettingsPanel::General;
    }
    if active == SettingsPanel::Attendance
        && app.attendance.data.is_none()
        && app.attendance.last_error.is_none()
        && !app.attendance.loading
        && !app.attendance_action.loading
    {
        app.load_attendance();
    }

    let width = ui.available_width();
    let height = ui.available_height().max(1.0);
    let left_width = 252.0_f32.min(width);
    if width >= 620.0 {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(left_width);
                ui.set_height(height);
                settings_navigation(app, ui, ctx, palette, &mut active);
            });
            let x = ui.cursor().left() + 5.0;
            let top = ui.cursor().top();
            ui.painter().line_segment(
                [egui::pos2(x, top), egui::pos2(x, top + height)],
                egui::Stroke::new(1.0, palette.outline),
            );
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.set_width((width - left_width - 20.0).max(1.0));
                ui.set_height(height);
                settings_content(app, ui, palette, active);
            });
        });
    } else {
        // Keep the close control and content accessible on small windows.
        egui::ScrollArea::vertical()
            .id_salt("settings-narrow-navigation")
            .max_height((height * 0.36).max(1.0))
            .show(ui, |ui| {
                settings_navigation(app, ui, ctx, palette, &mut active)
            });
        ui.add_space(8.0);
        settings_content(app, ui, palette, active);
    }
    // Entering Updates requests a fresh manifest, subject to the same global
    // cooldown as manual and scheduled checks. It never downloads an artifact.
    // The offline visual fixture has no API client and does not hit the network.
    if active == SettingsPanel::Updates
        && (first_frame || previous_panel != active)
        && app.client.is_some()
        && app.update.progress.is_none()
    {
        app.check_for_updates(false);
    }
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(PANEL_ID), active));
}

fn settings_navigation(
    app: &mut AtlasApp,
    ui: &mut egui::Ui,
    ctx: &Context,
    palette: &Palette,
    active: &mut SettingsPanel,
) {
    let mut query = ctx
        .data(|data| data.get_temp::<String>(egui::Id::new(SEARCH_ID)))
        .unwrap_or_default();
    let search_width = ui.available_width();
    widgets::search_field(
        ui,
        &mut query,
        t!(app.locale, "Search settings"),
        search_width,
    );
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(SEARCH_ID), query.clone()));
    ui.add_space(10.0);
    let name = app
        .session
        .as_ref()
        .map(|session| {
            if session.display_name.is_empty() {
                session.username.as_str()
            } else {
                session.display_name.as_str()
            }
        })
        .unwrap_or("");
    let initial = name
        .chars()
        .next()
        .unwrap_or('A')
        .to_uppercase()
        .collect::<String>();
    let account = card(palette)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                avatar(
                    ui,
                    app.avatar_texture_uri.as_deref(),
                    &initial,
                    36.0,
                    palette.surface_active,
                    palette.text,
                    palette.outline,
                );
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width());
                    ui.add(
                        egui::Label::new(RichText::new(name).strong().color(palette.text))
                            .truncate(),
                    );
                    let host = app
                        .session
                        .as_ref()
                        .map(|s| atlas_core::ServerUrl::host(&s.server_url))
                        .unwrap_or_default();
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("{} · {}", role_label(app), host))
                                .small()
                                .color(palette.secondary),
                        )
                        .truncate(),
                    );
                });
            });
        });
    if account.response.interact(egui::Sense::click()).clicked() {
        *active = SettingsPanel::Profile;
    }
    ui.add_space(6.0);
    let status = if app.update.checking {
        t!(app.locale, "Checking for updates…")
    } else if app.update.last_manifest.is_some() || app.update.ready_version.is_some() {
        t!(app.locale, "Update available")
    } else if app.update.up_to_date_at.is_some() && app.update.error.is_none() {
        t!(app.locale, "You are on the latest version.")
    } else {
        t!(app.locale, "Updates and about")
    };
    let update_card = card(palette)
        .fill(if *active == SettingsPanel::Updates {
            palette.surface_active
        } else {
            palette.surface
        })
        .stroke(egui::Stroke::new(
            1.0,
            if *active == SettingsPanel::Updates {
                palette.copper_border
            } else {
                palette.outline
            },
        ))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let mark_pos = ui.next_widget_position();
                crate::branding::mark(ui, palette, 36.0);
                if app.update.up_to_date_at.is_some()
                    && app.update.error.is_none()
                    && app.update.last_manifest.is_none()
                    && app.update.ready_version.is_none()
                {
                    let center = mark_pos + egui::vec2(31.0, 31.0);
                    ui.painter()
                        .circle_filled(center, 7.0, egui::Color32::from_rgb(34, 197, 94));
                    ui.painter().line_segment(
                        [
                            center + egui::vec2(-3.0, 0.0),
                            center + egui::vec2(-1.0, 2.0),
                        ],
                        egui::Stroke::new(1.5, egui::Color32::WHITE),
                    );
                    ui.painter().line_segment(
                        [
                            center + egui::vec2(-1.0, 2.0),
                            center + egui::vec2(3.0, -2.0),
                        ],
                        egui::Stroke::new(1.5, egui::Color32::WHITE),
                    );
                }
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width());
                    ui.add(
                        egui::Label::new(
                            RichText::new(status)
                                .strong()
                                .color(palette.text)
                                .size(12.0),
                        )
                        .truncate(),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("v{} · Desktop", env!("CARGO_PKG_VERSION")))
                                .small()
                                .color(palette.secondary),
                        )
                        .truncate(),
                    );
                });
            });
        });
    if update_card
        .response
        .interact(egui::Sense::click())
        .clicked()
    {
        *active = SettingsPanel::Updates;
    }
    let needle = crate::shortcuts::normalize(query.trim());
    if !needle.is_empty() {
        egui::ScrollArea::vertical()
            .id_salt("settings-search-results")
            .max_height(ui.available_height().max(1.0))
            .show(ui, |ui| {
                let mut count = 0;
                for (panel, key) in [
                    (SettingsPanel::Profile, "Profile"),
                    (SettingsPanel::General, "Application"),
                    (SettingsPanel::Appearance, "Appearance"),
                    (SettingsPanel::Agent, "AI Agent"),
                    (SettingsPanel::Attendance, "Attendance settings"),
                    (SettingsPanel::Server, "Server"),
                    (SettingsPanel::Integrations, "Integrations"),
                    (SettingsPanel::Security, "Security"),
                    (SettingsPanel::Updates, "Updates and about"),
                ] {
                    if matches!(
                        panel,
                        SettingsPanel::Integrations | SettingsPanel::Attendance
                    ) && !is_admin(app)
                    {
                        continue;
                    }
                    let label = crate::i18n::gettext(app.locale, key);
                    if settings_search_matches(&needle, key, &label) {
                        count += 1;
                        if secondary_button(ui, label, palette) {
                            *active = panel;
                            ctx.data_mut(|data| {
                                data.insert_temp(egui::Id::new(SEARCH_TARGET_ID), String::new());
                            });
                        }
                    }
                }
                for &(panel, key) in SEARCHABLE_SETTINGS {
                    if panel == SettingsPanel::Attendance && !is_admin(app) {
                        continue;
                    }
                    let label = crate::i18n::gettext(app.locale, key);
                    if !settings_search_matches(&needle, key, &label) {
                        continue;
                    }
                    count += 1;
                    if secondary_button(ui, label, palette) {
                        *active = panel;
                        ctx.data_mut(|data| {
                            data.insert_temp(egui::Id::new(SEARCH_TARGET_ID), key.to_owned());
                            data.insert_temp(egui::Id::new(SEARCH_SCROLL_ID), true);
                        });
                    }
                }
                if count == 0 {
                    ui.label(
                        RichText::new(t!(app.locale, "No matching settings."))
                            .color(palette.secondary),
                    );
                }
            });
        return;
    }
    let groups: &[(&str, &[(SettingsPanel, &str)])] = &[
        (
            "Your account",
            &[
                (SettingsPanel::Profile, "Profile"),
                (SettingsPanel::Agent, "AI Agent"),
            ],
        ),
        (
            "Your workspace",
            &[
                (SettingsPanel::General, "Application"),
                (SettingsPanel::Appearance, "Appearance"),
            ],
        ),
        (
            "Company",
            &[(SettingsPanel::Attendance, "Attendance settings")],
        ),
        (
            "Connections and security",
            &[
                (SettingsPanel::Server, "Server"),
                (SettingsPanel::Integrations, "Integrations"),
                (SettingsPanel::Security, "Security"),
            ],
        ),
        (
            "Maintenance",
            &[(SettingsPanel::Updates, "Updates and about")],
        ),
    ];
    egui::ScrollArea::vertical()
        .id_salt("settings-navigation-list")
        .max_height(ui.available_height().max(1.0))
        .show(ui, |ui| {
            for (group, entries) in groups {
                let visible: Vec<_> = entries
                    .iter()
                    .filter(|(panel, label)| {
                        (!matches!(
                            *panel,
                            SettingsPanel::Integrations | SettingsPanel::Attendance
                        ) || is_admin(app))
                            && (needle.is_empty()
                                || crate::i18n::gettext(app.locale, label)
                                    .to_lowercase()
                                    .contains(&needle)
                                || crate::i18n::gettext(app.locale, group)
                                    .to_lowercase()
                                    .contains(&needle)
                                || crate::i18n::gettext(app.locale, panel_details(*panel).1)
                                    .to_lowercase()
                                    .contains(&needle))
                    })
                    .collect();
                if visible.is_empty() {
                    continue;
                }
                ui.add_space(12.0);
                ui.label(
                    RichText::new(crate::i18n::gettext(app.locale, group).to_uppercase())
                        .size(10.5)
                        .color(palette.secondary),
                );
                for (panel, label) in visible {
                    let (glyph, subtitle) = panel_details(*panel);
                    let selected = *active == *panel;
                    let row = egui::Frame::NONE
                        .fill(if selected {
                            palette.surface_active
                        } else {
                            palette.surface
                        })
                        .stroke(egui::Stroke::new(1.0, palette.outline))
                        .corner_radius(12.0)
                        .inner_margin(egui::Margin::same(9))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                icon(ui, glyph, palette.secondary, 18.0);
                                ui.vertical(|ui| {
                                    ui.set_max_width(ui.available_width());
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(crate::i18n::gettext(app.locale, label))
                                                .strong()
                                                .color(palette.text)
                                                .size(12.5),
                                        )
                                        .truncate(),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(crate::i18n::gettext(
                                                app.locale, subtitle,
                                            ))
                                            .color(palette.secondary)
                                            .size(11.0),
                                        )
                                        .truncate(),
                                    );
                                });
                            });
                        });
                    if row.response.interact(egui::Sense::click()).clicked() {
                        *active = *panel;
                    }
                    ui.add_space(3.0);
                }
            }
        });
}

fn panel_details(panel: SettingsPanel) -> (Icon, &'static str) {
    match panel {
        SettingsPanel::Profile => (Icon::User, "Account and profile"),
        SettingsPanel::General => (Icon::Settings, "Language, startup and notifications"),
        SettingsPanel::Appearance => (Icon::Palette, "Theme, scale and motion"),
        SettingsPanel::Agent => (Icon::Bot, "Assistant preferences"),
        SettingsPanel::Attendance => (Icon::Clock, "Company attendance module"),
        SettingsPanel::Server => (Icon::Server, "Server and connection"),
        SettingsPanel::Integrations => (Icon::KeyRound, "API keys and integrations"),
        SettingsPanel::Security => (Icon::ShieldCheck, "Trusted devices and security"),
        SettingsPanel::Updates => (Icon::Download, "Updates and release notes"),
    }
}

fn settings_content(
    app: &mut AtlasApp,
    ui: &mut egui::Ui,
    palette: &Palette,
    active: SettingsPanel,
) {
    let (title, description) = match active {
        SettingsPanel::Profile => ("Profile", "Profile and access to your account."),
        SettingsPanel::General => (
            "Application",
            "Personalize the app and how it behaves on this computer.",
        ),
        SettingsPanel::Appearance => (
            "Appearance",
            "Personalize the app and how it behaves on this computer.",
        ),
        SettingsPanel::Agent => ("AI Agent", "Your workspace"),
        SettingsPanel::Attendance => (
            "Attendance settings",
            "Configure attendance for the company.",
        ),
        SettingsPanel::Server => (
            "Server",
            "Review your server, integrations and connected devices.",
        ),
        SettingsPanel::Integrations => (
            "Integrations",
            "Review your server, integrations and connected devices.",
        ),
        SettingsPanel::Security => (
            "Security",
            "Review your server, integrations and connected devices.",
        ),
        SettingsPanel::Updates => (
            "Updates and about",
            "Updates and information about Atlas Commerce.",
        ),
    };
    ui.horizontal(|ui| {
        egui::Frame::NONE
            .fill(palette.surface)
            .stroke(egui::Stroke::new(1.0, palette.outline))
            .corner_radius(255.0)
            .inner_margin(egui::Margin::symmetric(14, 7))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(crate::i18n::gettext(app.locale, title))
                        .strong()
                        .color(palette.text),
                );
            });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_circle_button(ui, palette, Icon::X, true)
                .on_hover_text(t!(app.locale, "Close"))
                .clicked()
            {
                app.settings_dialog_open = false;
            }
        });
    });
    ui.add_space(12.0);
    let remaining = ui.available_height().max(1.0);
    egui::ScrollArea::vertical()
        .id_salt(("settings-content", active as u8))
        .max_height(remaining)
        .show(ui, |ui| {
            ui.set_max_width(680.0);
            group_heading(
                ui,
                palette,
                crate::i18n::gettext(app.locale, title),
                crate::i18n::gettext(app.locale, description),
            );
            match active {
                SettingsPanel::Profile => profile_section(app, ui, palette),
                SettingsPanel::General => application_section(app, ui, palette),
                SettingsPanel::Appearance => appearance_section(app, ui, palette),
                SettingsPanel::Agent => assist_section(app, ui, palette),
                SettingsPanel::Attendance if is_admin(app) => attendance_section(app, ui, palette),
                SettingsPanel::Attendance => {}
                SettingsPanel::Server => servers_section(app, ui, palette),
                SettingsPanel::Integrations if is_admin(app) => {
                    integrations_section(app, ui, palette)
                }
                SettingsPanel::Integrations => {}
                SettingsPanel::Security => security_section(app, ui, palette),
                SettingsPanel::Updates => updates_section(app, ui, palette),
            }
        });
}

fn attendance_section(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    search_anchor(ui, palette, app.locale, "Enable attendance");
    let disabled = app
        .attendance
        .last_error
        .as_ref()
        .is_some_and(|error| error.code.as_deref() == Some("ATTENDANCE_DISABLED"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(t!(app.locale, "Attendance module"))
                .strong()
                .color(palette.text),
        );
        if app.attendance.loading {
            ui.label(t!(app.locale, "Checking attendance status…"));
        } else if disabled {
            ui.label(
                RichText::new(t!(app.locale, "Attendance is disabled.")).color(palette.secondary),
            );
            if primary_button(
                ui,
                t!(app.locale, "Enable attendance"),
                palette,
                !app.attendance_action.loading && app.attendance_action.error.is_none(),
            ) {
                app.confirm_dialog = Some(Confirm {
                    title: t!(app.locale, "Enable attendance?").into_owned(),
                    body: t!(
                        app.locale,
                        "This enables attendance for the entire company."
                    )
                    .into_owned(),
                    confirm_label: t!(app.locale, "Enable now").into_owned(),
                    danger: false,
                    action: ConfirmAction::EnableAttendance,
                });
            }
        } else if let Some(error) = &app.attendance.error {
            error_banner(ui, error, palette);
        } else if app.attendance.data.is_some() {
            ui.label(
                RichText::new(t!(app.locale, "Attendance is enabled."))
                    .color(palette.finance_positive),
            );
        } else {
            ui.label(
                RichText::new(t!(app.locale, "Attendance status is unknown."))
                    .color(palette.secondary),
            );
        }
        if let Some(error) = &app.attendance_action.error {
            error_banner(ui, error, palette);
            ui.label(
                RichText::new(t!(
                    app.locale,
                    "Refresh before retrying: the server may have saved the change."
                ))
                .color(palette.secondary),
            );
        }
        if secondary_button(ui, t!(app.locale, "Refresh"), palette) {
            app.load_attendance();
        }
    });
}

fn group_heading(
    ui: &mut egui::Ui,
    palette: &Palette,
    title: impl AsRef<str>,
    description: impl AsRef<str>,
) {
    ui.label(
        RichText::new(title.as_ref())
            .color(palette.text)
            .strong()
            .size(18.0),
    );
    ui.label(
        RichText::new(description.as_ref())
            .color(palette.secondary)
            .size(12.5),
    );
    ui.add_space(12.0);
}

fn is_admin(app: &AtlasApp) -> bool {
    matches!(
        app.session.as_ref().map(|session| session.role.as_str()),
        Some("owner" | "admin")
    )
}

pub(super) fn role_label(app: &AtlasApp) -> String {
    let role = app
        .session
        .as_ref()
        .map(|session| session.role.to_ascii_lowercase())
        .unwrap_or_default();
    match role.as_str() {
        "owner" => t!(app.locale, "Owner").into_owned(),
        "admin" => t!(app.locale, "Administrator").into_owned(),
        "secretaire" => t!(app.locale, "Secretary").into_owned(),
        "stock" => t!(app.locale, "Warehouse keeper").into_owned(),
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
            let avatar_url = app.avatar_texture_uri.as_deref();
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

        search_anchor(ui, palette, app.locale, "Keep running in the notification area when closed");
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
        search_anchor(ui, palette, app.locale, "Start at login");
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
        search_anchor(ui, palette, app.locale, "Native notifications for stock alerts");
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

        #[cfg(target_os = "windows")]
        {
            ui.add_space(10.0);
            if switch_row(
                ui,
                palette,
                t!(app.locale, "Windows material"),
                Some(&t!(app.locale, "Use Mica or Acrylic when Windows permits transparency; otherwise use a solid background.")),
                &mut app.settings.windows_material,
                app.settings.reduced_motion,
            ) {
                app.save_settings();
            }
        }

        ui.add_space(10.0);
        search_anchor(ui, palette, app.locale, "Reduce animations");
        let reduced_motion = app.settings.reduced_motion;
        if switch_row(
            ui,
            palette,
            t!(app.locale, "Reduce animations"),
            Some(&t!(app.locale, "Remove transition and entrance effects.")),
            &mut app.settings.reduced_motion,
            reduced_motion,
        ) {
            app.save_settings();
        }

        ui.add_space(12.0);
        search_anchor(ui, palette, app.locale, "Refresh interval");
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

        search_anchor(ui, palette, app.locale, "Interface size");
        ui.label(
            RichText::new(t!(app.locale, "Interface size"))
                .color(palette.text)
                .strong()
                .size(13.5),
        );
        ui.label(
            RichText::new(t!(
                app.locale,
                "Adjust the size of text and controls on this device."
            ))
            .color(palette.dim)
            .size(11.5),
        );
        ui.add_space(6.0);
        let options = [
            ("90 %", 90_u16),
            ("100 %", 100),
            ("110 %", 110),
            ("125 %", 125),
        ];
        if let Some(scale) = segmented(ui, palette, &options, Some(app.settings.ui_scale_percent)) {
            app.settings.ui_scale_percent = scale;
            app.save_settings();
        }
        ui.add_space(12.0);
        search_anchor(ui, palette, app.locale, "Language");
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
        search_anchor(ui, palette, app.locale, "Theme");
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
pub(super) fn draw_created_key_modal(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let Some(created) = app.created_key.clone() else {
        return;
    };
    let screen = ui.ctx().input(|input| input.viewport_rect());
    let width = 480.0;
    let height = 250.0;
    let rect = egui::Rect::from_center_size(screen.center(), egui::vec2(width, height));
    if widgets::scrim_clicked(ui, screen, rect, egui::Id::new("created-key-scrim")) {
        app.created_key = None;
        return;
    }
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height((rect.height() - 46.0).max(1.0))
                .show(ui, |ui| {
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
    });
}

// ── Servers ────────────────────────────────────────────────────────────────

fn servers_section(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    section_header(ui, palette, t!(app.locale, "Servers"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        // Installed copies fix their server: the installer wrote it and the
        // only way to change it is a reinstall. Settings stays honest about
        // that instead of offering a change that a guard would refuse.
        if app.server_store.is_locked() {
            let entry = app.server_store.saved.first().cloned();
            ui.horizontal(|ui| {
                icon(ui, Icon::Lock, palette.secondary, 18.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(
                            entry
                                .as_ref()
                                .map(|entry| entry.label.clone())
                                .unwrap_or_else(|| t!(app.locale, "Server").into_owned()),
                        )
                        .color(palette.text)
                        .strong()
                        .size(13.5),
                    );
                    ui.label(
                        RichText::new(
                            entry
                                .as_ref()
                                .map(|entry| entry.url.clone())
                                .unwrap_or_else(|| "—".to_owned()),
                        )
                        .color(palette.dim)
                        .size(11.5),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    pill(
                        ui,
                        t!(app.locale, "Fixed"),
                        palette.accent.gamma_multiply(0.18),
                        palette.accent,
                    );
                });
            });
            ui.add_space(10.0);
            ui.label(
                RichText::new(t!(
                    app.locale,
                    "This server was chosen during installation. To use another one, uninstall the app and install it again; it cannot be changed from Settings."
                ))
                .color(palette.secondary)
                .size(12.5),
            );
            return;
        }
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
        egui::CollapsingHeader::new(t!(app.locale, "This device"))
            .id_salt("local-device-details")
            .show(ui, |ui| {
                ui.label(t!(app.locale, "Device fingerprint"));
                let fingerprint = app.identity.fingerprint();
                ui.monospace(fingerprint);
                if secondary_button(ui, t!(app.locale, "Copy"), palette) {
                    ui.ctx().copy_text(fingerprint.to_owned());
                }
            });

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
        } else if let Some(error) = app.devices.error.clone() {
            error_banner(ui, &error, palette);
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
                let name = if device.label.trim().is_empty() {
                    t!(app.locale, "Device").into_owned()
                } else {
                    device.label.clone()
                };
                let title = if device.current {
                    format!("{} · {}", name, t!(app.locale, "This device"))
                } else {
                    name
                };
                egui::CollapsingHeader::new(title)
                    .id_salt(("trusted-device", &device.fingerprint))
                    .show(ui, |ui| {
                        ui.label(format!(
                            "{}: {}",
                            t!(app.locale, "Device fingerprint"),
                            device.revoke_id()
                        ));
                        ui.label(format!(
                            "IP : {}",
                            device
                                .last_ip
                                .as_deref()
                                .filter(|v| !v.is_empty())
                                .unwrap_or("—")
                        ));
                        ui.label(t!(app.locale, "City and country unavailable"));
                        if secondary_button(ui, t!(app.locale, "Copy"), palette) {
                            ui.ctx().copy_text(device.revoke_id().to_owned());
                        }
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
                ui.separator();
            }
        }
    });
}

// ── Self-update and about ──────────────────────────────────────────────────

fn updates_section(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let channel = match app.settings.update_channel {
        UpdateChannel::Stable => t!(app.locale, "Stable"),
        UpdateChannel::Prerelease => t!(app.locale, "Pre-release"),
    };
    let title = if app.update.checking {
        t!(app.locale, "Checking for updates…").into_owned()
    } else if let Some(version) = &app.update.ready_version {
        format!(
            "{} · v{version}",
            t!(app.locale, "Update downloaded. Restart to install.")
        )
    } else if let Some(manifest) = &app.update.last_manifest {
        format!(
            "{} · v{}",
            t!(app.locale, "Update available"),
            manifest.version
        )
    } else if app.update.error.is_none() && app.update.up_to_date_at.is_some() {
        t!(app.locale, "You are on the latest version.").into_owned()
    } else {
        t!(app.locale, "Updates and about").into_owned()
    };
    card(palette)
        .inner_margin(egui::Margin::same(24))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                if app.update.checking && !app.settings.reduced_motion {
                    super::spinner(ui, palette);
                } else if (app.update.up_to_date_at.is_some()
                    && app.update.error.is_none()
                    && app.update.last_manifest.is_none()
                    && app.update.ready_version.is_none())
                    || app.update.last_manifest.is_some()
                    || app.update.ready_version.is_some()
                {
                    let current =
                        app.update.last_manifest.is_none() && app.update.ready_version.is_none();
                    let (badge, _) =
                        ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::hover());
                    ui.painter().circle_filled(
                        badge.center(),
                        24.0,
                        if current {
                            egui::Color32::from_rgb(34, 197, 94)
                        } else {
                            palette.accent
                        },
                    );
                    widgets::overlay_at(
                        ui,
                        egui::Rect::from_center_size(badge.center(), egui::vec2(24.0, 24.0)),
                        |ui| {
                            icon(
                                ui,
                                if current { Icon::Check } else { Icon::Download },
                                if current {
                                    egui::Color32::WHITE
                                } else {
                                    palette.on_accent
                                },
                                24.0,
                            );
                        },
                    );
                } else {
                    crate::branding::mark(ui, palette, 48.0);
                }
                ui.add_space(8.0);
                ui.label(RichText::new(title).color(palette.text).strong().size(16.0));
                ui.label(
                    RichText::new(format!("v{} · {}", env!("CARGO_PKG_VERSION"), channel))
                        .color(palette.secondary)
                        .size(12.0),
                );
                ui.add_space(10.0);
                if app.update.ready_version.is_some() {
                    if primary_button(ui, t!(app.locale, "Restart and install"), palette, true) {
                        app.update.dialog_open = true;
                    }
                } else if app.update.last_manifest.is_some() && app.update.progress.is_none() {
                    if primary_button(
                        ui,
                        t!(app.locale, "Download and install update"),
                        palette,
                        true,
                    ) {
                        app.update.dialog_open = true;
                    }
                } else {
                    let remaining = app
                        .update
                        .last_check_started
                        .map_or(std::time::Duration::ZERO, |(at, _)| {
                            crate::state::update_check_cooldown_remaining(at.elapsed())
                        });
                    let cooling_down = !remaining.is_zero();
                    if cooling_down {
                        ui.ctx().request_repaint_after(remaining);
                    }
                    ui.add_enabled_ui(
                        !app.update.checking && app.update.progress.is_none() && !cooling_down,
                        |ui| {
                            if secondary_button(
                                ui,
                                t!(app.locale, "Check for updates now"),
                                palette,
                            ) {
                                app.check_for_updates(false);
                            }
                        },
                    );
                }
            });
            if let Some(manifest) = &app.update.last_manifest {
                if !manifest.published_at.is_empty() {
                    ui.label(RichText::new(&manifest.published_at).color(palette.secondary));
                }
                widgets::update_metadata(
                    ui,
                    app.update.desktop_release.as_ref(),
                    palette,
                    app.locale,
                );
            }
            if let Some((received, total)) = app.update.progress {
                ui.add_space(8.0);
                let progress = if total > 0 {
                    (received as f32 / total as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                ui.label(RichText::new(&app.update.stage).color(palette.secondary));
                ui.add(egui::ProgressBar::new(progress).show_percentage());
            }
            if let Some(error) = app.update.error.clone() {
                ui.add_space(8.0);
                error_banner(ui, &error, palette);
            }
        });

    ui.add_space(12.0);
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        search_anchor(ui, palette, app.locale, "Check for updates automatically");
        if switch_row(
            ui,
            palette,
            t!(app.locale, "Check for updates automatically"),
            Some(&t!(
                app.locale,
                "Checks run on startup and periodically; downloads are always manual."
            )),
            &mut app.settings.update_check_on_start,
            app.settings.reduced_motion,
        ) {
            app.save_settings();
            app.schedule_update_checks();
            if app.settings.update_check_on_start {
                app.maybe_auto_check_updates();
            }
        }
        if app.settings.update_check_on_start {
            setting_row(ui, palette, t!(app.locale, "Check every"), None, |ui| {
                let six = t!(app.locale, "6 hours");
                let twelve = t!(app.locale, "12 hours");
                let day = t!(app.locale, "24 hours");
                let options = [
                    (six.as_ref(), 6_u64),
                    (twelve.as_ref(), 12),
                    (day.as_ref(), 24),
                ];
                if let Some(hours) = segmented(
                    ui,
                    palette,
                    &options,
                    Some(app.settings.update_check_interval_hours),
                ) {
                    app.settings.update_check_interval_hours = hours;
                    app.save_settings();
                    app.schedule_update_checks();
                }
            });
        }
        search_anchor(ui, palette, app.locale, "Channel");
        setting_row(ui, palette, t!(app.locale, "Channel"), None, |ui| {
            let stable = t!(app.locale, "Stable");
            let prerelease = t!(app.locale, "Pre-release");
            let options = [
                (stable.as_ref(), UpdateChannel::Stable),
                (prerelease.as_ref(), UpdateChannel::Prerelease),
            ];
            let selection = ui
                .add_enabled_ui(
                    !app.update.checking && app.update.progress.is_none(),
                    |ui| segmented(ui, palette, &options, Some(app.settings.update_channel)),
                )
                .inner;
            if let Some(channel) = selection
                && app.settings.update_channel != channel
            {
                app.settings.update_channel = channel;
                app.update.last_manifest = None;
                app.update.desktop_release = None;
                app.update.release_notes = None;
                app.update.up_to_date_at = None;
                app.update.ready_version = None;
                app.save_settings();
                let remaining = app
                    .update
                    .last_check_started
                    .map_or(std::time::Duration::ZERO, |(at, _)| {
                        crate::state::update_check_cooldown_remaining(at.elapsed())
                    });
                if remaining.is_zero() {
                    app.check_for_updates(false);
                } else {
                    app.update.error = Some(
                        t!(
                            app.locale,
                            "Update checks are temporarily limited. Try again in one minute."
                        )
                        .into_owned(),
                    );
                    if app.settings.update_check_on_start {
                        app.update.next_check_at = Some(std::time::Instant::now() + remaining);
                    }
                }
            }
        });
    });

    // Release notes are supplied by the actual release feed, not a hard-coded
    // list of features which may not exist in the running build.
    if let Some(manifest) = app
        .update
        .last_manifest
        .as_ref()
        .or(app.update.release_notes.as_ref())
        && !manifest.notes.trim().is_empty()
    {
        ui.add_space(18.0);
        section_header(
            ui,
            palette,
            format!(
                "{} · v{}",
                t!(app.locale, "Release notes"),
                manifest.version
            ),
        );
        card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::release_notes(ui, &manifest.notes, palette, 300.0);
        });
    }

    ui.add_space(18.0);
    section_header(ui, palette, t!(app.locale, "About your version"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        version_row(
            ui,
            palette,
            &t!(app.locale, "Version"),
            env!("CARGO_PKG_VERSION"),
        );
        version_row(ui, palette, &t!(app.locale, "Channel"), &channel);
        version_row(
            ui,
            palette,
            &t!(app.locale, "Platform"),
            &format!("{} · {}", std::env::consts::OS, std::env::consts::ARCH),
        );
        if let Some(first) = app.settings.first_observed_version.as_deref() {
            version_row(
                ui,
                palette,
                &t!(app.locale, "First version recorded"),
                first,
            );
        }
    });

    ui.add_space(18.0);
    search_anchor(ui, palette, app.locale, "Version history");
    section_header(ui, palette, t!(app.locale, "Version history"));
    ui.label(
        RichText::new(t!(
            app.locale,
            "Versions observed on this device; earlier installations may not appear."
        ))
        .color(palette.secondary)
        .size(11.5),
    );
    if app.settings.update_history.is_empty() {
        ui.label(
            RichText::new(t!(app.locale, "No version history recorded.")).color(palette.secondary),
        );
    } else {
        for (index, entry) in app.settings.update_history.iter().rev().enumerate() {
            ui.add_space(6.0);
            card(palette)
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(format!("Atlas Commerce {}", entry.version))
                                .strong()
                                .color(palette.text),
                        );
                        if index == 0 && entry.version == env!("CARGO_PKG_VERSION") {
                            pill(
                                ui,
                                t!(app.locale, "Current"),
                                palette.finance_positive_soft,
                                palette.finance_positive,
                            );
                        }
                        if entry.observed_unix > 0
                            && let Ok(seconds) = i64::try_from(entry.observed_unix)
                            && let Some(date) =
                                chrono::DateTime::<chrono::Utc>::from_timestamp(seconds, 0)
                        {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(date.format("%Y-%m-%d").to_string())
                                            .color(palette.secondary),
                                    );
                                },
                            );
                        }
                    });
                });
        }
    }

    ui.add_space(14.0);
    ui.horizontal_wrapped(|ui| {
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
    ui.add_space(14.0);
    section_header(ui, palette, t!(app.locale, "Logs"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(t!(
                app.locale,
                "Plain-text per-run logs, useful when reporting a problem."
            ))
            .color(palette.secondary)
            .size(12.5),
        );
        ui.horizontal_wrapped(|ui| {
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
                && secondary_button(ui, t!(app.locale, "Open the previous run's log"), palette)
                && let Err(error) = open::that(previous)
            {
                log::warn!("could not open the previous log: {error}");
            }
        });
    });
}

fn version_row(ui: &mut egui::Ui, palette: &Palette, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [130.0, 22.0],
            egui::Label::new(RichText::new(label).color(palette.secondary)),
        );
        ui.label(RichText::new(value).color(palette.text));
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

#[cfg(test)]
mod search_tests {
    use super::settings_search_matches;

    #[test]
    fn search_matches_translated_and_source_labels_without_accents() {
        assert!(settings_search_matches("  echelle ", "Scale", "Échelle"));
        assert!(settings_search_matches("scale", "Scale", "Échelle"));
        assert!(!settings_search_matches("password", "Scale", "Échelle"));
        assert!(!settings_search_matches(" ", "Scale", "Échelle"));
    }
}
