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

// ── Splash ────────────────────────────────────────────────────────────────

pub fn draw_splash(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.32);
        crate::brand(ui, palette, 56.0);
        ui.add_space(18.0);
        ui.label(
            RichText::new(t!(app.locale, "Atlas Commerce Desktop"))
                .strong()
                .size(24.0)
                .color(palette.text),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(t!(app.locale, "Native and fast stock management"))
                .color(palette.dim)
                .size(13.5),
        );
        ui.add_space(28.0);
        crate::screens::spinner(ui, palette);
    });

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
        crate::brand(ui, palette, 48.0);
        ui.add_space(20.0);
        page_title(ui, &t!(app.locale, "Welcome to Atlas Commerce"), palette);
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
        .fill(palette.accent)
        .rounding(10.0)
        .min_size(egui::vec2(240.0, 42.0));
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
    if let Some(job) = &app.server_form.probe_job {
        if let Some(report) = job.poll() {
            app.server_form.probe_report = Some(report);
            app.server_form.probe_job = None;
        }
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(if embedded { 12.0 } else { 40.0 });
                icon(ui, Icon::Server, palette.accent, 30.0);
                ui.add_space(10.0);
                page_title(ui, &t!(app.locale, "Choose your server"), palette);
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
                                    .rounding(10.0)
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
                                                            .fill(palette.surface)
                                                            .rounding(8.0),
                                                        )
                                                        .clicked()
                                                    {
                                                        forget = Some(entry.url.clone());
                                                    }
                                                    if primary_button(
                                                        ui,
                                                        if selected {
                                                            t!(app.locale, "Selected")
                                                        } else {
                                                            t!(app.locale, "Use")
                                                        }
                                                        .as_ref(),
                                                        palette,
                                                        !selected,
                                                    ) {
                                                        select_server(app, entry.url.clone());
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
                                &t!(app.locale, "Label (optional)"),
                                &mut app.server_form.custom_label,
                                palette,
                                false,
                            );
                            ui.add_space(8.0);
                            labelled_field(
                                ui,
                                &t!(app.locale, "Server address (https://…)"),
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
                                        t!(app.locale, "Checking…").as_ref()
                                    } else {
                                        t!(app.locale, "Check connection").as_ref()
                                    },
                                    palette,
                                ) && !probing
                                {
                                    start_probe(app);
                                }
                                if let Some(report) = &app.server_form.probe_report {
                                    let (icon, color) = if report.ok {
                                        (Icon::CircleCheck, Color32::from_rgb(0x4a, 0xe0, 0x8a))
                                    } else {
                                        (Icon::CircleX, palette.danger)
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
                            let valid = app.server_store.selected_url().is_some();
                            if primary_button(
                                ui,
                                &t!(app.locale, "Save and continue"),
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

fn start_probe(app: &mut AtlasApp) {
    let url = app.server_form.custom_url.clone();
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
    app.server_form.probe_report = None;
}

// ── Login ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum LoginTab {
    Password,
    Qr,
}

pub fn draw_login(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    poll_login_jobs(app);

    // A one-tap e-mail link arrived through the protocol handler.
    if let Some(link) = app.login.pending_link.clone() {
        app.login.pending_link = None;
        start_verify_link(app, link);
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.08);
                crate::brand(ui, palette, 40.0);
                ui.add_space(12.0);
                page_title(ui, &t!(app.locale, "Sign in"), palette);
                let host = app
                    .server_store
                    .selected_url()
                    .map(ServerUrl::host)
                    .unwrap_or_else(|| "—".to_owned());
                ui.label(RichText::new(host).color(palette.dim).size(12.5));
                ui.add_space(18.0);

                let width = 400.0_f32;
                ui.allocate_ui_with_layout(
                    egui::vec2(width, ui.available_height() - 60.0),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        // Tabs.
                        ui.horizontal(|ui| {
                            let tab = if app.qr.active {
                                LoginTab::Qr
                            } else {
                                LoginTab::Password
                            };
                            for (candidate, label, icon_kind) in [
                                (LoginTab::Password, t!(app.locale, "Password"), Icon::Lock),
                                (LoginTab::Qr, t!(app.locale, "Phone QR"), Icon::QrCode),
                            ] {
                                let selected = tab == candidate;
                                let tint = if selected {
                                    palette.accent
                                } else {
                                    palette.secondary
                                };
                                let button = egui::Button::new(
                                    RichText::new(label.as_ref())
                                        .color(if selected {
                                            palette.text
                                        } else {
                                            palette.secondary
                                        })
                                        .size(13.5),
                                )
                                .fill(if selected {
                                    palette.surface_hover
                                } else {
                                    palette.surface
                                })
                                .rounding(9.0);
                                let response = ui.add(button);
                                icon(ui, icon_kind, tint, 15.0);
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
                        ui.add_space(14.0);

                        if app.qr.active {
                            draw_qr(app, ui, palette);
                        } else {
                            draw_password(app, ui, palette);
                        }
                    },
                );
            });
        });
}

