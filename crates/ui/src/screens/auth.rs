//! Boot and authentication screens: splash, onboarding, server setup,
//! password + OTP login, one-tap e-mail links and QR phone pairing, and the
//! mandatory-update gate.

use std::time::Duration;

use atlas_api::ApiError;
use atlas_api::models::LoginResponse;
use atlas_api::probe;
use atlas_core::{ServerUrl, VerifyLink};
use egui::{Color32, RichText};

use crate::icons::Icon;
use crate::state::{AtlasApp, Screen};
use crate::t;
use crate::theme::Palette;
use crate::widgets::*;

/// Short local aliases for the 44 px interactive-target and 12 px control
/// radius constants, so the deeply indented builder chains in the server
/// cards stay within width.
const TARGET: f32 = crate::theme::TARGET_MIN_HEIGHT;
const RADIUS: f32 = crate::theme::RADIUS_CONTROL as f32;

// ── Splash ────────────────────────────────────────────────────────────────

pub fn draw_splash(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    // The splash is time-gated and hosts an animated bar, so it must keep
    // repainting even though no background job holds `busy()`.
    ui.ctx().request_repaint_after(Duration::from_millis(50));
    let time = ui.input(|input| input.time) as f32;
    let screen = ui.available_size();

    ui.vertical_centered(|ui| {
        ui.add_space(screen.y * 0.24);

        // Brand medallion: a soft pulsing halo and a thin ring around the
        // monochrome Atlas tile, matching the Android launch screen.
        let box_size = 104.0;
        let (medallion, _) =
            ui.allocate_exact_size(egui::Vec2::splat(box_size), egui::Sense::hover());
        let center = medallion.center();
        let pulse = (time * 2.2).sin() * 0.5 + 0.5;
        ui.painter().circle_filled(
            center,
            box_size * 0.47,
            palette.accent.gamma_multiply(0.07 + 0.06 * pulse),
        );
        ui.painter().circle_stroke(
            center,
            box_size * 0.47,
            egui::Stroke::new(1.0, palette.outline),
        );
        crate::widgets::overlay_at(
            ui,
            egui::Rect::from_center_size(center, egui::Vec2::splat(56.0)),
            |ui| {
                crate::branding::mark(ui, palette, 56.0);
            },
        );

        ui.add_space(22.0);
        ui.label(
            RichText::new(t!(app.locale, "Atlas Commerce Desktop"))
                .font(crate::fonts::bold(24.0))
                .color(palette.text),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(t!(app.locale, "Stock · Cash · Invoicing"))
                .color(palette.dim)
                .size(13.5),
        );
        ui.add_space(30.0);

        // Slim indeterminate track with a sweeping accent segment.
        let track_width = 220.0_f32.min(screen.x * 0.6);
        let (track, _) = ui.allocate_exact_size(egui::vec2(track_width, 4.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(track, 999.0, palette.surface_active);
        let span = 0.34;
        let head = (time * 0.85).fract() * (1.0 - span);
        let segment = egui::Rect::from_min_size(
            egui::pos2(track.left() + track.width() * head, track.top()),
            egui::vec2(track.width() * span, track.height()),
        );
        ui.painter().rect_filled(segment, 999.0, palette.accent);

        ui.add_space(14.0);
        ui.horizontal_centered(|ui| {
            icon(ui, Icon::ShieldCheck, palette.secondary, 14.0);
            ui.add_space(6.0);
            ui.label(
                RichText::new(t!(app.locale, "Securing your workspace…"))
                    .color(palette.secondary)
                    .size(12.5),
            );
        });
    });

    // Version pinned to the bottom of the splash.
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let galley =
        ui.painter()
            .layout_no_wrap(version, egui::FontId::proportional(11.5), palette.dim);
    ui.painter().galley(
        egui::pos2(screen.x / 2.0 - galley.size().x / 2.0, screen.y - 34.0),
        galley,
        palette.dim,
    );

    // Brief, calm splash while state restores.
    if app.started_at.elapsed() >= Duration::from_millis(650) {
        if app.update.mandatory.is_some() {
            app.screen = Screen::UpdateGate;
        } else if app.session.is_some() && app.client.is_some() {
            app.screen = Screen::Shell(crate::state::Route::Dashboard);
            app.bootstrap_shell();
        } else if !app.server_store.onboarding_done || app.server_store.selected_url().is_none() {
            app.screen = Screen::Onboarding;
        } else {
            app.screen = Screen::Login;
        }
    }
}

// ── Onboarding ─────────────────────────────────────────────────────────────

pub fn draw_onboarding(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.2);
        crate::branding::mark(ui, palette, 48.0);
        ui.add_space(20.0);
        page_title(ui, t!(app.locale, "Welcome to Atlas Commerce"), palette);
        ui.add_space(10.0);
        ui.label(
            RichText::new(t!(
                app.locale,
                "Your stock, sales and AI assistant in one native application."
            ))
            .color(palette.secondary)
            .size(14.0),
        );
        ui.add_space(30.0);
        let button = egui::Button::new(
            RichText::new(t!(app.locale, "Get started"))
                .color(palette.on_accent)
                .strong(),
        )
        .wrap_mode(egui::TextWrapMode::Extend)
        .fill(palette.accent)
        .corner_radius(RADIUS)
        .min_size(egui::vec2(240.0, crate::theme::PRIMARY_BUTTON_HEIGHT));
        if ui.add(button).clicked() {
            app.screen = Screen::ServerSetup;
        }
        ui.add_space(12.0);
        ui.label(
            RichText::new(t!(
                app.locale,
                "No account yet? Ask your shop's Atlas administrator."
            ))
            .color(palette.dim)
            .size(12.5),
        );
    });
}

