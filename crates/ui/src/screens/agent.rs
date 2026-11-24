//! Native Atlas Agent drawer. Chat turns use the Worker's buffered endpoint;
//! server-proposed actions are never executed or approved implicitly.

use crate::icons::Icon;
use crate::state::{AtlasApp, ChatMessage};
use crate::t;
use crate::theme::Palette;
use crate::widgets::{card, icon, info_banner, pill, primary_button, secondary_button};
use egui::RichText;
use std::time::Instant;

const MAX_MESSAGE_UTF16: usize = 2500;

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let retry_gate_open = app
        .agent
        .policy_retry_at
        .is_none_or(|at| at <= Instant::now());
    if app.agent.policy.is_none()
        && app.agent.job_policy.is_none()
        && retry_gate_open
        && let Some(client) = app.client.clone()
    {
        app.agent.job_policy = Some(app.runner.spawn(async move { client.agent_policy().await }));
    }

    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        if let crate::state::Screen::Shell(route) = app.screen {
            let (_, name) = super::shell::route_meta(app.locale, route);
            pill(
                ui,
                format!("{} · {name}", t!(app.locale, "Current page")),
                palette.surface_active,
                palette.secondary,
            );
        }
        if secondary_button(ui, t!(app.locale, "History"), palette) {
            app.agent.history_open = !app.agent.history_open;
            if app.agent.history_open
                && app.agent.history_list.data.is_none()
                && !app.agent.history_list.loading
                && let Some(client) = app.client.clone()
            {
                app.agent.history_list.start(
                    app.runner
                        .spawn(async move { client.agent_threads().await }),
                );
            }
        }
        ui.add_enabled_ui(!app.agent.busy, |ui| {
            if secondary_button(ui, t!(app.locale, "New conversation"), palette) {
                app.agent.messages.clear();
                app.agent.thread_id.clear();
                app.agent.draft.clear();
            }
        });
    });
    if app.agent.history_open {
        draw_history(app, ui, palette);
        return;
    }
    if let Some(policy) = &app.agent.policy {
        if policy.blocked {
            info_banner(
                ui,
                t!(
                    app.locale,
                    "The AI agent is not available for this account."
                ),
                palette,
            );
        } else if !policy.is_admin {
            ui.label(
                RichText::new(format!(
                    "{}: {}/{} · {}: {}/{}",
                    t!(app.locale, "Daily quota"),
                    policy.daily_used,
                    policy.daily_limit,
                    t!(app.locale, "Weekly quota"),
                    policy.weekly_used,
                    policy.weekly_limit
                ))
                .color(palette.secondary)
                .size(11.0),
            );
        }
    } else if app.agent.policy_retry_at.is_some() {
        ui.label(
            RichText::new(t!(
                app.locale,
                "Quota information is unavailable; the server will check access when you send."
            ))
            .color(palette.secondary)
            .size(11.0),
        );
    }
    ui.add_space(8.0);

    // Keep the composer visible in the narrow drawer while the transcript scrolls.
    let transcript_height = (ui.available_height() - 210.0).clamp(90.0, 520.0);
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::ScrollArea::vertical()
            .id_salt("atlas-agent-transcript")
            .max_height(transcript_height)
            .min_scrolled_height(transcript_height)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if app.agent.messages.is_empty() && app.agent.job_policy.is_some() {
                    agent_skeleton(ui, palette, app.settings.reduced_motion, 2);
                } else if app.agent.messages.is_empty() {
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        crate::branding::mark(ui, palette, 32.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("Atlas Agent")
                                    .color(palette.text)
                                    .strong()
                                    .size(17.0),
                            );
                            ui.label(
                                RichText::new(t!(app.locale, "How can I help with your shop?"))
                                    .color(palette.secondary),
                            );
                        });
                    });
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(t!(app.locale, "Ask about sales, stock or your daily work."))
                            .color(palette.secondary),
                    );
                } else {
                    for message in &app.agent.messages {
                        chat_bubble(ui, palette, app.locale, message);
                        ui.add_space(10.0);
                    }
                }
                if app.agent.busy {
                    agent_skeleton(ui, palette, app.settings.reduced_motion, 1);
                }
            });
    });
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.add_enabled_ui(!app.agent.busy, |ui| {
            if secondary_button(ui, t!(app.locale, "Low stock"), palette) {
                app.agent.draft = t!(app.locale, "Summarize low stock.").into_owned();
            }
            if secondary_button(ui, t!(app.locale, "Daily report"), palette) {
                app.agent.draft = t!(app.locale, "Summarize today's sales.").into_owned();
            }
        });
    });
    ui.add_space(6.0);
    let blocked = app
        .agent
        .policy
        .as_ref()
        .is_some_and(|policy| policy.blocked);
    let length = app.agent.draft.trim().encode_utf16().count();
    card(palette)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let response = ui.add_enabled(
                !blocked && !app.agent.busy,
                egui::TextEdit::multiline(&mut app.agent.draft)
                    .desired_width(ui.available_width())
                    .desired_rows(2)
                    .hint_text(t!(app.locale, "Write a message…")),
            );
            let keyboard_send = response.has_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!("{length}/{MAX_MESSAGE_UTF16} · Ctrl/⌘ + Entrée"))
                        .color(if length > MAX_MESSAGE_UTF16 {
                            palette.danger
                        } else {
                            palette.secondary
                        })
                        .size(11.0),
                );
                let enabled =
                    !blocked && !app.agent.busy && length > 0 && length <= MAX_MESSAGE_UTF16;
                if primary_button(ui, t!(app.locale, "Send"), palette, enabled)
                    || (keyboard_send && enabled)
                {
                    send_message(app);
                }
            });
        });
}