fn draw_password(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    card(palette).show(ui, |ui| {
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
            &t!(app.locale, "Username"),
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
                    .margin(egui::Margin::symmetric(10, 8))
                    .clip_text(true);
                ui.add_sized([ui.available_width() - 38.0, 34.0], edit);
                let eye = if app.login.show_password {
                    Icon::EyeOff
                } else {
                    Icon::Eye
                };
                if ui
                    .add(
                        egui::Button::new(eye.image(palette.secondary, 17.0))
                            .fill(palette.surface)
                            .rounding(8.0),
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
                &t!(app.locale, "E-mail verification code (6 digits)"),
                &mut app.login.otp,
                palette,
                false,
            );
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

        ui.add_space(14.0);
        dialog_actions(ui, |ui| {
            let busy = app.login.busy;
            if app.login.needs_otp {
                if primary_button(ui, &t!(app.locale, "Verify"), palette, !busy) {
                    start_otp(app);
                }
            } else if primary_button(ui, &t!(app.locale, "Sign in"), palette, !busy) {
                start_login(app);
            }
        });
    });

    ui.add_space(10.0);
    if ui
        .add(
            egui::Button::new(
                RichText::new(t!(app.locale, "Change server"))
                    .color(palette.secondary)
                    .size(12.5),
            )
            .fill(Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE),
        )
        .clicked()
    {
        app.screen = Screen::ServerSetup;
    }
}

fn draw_qr(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    // Status polling, at most every 1.5 s.
    if app.qr.active && app.qr.job_status.is_none() {
        let due = app
            .qr
            .next_poll
            .is_none_or(|at| std::time::Instant::now() >= at);
        if due && !app.qr.token.is_empty() {
            start_qr_poll(app);
        }
    }

    card(palette).show(ui, |ui| {
        ui.vertical_centered(|ui| {
            icon(ui, Icon::Smartphone, palette.accent, 22.0);
            ui.add_space(8.0);
            ui.label(
                RichText::new(t!(app.locale, "Scan with your phone"))
                    .strong()
                    .color(palette.text),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(t!(
                    app.locale,
                    "Open Atlas Commerce on your phone and scan this code to approve the computer."
                ))
                .color(palette.dim)
                .size(12.5),
            );
            ui.add_space(12.0);

            if app.qr.qr_url.is_empty() {
                crate::screens::spinner(ui, palette);
            } else {
                qr_image(ui, &app.qr.qr_url, palette);
            }
            ui.add_space(12.0);

            let (text, color) = match app.qr.status.as_str() {
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
            if secondary_button(ui, &t!(app.locale, "New code"), palette) {
                start_qr_session(app);
            }
        });
    });
}

fn qr_image(ui: &mut egui::Ui, url: &str, _palette: &Palette) {
    let code = match qrcode::QrCode::new(url) {
        Ok(code) => code,
        Err(_) => return,
    };
    let modules = code.to_colors();
    let width = code.width();
    let size = 210.0_f32;
    let module = size / width as f32;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 10.0, Color32::WHITE);
    for (index, color) in modules.iter().enumerate() {
        if matches!(color, qrcode::Color::Dark) {
            let column = index % width;
            let row = index / width;
            let module_rect = egui::Rect::from_min_size(
                egui::pos2(
                    rect.min.x + column as f32 * module,
                    rect.min.y + row as f32 * module,
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
    if let Some(job) = &app.login.job_login {
        if let Some(result) = job.poll() {
            app.login.job_login = None;
            app.login.busy = false;
            handle_login_response(app, result);
        }
    }
    // OTP.
    if let Some(job) = &app.login.job_otp {
        if let Some(result) = job.poll() {
            app.login.job_otp = None;
            app.login.busy = false;
            handle_login_response(app, result);
        }
    }
    // One-tap link.
    if let Some(job) = &app.login.job_link {
        if let Some(result) = job.poll() {
            app.login.job_link = None;
            app.login.busy = false;
            app.login.notice = None;
            handle_login_response(app, result);
        }
    }
    // QR session creation.
    if let Some(job) = &app.qr.job_create {
        if let Some(result) = job.poll() {
            app.qr.job_create = None;
            match result {
                Ok(session) => {
                    app.qr.token = session.token;
                    app.qr.qr_url = session.qr_url;
                    app.qr.next_poll = None;
                }
                Err(error) => app.notice(error.message),
            }
        }
    }
    // QR status.
    if let Some(job) = &app.qr.job_status {
        if let Some(result) = job.poll() {
            app.qr.job_status = None;
            match result {
                Ok(status) => {
                    app.qr.status = status.status.clone();
                    if let Some(name) = status.display_name.or(status.username) {
                        app.qr.account = name;
                    }
                    if matches!(status.status.as_str(), "expired" | "denied") {
                        app.qr.expired = true;
                    }
                    if status.status == "approved" {
                        // PC fingerprint completes the one-time exchange.
                        if let Some(client) = app.client.clone() {
                            let token = app.qr.token.clone();
                            app.qr.job_complete = Some(
                                app.runner
                                    .spawn(async move { client.qr_complete(&token).await }),
                            );
                        }
                    }
                }
                Err(error) => log::warn!("qr status poll failed: {}", error.message),
            }
        }
    }
    // QR completion.
    if let Some(job) = &app.qr.job_complete {
        if let Some(result) = job.poll() {
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
            &t!(app.locale, "A required update is available"),
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
            ui.label(
                RichText::new(manifest.notes.clone())
                    .color(palette.secondary)
                    .size(13.0),
            );
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
            &t!(app.locale, "Update now"),
            palette,
            app.update.progress.is_none(),
        ) {
            app.start_update_download();
        }
        ui.add_space(8.0);
        if let Some((received, total)) = app.update.progress {
            let percent = if total > 0 { received * 100 / total } else { 0 };
            ui.label(
                RichText::new(format!("{} %", percent))
                    .color(palette.dim)
                    .size(12.5),
            );
        }
    });
}