// ── Server setup ───────────────────────────────────────────────────────────

pub fn draw_server_setup(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette, embedded: bool) {
    // Poll the probe.
    if let Some(job) = &app.server_form.probe_job
        && let Some(report) = job.poll()
    {
        app.server_form.probe_job = None;
        if let Some(url) = app.server_form.probing_saved.take()
            && probe_matches_current_url(&url, &report)
        {
            select_server(app, url.clone());
            app.server_form.verified_selected = Some(url);
        }
        app.server_form.probe_report = Some(report);
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(if embedded { 12.0 } else { 40.0 });
                crate::branding::mark(ui, palette, 46.0);
                ui.add_space(10.0);
                page_title(ui, t!(app.locale, "Choose your server"), palette);
                ui.add_space(4.0);
                ui.label(
                    RichText::new(t!(
                        app.locale,
                        "Atlas Cloud or your own hosted Atlas Commerce worker."
                    ))
                    .color(palette.dim),
                );
                ui.add_space(20.0);
            });

            let width = (ui.available_width().min(560.0) - 24.0).max(320.0);
            ui.vertical_centered(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(width, ui.available_height() - 40.0),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        card(palette).show(ui, |ui| {
                            ui.label(
                                RichText::new(t!(app.locale, "Saved servers"))
                                    .strong()
                                    .color(palette.text),
                            );
                            ui.add_space(8.0);
                            let saved = app.server_store.saved.clone();
                            let mut forget: Option<String> = None;
                            for entry in &saved {
                                let selected =
                                    app.server_store.selected_url() == Some(entry.url.as_str());
                                egui::Frame::NONE
                                    .fill(if selected {
                                        palette.accent.gamma_multiply(0.14)
                                    } else {
                                        palette.window
                                    })
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        if selected {
                                            palette.accent
                                        } else {
                                            palette.outline
                                        },
                                    ))
                                    .corner_radius(crate::theme::RADIUS_CARD as f32)
                                    .inner_margin(egui::Margin::symmetric(12, 10))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    RichText::new(entry.label.clone())
                                                        .strong()
                                                        .color(palette.text),
                                                );
                                                ui.label(
                                                    RichText::new(entry.url.clone())
                                                        .color(palette.dim)
                                                        .size(12.0),
                                                );
                                            });
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui
                                                        .add(
                                                            egui::Button::new(
                                                                RichText::new(t!(
                                                                    app.locale, "Forget"
                                                                ))
                                                                .color(palette.secondary),
                                                            )
                                                            .wrap_mode(egui::TextWrapMode::Extend)
                                                            .fill(palette.surface)
                                                            .min_size(egui::vec2(0.0, TARGET))
                                                            .corner_radius(RADIUS),
                                                        )
                                                        .clicked()
                                                    {
                                                        forget = Some(entry.url.clone());
                                                    }
                                                    if primary_button(
                                                        ui,
                                                        if selected
                                                            && app
                                                                .server_form
                                                                .verified_selected
                                                                .as_deref()
                                                                == Some(entry.url.as_str())
                                                        {
                                                            t!(app.locale, "Selected")
                                                        } else {
                                                            t!(app.locale, "Use")
                                                        }
                                                        .as_ref(),
                                                        palette,
                                                        app.server_form.probe_job.is_none(),
                                                    ) {
                                                        start_saved_probe(app, entry.url.clone());
                                                    }
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(6.0);
                            }
                            if let Some(url) = forget {
                                app.server_store.forget(&url);
                                app.save_servers();
                            }
                        });

                        ui.add_space(14.0);

                        card(palette).show(ui, |ui| {
                            ui.label(
                                RichText::new(t!(app.locale, "Add a custom server"))
                                    .strong()
                                    .color(palette.text),
                            );
                            ui.add_space(8.0);
                            labelled_field(
                                ui,
                                t!(app.locale, "Label (optional)"),
                                &mut app.server_form.custom_label,
                                palette,
                                false,
                            );
                            ui.add_space(8.0);
                            labelled_field(
                                ui,
                                t!(app.locale, "Server address (https://…)"),
                                &mut app.server_form.custom_url,
                                palette,
                                false,
                            );
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                let probing = app.server_form.probe_job.is_some();
                                if secondary_button(
                                    ui,
                                    if probing {
                                        t!(app.locale, "Checking…")
                                    } else {
                                        t!(app.locale, "Check connection")
                                    },
                                    palette,
                                ) && !probing
                                {
                                    start_probe(app);
                                }
                                // A successful probe is valid only for the exact
                                // normalized address currently in the field.
                                let verified =
                                    app.server_form.probe_report.as_ref().is_some_and(|report| {
                                        probe_matches_current_url(
                                            &app.server_form.custom_url,
                                            report,
                                        )
                                    });
                                if verified
                                    && secondary_button(
                                        ui,
                                        t!(app.locale, "Add this server"),
                                        palette,
                                    )
                                {
                                    let url = app.server_form.custom_url.clone();
                                    let label = app.server_form.custom_label.clone();
                                    match app.server_store.select(&url, &label) {
                                        Ok(url) => {
                                            app.server_form.verified_selected = Some(url);
                                            app.server_form.saved_error = None;
                                            app.server_form.custom_url = String::new();
                                            app.server_form.custom_label = String::new();
                                            app.server_form.probe_report = None;
                                            app.server_form.probe_job = None;
                                            app.save_servers();
                                            app.rebuild_client();
                                        }
                                        Err(error) => {
                                            app.server_form.saved_error = Some(error.to_string());
                                        }
                                    }
                                }
                                if let Some(report) = &app.server_form.probe_report {
                                    // Probe result is a status: the finance
                                    // positive token already resolves to
                                    // #34D399 (dark) or #0F7B4F (light).
                                    let (icon, color) = if report.ok {
                                        (Icon::CircleCheck, palette.finance_positive)
                                    } else {
                                        (Icon::CircleX, palette.finance_danger)
                                    };
                                    self::icon(ui, icon, color, 16.0);
                                }
                            });
                            if let Some(report) = &app.server_form.probe_report {
                                ui.add_space(6.0);
                                let color = if report.ok {
                                    palette.secondary
                                } else {
                                    palette.danger
                                };
                                ui.label(
                                    RichText::new(report.message.clone())
                                        .color(color)
                                        .size(12.5),
                                );
                                if report.ok {
                                    ui.label(
                                        RichText::new(format!(
                                            "{} · D1: {} · R2: {}",
                                            report
                                                .environment
                                                .clone()
                                                .unwrap_or_else(|| "env ?".to_owned()),
                                            if report.database { "✓" } else { "✗" },
                                            if report.storage { "✓" } else { "✗" }
                                        ))
                                        .color(palette.dim)
                                        .size(12.0),
                                    );
                                }
                            }
                            if let Some(error) = &app.server_form.saved_error {
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(error.clone())
                                        .color(palette.danger)
                                        .size(12.5),
                                );
                            }
                        });

                        ui.add_space(16.0);
                        dialog_actions(ui, |ui| {
                            let valid = app.server_store.selected_url().is_some_and(|url| {
                                app.server_form.verified_selected.as_deref() == Some(url)
                            }) && app.server_form.custom_url.trim().is_empty()
                                && app.server_form.probe_job.is_none();
                            if primary_button(
                                ui,
                                t!(app.locale, "Save and continue"),
                                palette,
                                valid,
                            ) {
                                app.server_store.onboarding_done = true;
                                app.save_servers();
                                app.rebuild_client();
                                app.screen = Screen::Login;
                            }
                        });
                    },
                );
            });
        });
}