/// Read-only access to the web client's thread snapshots. A snapshot is not
/// treated as a writable live thread: the existing API has no revision check.
fn draw_history(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.add_space(8.0);
    ui.label(
        RichText::new(t!(app.locale, "Server conversation history (read-only)"))
            .strong()
            .color(palette.text),
    );
    ui.label(
        RichText::new(t!(
            app.locale,
            "Current desktop conversations are not yet synced to this list."
        ))
        .color(palette.secondary)
        .size(11.0),
    );
    ui.add_space(6.0);
    if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette)
        && !app.agent.history_list.loading
        && let Some(client) = app.client.clone()
    {
        app.agent.history_list.start(
            app.runner
                .spawn(async move { client.agent_threads().await }),
        );
    }
    if app.agent.history_list.loading {
        agent_skeleton(ui, palette, app.settings.reduced_motion, 2);
    }
    if let Some(error) = &app.agent.history_list.error {
        crate::widgets::error_banner(ui, error, palette);
    }
    if let Some(threads) = app.agent.history_list.data.clone() {
        egui::ScrollArea::vertical()
            .id_salt("agent-history-list")
            .max_height(140.0)
            .show(ui, |ui| {
                for thread in threads.iter().take(40) {
                    let title = if thread.title.is_empty() {
                        thread.id.as_str()
                    } else {
                        thread.title.as_str()
                    };
                    if secondary_button(ui, title, palette)
                        && !app.agent.history_detail.loading
                        && let Some(client) = app.client.clone()
                    {
                        let id = thread.id.clone();
                        app.agent.history_detail = Default::default();
                        app.agent.history_detail.start(
                            app.runner
                                .spawn(async move { client.agent_thread(&id).await }),
                        );
                    }
                }
                if threads.is_empty() {
                    ui.label(t!(app.locale, "No saved conversations."));
                }
            });
    }
    ui.add_space(8.0);
    if app.agent.history_detail.loading {
        agent_skeleton(ui, palette, app.settings.reduced_motion, 2);
    }
    if let Some(error) = &app.agent.history_detail.error {
        crate::widgets::error_banner(ui, error, palette);
    }
    if let Some(thread) = &app.agent.history_detail.data {
        ui.label(RichText::new(&thread.title).strong().color(palette.text));
        egui::ScrollArea::vertical()
            .id_salt("agent-history-detail")
            .max_height((ui.available_height() - 15.0).max(100.0))
            .show(ui, |ui| {
                for entry in thread
                    .messages
                    .iter()
                    .rev()
                    .take(80)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                {
                    if entry.role != "user" && entry.role != "assistant" {
                        continue;
                    }
                    let message = ChatMessage {
                        from_user: entry.role == "user",
                        text: entry.text.clone(),
                        ..Default::default()
                    };
                    chat_bubble(ui, palette, app.locale, &message);
                    ui.add_space(8.0);
                }
            });
    }
}