fn probe_matches_current_url(raw: &str, report: &probe::ProbeReport) -> bool {
    report.ok && ServerUrl::normalize(raw).as_deref() == Some(report.base_url.as_str())
}

fn start_saved_probe(app: &mut AtlasApp, url: String) {
    app.server_form.probing_saved = Some(url.clone());
    spawn_probe(app, url);
}

fn start_probe(app: &mut AtlasApp) {
    app.server_form.probing_saved = None;
    let url = app.server_form.custom_url.clone();
    spawn_probe(app, url);
}

fn spawn_probe(app: &mut AtlasApp, url: String) {
    let fingerprint = app.identity.fingerprint().to_owned();
    let ua = atlas_core::DeviceIdentity::user_agent();
    let job = app
        .runner
        .spawn(async move { probe::probe(&url, &fingerprint, &ua).await });
    app.server_form.probe_job = Some(job);
    app.server_form.probe_report = None;
}

fn select_server(app: &mut AtlasApp, url: String) {
    // Switching server signs the current session out, as on Android.
    if app.session.is_some() {
        app.sign_out();
    }
    app.server_store.selected = Some(url);
    app.save_servers();
    app.rebuild_client();
    app.server_form.custom_url.clear();
    app.server_form.custom_label.clear();
    app.server_form.probe_report = None;
    app.server_form.probe_job = None;
    app.server_form.probing_saved = None;
    app.server_form.verified_selected = None;
}

// ── Login ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum LoginTab {
    Password,
    Qr,
}

pub fn draw_login(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    poll_login_jobs(app);
    if !matches!(app.screen, Screen::Login) {
        return;
    }

    if !app.requires_password
        && let Some(link) = app.login.pending_link.clone()
    {
        app.login.pending_link = None;
        start_verify_link(app, link);
    }

    let width = (ui.available_width() - 48.0).clamp(220.0, 820.0);
    let split = width >= 680.0;
    // The desktop QR is a real one-time pairing session, never a decorative
    // code. Do not initiate it while the password-only lock is active.
    if split && !app.requires_password && !app.qr.active && app.client.is_some() {
        start_qr_session(app);
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(32.0);
                card(palette)
                    .corner_radius(18.0)
                    .inner_margin(egui::Margin::same(0))
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.horizontal_top(|ui| {
                            let left_width = if split { width * 0.69 } else { width };
                            ui.allocate_ui_with_layout(
                                egui::vec2(left_width, 520.0),
                                egui::Layout::top_down(egui::Align::Center),
                                |ui| draw_login_form_panel(app, ui, palette, split),
                            );
                            if split {
                                ui.separator();
                                let right_width = ui.available_width();
                                ui.allocate_ui_with_layout(
                                    egui::vec2(right_width, 520.0),
                                    egui::Layout::top_down(egui::Align::Center),
                                    |ui| {
                                        ui.add_space(80.0);
                                        if app.requires_password {
                                            icon(ui, Icon::Lock, palette.secondary, 32.0);
                                            ui.add_space(16.0);
                                            ui.label(
                                                RichText::new(t!(
                                                    app.locale,
                                                    "Enter your password to unlock this workspace."
                                                ))
                                                .color(palette.secondary),
                                            );
                                        } else {
                                            draw_qr(app, ui, palette);
                                        }
                                    },
                                );
                            }
                        });
                    });
                ui.add_space(14.0);
                ui.label(
                    RichText::new(format!(
                        "{} · v{}",
                        t!(app.locale, "Native desktop client"),
                        env!("CARGO_PKG_VERSION")
                    ))
                    .color(palette.dim)
                    .size(11.5),
                );
            });
        });
}

/// Left half of the login card. Reserve the action before scrolling the form,
/// so errors and OTP prompts cannot push it out of the card.
fn draw_login_form_panel(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette, split: bool) {
    let show_form = split || app.requires_password || !app.qr.active;
    if show_form {
        egui::Panel::bottom(egui::Id::new("login-submit"))
            .exact_size(80.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE.fill(palette.surface))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    let width = (ui.available_width() - 48.0).clamp(180.0, 340.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| draw_password_submit(app, ui, palette),
                    );
                });
            });
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(32.0);
            crate::branding::lockup(ui, palette, 44.0);
            ui.add_space(16.0);
            ui.label(
                RichText::new(t!(app.locale, "Welcome back"))
                    .color(palette.text)
                    .strong()
                    .size(20.0),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(t!(app.locale, "Sign in to continue to your workspace."))
                    .color(palette.secondary)
                    .size(12.5),
            );
            ui.add_space(12.0);
            let width = (ui.available_width() - 48.0).clamp(180.0, 340.0);
            ui.allocate_ui_with_layout(
                egui::vec2(width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    draw_login_server(app, ui, palette);
                    ui.add_space(8.0);
                    if !split && !app.requires_password {
                        segmented_tabs(app, ui, palette);
                        ui.add_space(8.0);
                    }
                    if show_form {
                        if app.requires_password {
                            ui.label(
                                RichText::new(t!(
                                    app.locale,
                                    "Enter your password to unlock this workspace."
                                ))
                                .color(palette.secondary),
                            );
                            ui.add_space(8.0);
                        }
                        draw_password(app, ui, palette);
                    } else {
                        draw_qr(app, ui, palette);
                    }
                },
            );
        });
    });
}