/// Chat-shaped placeholders retain their layout without animating when the
/// user has enabled reduced motion. No real messages are represented here.
fn agent_skeleton(ui: &mut egui::Ui, palette: &Palette, reduced_motion: bool, count: usize) {
    let phase = if reduced_motion {
        0.55
    } else {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(80));
        0.50 + 0.15 * (ui.input(|input| input.time) as f32 * 2.0).sin()
    };
    let width = ui.available_width().max(0.0);
    for index in 0..count {
        let bubble_width = (width * if index % 2 == 0 { 0.86 } else { 0.68 }).min(width);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 58.0), egui::Sense::hover());
        let left = if index % 2 == 0 {
            rect.left()
        } else {
            rect.right() - bubble_width
        };
        let bubble =
            egui::Rect::from_min_size(egui::pos2(left, rect.top()), egui::vec2(bubble_width, 50.0));
        ui.painter()
            .rect_filled(bubble, 12.0, palette.surface_active.gamma_multiply(phase));
        for (y, ratio) in [(13.0, 0.72), (29.0, 0.46)] {
            let line = egui::Rect::from_min_size(
                egui::pos2(left + 10.0_f32.min(bubble_width), rect.top() + y),
                egui::vec2((bubble_width - 20.0).max(0.0) * ratio, 7.0),
            );
            ui.painter()
                .rect_filled(line, 4.0, palette.secondary.gamma_multiply(0.25));
        }
    }
}

fn chat_bubble(
    ui: &mut egui::Ui,
    palette: &Palette,
    locale: crate::i18n::Locale,
    message: &ChatMessage,
) {
    let fill = if message.from_user {
        palette.accent.gamma_multiply(0.18)
    } else {
        palette.surface
    };
    egui::Frame::NONE
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, palette.outline))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icon(
                    ui,
                    if message.from_user {
                        Icon::User
                    } else {
                        Icon::Bot
                    },
                    palette.accent,
                    15.0,
                );
                ui.label(
                    RichText::new(if message.from_user {
                        t!(locale, "You")
                    } else {
                        "Atlas Agent".into()
                    })
                    .strong()
                    .color(palette.text)
                    .size(12.0),
                );
            });
            ui.add_space(4.0);
            ui.label(RichText::new(&message.text).color(palette.text).size(13.5));
            if !message.tools_used.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "{}: {}",
                        t!(locale, "Tools used"),
                        message.tools_used.join(", ")
                    ))
                    .color(palette.secondary)
                    .size(11.0),
                );
            }
            if !message.proposals.is_empty() {
                ui.add_space(5.0);
                egui::CollapsingHeader::new(format!(
                    "{} ({})",
                    t!(locale, "Action proposals"),
                    message.proposals.len()
                ))
                .id_salt(ui.next_auto_id())
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(t!(
                            locale,
                            "Review only: these proposals are not approved by this app."
                        ))
                        .color(palette.secondary)
                        .size(12.0),
                    );
                    for proposal in &message.proposals {
                        let formatted = serde_json::to_string_pretty(proposal).unwrap_or_default();
                        let excerpt: String = formatted.chars().take(4000).collect();
                        ui.separator();
                        ui.label(
                            RichText::new(excerpt)
                                .monospace()
                                .color(palette.text)
                                .size(11.0),
                        );
                        if formatted.chars().count() > 4000 {
                            ui.label(
                                RichText::new(t!(locale, "Details truncated."))
                                    .color(palette.secondary),
                            );
                        }
                    }
                });
            }
            if !message.from_user && secondary_button(ui, t!(locale, "Copy"), palette) {
                ui.ctx().copy_text(message.text.clone());
            }
        });
}

fn send_message(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let text = app.agent.draft.trim().to_owned();
    if text.is_empty()
        || text.encode_utf16().count() > MAX_MESSAGE_UTF16
        || app.agent.busy
        || app
            .agent
            .policy
            .as_ref()
            .is_some_and(|policy| policy.blocked)
    {
        return;
    }
    if app.agent.thread_id.is_empty() {
        app.agent.thread_id = format!(
            "desktop-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        );
    }
    let thread_id = app.agent.thread_id.clone();
    let language = app.locale.tag().to_owned();
    app.agent.push_message(true, text.clone());
    app.agent.draft.clear();
    app.agent.busy = true;
    app.agent.job = Some(
        app.runner
            .spawn(async move { client.agent_chat(&text, &thread_id, &language).await }),
    );
}