fn draw_login_server(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let host = app
        .server_store
        .selected_url()
        .map(ServerUrl::host)
        .unwrap_or_else(|| "—".to_owned());
    let host = if host.chars().count() > 22 {
        format!("{}…", host.chars().take(21).collect::<String>())
    } else {
        host
    };
    ui.horizontal_wrapped(|ui| {
        icon(ui, Icon::Server, palette.secondary, 14.0);
        pill(ui, &host, palette.window, palette.secondary);
        if ui
            .add(
                egui::Button::new(
                    RichText::new(t!(app.locale, "Change server"))
                        .color(palette.secondary)
                        .size(12.5),
                )
                .fill(Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE)
                .min_size(egui::vec2(0.0, TARGET)),
            )
            .clicked()
        {
            // In-flight password and QR jobs belong to the old server.
            app.login = Default::default();
            app.qr = Default::default();
            app.server_form.verified_selected = None;
            app.screen = Screen::ServerSetup;
        }
    });
}

/// The password / phone-QR switch, drawn as one segmented control.
fn segmented_tabs(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let tab = if app.qr.active {
        LoginTab::Qr
    } else {
        LoginTab::Password
    };
    egui::Frame::NONE
        .fill(palette.window)
        .stroke(egui::Stroke::new(1.0, palette.outline))
        .corner_radius(RADIUS)
        .inner_margin(egui::Margin::symmetric(4, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let button_width = (ui.available_width() - 16.0) / 2.0;
            ui.horizontal(|ui| {
                for (candidate, label, icon_kind) in [
                    (LoginTab::Password, t!(app.locale, "Password"), Icon::Lock),
                    (LoginTab::Qr, t!(app.locale, "Phone QR"), Icon::QrCode),
                ] {
                    let selected = tab == candidate;
                    let response = ui
                        .add_sized(
                            [button_width, crate::theme::TARGET_MIN_HEIGHT],
                            egui::Button::new(
                                RichText::new(label.as_ref())
                                    .color(if selected {
                                        palette.text
                                    } else {
                                        palette.secondary
                                    })
                                    .strong()
                                    .size(13.5),
                            )
                            .wrap_mode(egui::TextWrapMode::Extend)
                            .fill(if selected {
                                palette.surface_active
                            } else {
                                Color32::TRANSPARENT
                            })
                            .corner_radius(RADIUS),
                        )
                        .on_hover_text(label.as_ref());
                    icon(
                        ui,
                        icon_kind,
                        if selected {
                            palette.accent
                        } else {
                            palette.secondary
                        },
                        15.0,
                    );
                    if response.clicked() {
                        if candidate == LoginTab::Qr && !app.qr.active {
                            start_qr_session(app);
                        }
                        if candidate == LoginTab::Password {
                            app.qr.active = false;
                        }
                    }
                }
            });
        });
}

fn draw_password(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    // The login card already provides the surface; no nested card frame.
    ui.vertical(|ui| {
        if let Some(error) = &app.login.error {
            error_banner(ui, error, palette);
            ui.add_space(8.0);
        }
        if let Some(notice) = &app.login.notice {
            info_banner(ui, notice, palette);
            ui.add_space(8.0);
        }

        labelled_field(
            ui,
            t!(app.locale, "Username"),
            &mut app.login.username,
            palette,
            false,
        );
        ui.add_space(8.0);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(t!(app.locale, "Password"))
                    .color(palette.secondary)
                    .size(12.5),
            );
            ui.horizontal(|ui| {
                let edit = egui::TextEdit::singleline(&mut app.login.password)
                    .password(!app.login.show_password)
                    .margin(egui::Margin::symmetric(14, 8))
                    .clip_text(true);
                ui.add_sized(
                    [
                        ui.available_width() - crate::theme::TARGET_MIN_HEIGHT - 8.0,
                        crate::theme::TARGET_MIN_HEIGHT,
                    ],
                    edit,
                );
                let eye = if app.login.show_password {
                    Icon::EyeOff
                } else {
                    Icon::Eye
                };
                if ui
                    .add(
                        egui::Button::new(eye.image(palette.secondary, 17.0))
                            .fill(palette.surface)
                            .min_size(egui::vec2(
                                crate::theme::TARGET_MIN_HEIGHT,
                                crate::theme::TARGET_MIN_HEIGHT,
                            ))
                            .corner_radius(RADIUS),
                    )
                    .on_hover_text(t!(app.locale, "Show password"))
                    .clicked()
                {
                    app.login.show_password = !app.login.show_password;
                }
            });
        });

        if app.login.needs_otp {
            ui.add_space(8.0);
            labelled_field(
                ui,
                t!(app.locale, "E-mail verification code (6 digits)"),
                &mut app.login.otp,
                palette,
                false,
            );
            if !app.requires_password {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(t!(
                        app.locale,
                        "Tip: open the link in the e-mail — this app verifies it without typing."
                    ))
                    .color(palette.dim)
                    .size(11.5),
                );
            }
        }
    });
}

/// Persistent login action, outside the scrolling credentials content.
fn draw_password_submit(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.add_space(16.0);
    let busy = app.login.busy;
    let label = if app.login.needs_otp {
        t!(app.locale, "Verify")
    } else {
        t!(app.locale, "Sign in")
    };
    let clicked = ui
        .add_sized(
            [ui.available_width(), crate::theme::PRIMARY_BUTTON_HEIGHT],
            egui::Button::new(
                RichText::new(label.as_ref())
                    .color(palette.on_accent)
                    .strong()
                    .size(14.0),
            )
            .wrap_mode(egui::TextWrapMode::Extend)
            .fill(if !busy {
                palette.accent
            } else {
                palette.surface_active
            })
            .corner_radius(RADIUS),
        )
        .clicked();
    if clicked && !busy {
        if app.login.needs_otp {
            start_otp(app);
        } else {
            start_login(app);
        }
    }
}

fn draw_qr(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    // Status polling, at most every 1.5 s. It stops as soon as the one-time
    // completion exchange is in flight so a repeated "approved" poll cannot
    // spawn it twice.
    if app.qr.active
        && !app.qr.expired
        && app.qr.job_status.is_none()
        && app.qr.job_complete.is_none()
    {
        let due = app
            .qr
            .next_poll
            .is_none_or(|at| std::time::Instant::now() >= at);
        if !app.qr.token.is_empty() {
            if due {
                start_qr_poll(app);
            } else {
                ui.ctx().request_repaint_after(Duration::from_millis(1500));
            }
        }
    }

    // The login card already provides the surface; no nested card frame.
    ui.vertical_centered(|ui| {
        if app.qr.qr_url.is_empty() {
            if app.qr.status != "error" {
                crate::screens::centered_spinner(ui, palette);
            }
        } else {
            qr_image(ui, &app.qr.qr_url, palette);
        }
        ui.add_space(16.0);
        ui.label(
            RichText::new(t!(app.locale, "Sign in with a QR code"))
                .strong()
                .color(palette.text),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(t!(
                app.locale,
                "Open Atlas Commerce on your phone and scan this code to approve the computer."
            ))
            .color(palette.secondary)
            .size(12.5),
        );
        ui.add_space(12.0);

        let (text, color) = match app.qr.status.as_str() {
            "error" => (
                t!(app.locale, "Could not create a QR code. Try again.").into_owned(),
                palette.danger,
            ),
            "approved" => (
                t!(app.locale, "Phone approved — finalizing…").into_owned(),
                palette.accent,
            ),
            "claimed" => (
                t!(app.locale, "Phone connected — confirm on the phone.").into_owned(),
                palette.secondary,
            ),
            "denied" => (
                t!(app.locale, "Login refused on the phone.").into_owned(),
                palette.danger,
            ),
            "expired" | "consumed" if app.qr.expired => {
                (t!(app.locale, "Code expired.").into_owned(), palette.danger)
            }
            _ => (
                t!(app.locale, "Waiting for the phone…").into_owned(),
                palette.secondary,
            ),
        };
        ui.label(RichText::new(text).color(color).size(13.0));
        if !app.qr.account.is_empty() {
            ui.label(
                RichText::new(app.qr.account.clone())
                    .color(palette.dim)
                    .size(12.0),
            );
        }
        ui.add_space(10.0);
        if secondary_button(ui, t!(app.locale, "New code"), palette) {
            start_qr_session(app);
        }
    });
}

fn qr_image(ui: &mut egui::Ui, url: &str, _palette: &Palette) {
    let code = match qrcode::QrCode::new(url) {
        Ok(code) => code,
        Err(_) => return,
    };
    let modules = code.to_colors();
    let width = code.width();
    // Four quiet-zone modules are required around the code for scanners.
    let size = 176.0_f32;
    let module = size / (width + 8) as f32;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 10.0, Color32::WHITE);
    for (index, color) in modules.iter().enumerate() {
        if matches!(color, qrcode::Color::Dark) {
            let column = index % width;
            let row = index / width;
            let module_rect = egui::Rect::from_min_size(
                egui::pos2(
                    rect.min.x + (column + 4) as f32 * module,
                    rect.min.y + (row + 4) as f32 * module,
                ),
                egui::vec2(module, module),
            );
            painter.rect_filled(module_rect, 0.0, Color32::BLACK);
        }
    }
}

// ── Job starters ────────────────────────────────────────────────────────────

fn start_login(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        app.login.error = Some(t!(app.locale, "Choose a server first.").into_owned());
        return;
    };
    let username = app.login.username.trim().to_lowercase();
    let password = app.login.password.clone();
    if username.is_empty() || password.is_empty() {
        app.login.error = Some(t!(app.locale, "Enter your username and password.").into_owned());
        return;
    }
    app.login.busy = true;
    app.login.error = None;
    app.login.job_login = Some(
        app.runner
            .spawn(async move { client.login(&username, &password).await }),
    );
}

fn start_otp(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let username = app.login.username.trim().to_lowercase();
    let password = app.login.password.clone();
    let otp: String = app.login.otp.chars().filter(char::is_ascii_digit).collect();
    if otp.len() < 4 {
        app.login.error =
            Some(t!(app.locale, "Enter the complete code received by e-mail.").into_owned());
        return;
    }
    app.login.busy = true;
    app.login.error = None;
    app.login.job_otp = Some(
        app.runner
            .spawn(async move { client.verify_otp(&username, &password, &otp).await }),
    );
}

fn start_verify_link(app: &mut AtlasApp, link: VerifyLink) {
    let Some(client) = app.client.clone() else {
        app.login.error = Some(t!(app.locale, "Choose a server first.").into_owned());
        return;
    };
    app.login.busy = true;
    app.login.error = None;
    app.login.notice = Some(t!(app.locale, "Verifying the e-mail link…").into_owned());
    app.login.job_link = Some(app.runner.spawn(async move {
        client
            .verify_link(&link.username, &link.otp, &link.sig, link.exp)
            .await
    }));
}

fn start_qr_session(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    app.qr.active = true;
    app.qr.status = "pending".to_owned();
    app.qr.expired = false;
    app.qr.account.clear();
    app.qr.job_status = None;
    app.qr.job_complete = None;
    app.qr.next_poll = None;
    app.qr.qr_url = String::new();
    app.qr.token = String::new();
    app.qr.job_create = Some(
        app.runner
            .spawn(async move { client.qr_create_pc_session().await }),
    );
}

fn start_qr_poll(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let token = app.qr.token.clone();
    if token.is_empty() {
        return;
    }
    app.qr.next_poll = Some(std::time::Instant::now() + Duration::from_millis(1500));
    app.qr.job_status = Some(
        app.runner
            .spawn(async move { client.qr_status(&token).await }),
    );
}

// ── Job polling / state transitions ─────────────────────────────────────────

pub fn poll_login_jobs(app: &mut AtlasApp) {
    // Login (password).
    if let Some(job) = &app.login.job_login
        && let Some(result) = job.poll()
    {
        app.login.job_login = None;
        app.login.busy = false;
        handle_login_response(app, result);
    }
    // OTP.
    if let Some(job) = &app.login.job_otp
        && let Some(result) = job.poll()
    {
        app.login.job_otp = None;
        app.login.busy = false;
        handle_login_response(app, result);
    }
    // One-tap link.
    if let Some(job) = &app.login.job_link
        && let Some(result) = job.poll()
    {
        app.login.job_link = None;
        app.login.busy = false;
        app.login.notice = None;
        handle_login_response(app, result);
    }
    // QR session creation.
    if let Some(job) = &app.qr.job_create
        && let Some(result) = job.poll()
    {
        app.qr.job_create = None;
        match result {
            Ok(session) => {
                app.qr.token = session.token;
                app.qr.qr_url = session.qr_url;
                app.qr.next_poll = None;
            }
            Err(error) => {
                app.qr.status = "error".to_owned();
                app.notice(error.message);
            }
        }
    }
    // QR status.
    if let Some(job) = &app.qr.job_status
        && let Some(result) = job.poll()
    {
        app.qr.job_status = None;
        match result {
            Ok(status) => {
                app.qr.status = status.status.clone();
                if let Some(name) = status.display_name.or(status.username) {
                    app.qr.account = name;
                }
                if matches!(status.status.as_str(), "expired" | "denied" | "consumed") {
                    app.qr.expired = true;
                }
                if status.status == "approved"
                    && app.qr.job_complete.is_none()
                    && let Some(client) = app.client.clone()
                {
                    // PC fingerprint completes the one-time exchange. The
                    // guard makes a second "approved" poll a no-op while the
                    // first exchange is still running.
                    let token = app.qr.token.clone();
                    app.qr.job_complete = Some(
                        app.runner
                            .spawn(async move { client.qr_complete(&token).await }),
                    );
                }
            }
            Err(error) => log::warn!("qr status poll failed: {}", error.message),
        }
    }
    // QR completion.
    if let Some(job) = &app.qr.job_complete
        && let Some(result) = job.poll()
    {
        app.qr.job_complete = None;
        match result {
            Ok(complete) => {
                if let Some(token) = complete.token {
                    app.enter_session(
                        token,
                        complete
                            .username
                            .unwrap_or_else(|| "utilisateur".to_owned()),
                        complete.role.unwrap_or_else(|| "user".to_owned()),
                        complete.display_name.unwrap_or_default(),
                        complete.avatar_url.unwrap_or_default(),
                    );
                }
            }
            Err(error) => app.notice(error.message),
        }
    }
    // Probe is polled in draw_server_setup.
}

fn handle_login_response(app: &mut AtlasApp, result: Result<LoginResponse, ApiError>) {
    match result {
        Ok(response) if response.success && response.needs_otp => {
            app.login.needs_otp = true;
            app.login.notice = Some(response.message.unwrap_or_else(|| {
                t!(app.locale, "A verification code was sent by e-mail.").into_owned()
            }));
        }
        Ok(response) if response.success => {
            if let Some(token) = response.token {
                app.enter_session(
                    token,
                    response
                        .username
                        .unwrap_or_else(|| app.login.username.clone()),
                    response.role.unwrap_or_else(|| "user".to_owned()),
                    response.display_name.unwrap_or_default(),
                    String::new(),
                );
            } else {
                app.login.error = Some(response.message.unwrap_or_else(|| {
                    t!(app.locale, "Incomplete login — try again.").into_owned()
                }));
            }
        }
        Ok(response) => {
            app.login.error = Some(
                response
                    .error
                    .unwrap_or_else(|| t!(app.locale, "Login failed.").into_owned()),
            );
        }
        Err(error) => app.login.error = Some(error.message),
    }
}

// ── Mandatory update gate ───────────────────────────────────────────────────

pub fn draw_update_gate(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let Some(manifest) = app.update.mandatory.clone() else {
        app.screen = Screen::Login;
        return;
    };
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.22);
        icon(ui, Icon::Download, palette.accent, 44.0);
        ui.add_space(16.0);
        page_title(
            ui,
            t!(app.locale, "A required update is available"),
            palette,
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                t!(
                    app.locale,
                    "This version can no longer talk to the server safely. Update to continue."
                )
                .into_owned(),
            )
            .color(palette.secondary),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!(
                "v{} → v{}",
                env!("CARGO_PKG_VERSION"),
                manifest.version
            ))
            .color(palette.dim)
            .size(12.5),
        );
        if !manifest.notes.is_empty() {
            ui.add_space(10.0);
            crate::widgets::release_notes(ui, &manifest.notes, palette, 220.0);
        }
        if let Some(error) = &app.update.error {
            ui.add_space(10.0);
            ui.label(
                RichText::new(error.clone())
                    .color(palette.danger)
                    .size(12.5),
            );
        }
        ui.add_space(20.0);
        if primary_button(
            ui,
            t!(app.locale, "Update now"),
            palette,
            app.update.progress.is_none(),
        ) {
            app.start_update_download();
        }
        ui.add_space(8.0);
        if let Some((received, total)) = app.update.progress {
            let percent = (received * 100).checked_div(total).unwrap_or(0);
            ui.label(
                RichText::new(format!("{} %", percent))
                    .color(palette.dim)
                    .size(12.5),
            );
        }
    });
}

#[cfg(test)]
mod server_verification_tests {
    use super::{probe, probe_matches_current_url};

    #[test]
    fn probe_is_only_valid_for_its_successful_address() {
        let mut report = probe::ProbeReport {
            ok: true,
            base_url: "https://stock.afmarbre.com".to_owned(),
            latency_ms: 1,
            environment: None,
            database: true,
            storage: true,
            message: String::new(),
        };
        assert!(probe_matches_current_url("stock.afmarbre.com/", &report));
        assert!(!probe_matches_current_url("https://other.example", &report));
        report.ok = false;
        assert!(!probe_matches_current_url("stock.afmarbre.com", &report));
    }
}
