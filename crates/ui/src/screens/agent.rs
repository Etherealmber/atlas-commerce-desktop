//! Agent IA: one persistent conversation per account, with the server-side
//! policy (quota, block state) shown above the transcript.

use std::time::Instant;

use egui::RichText;

use crate::icons::Icon;
use crate::state::{AtlasApp, ChatMessage};
use crate::t;
use crate::theme::Palette;
use crate::widgets::{card, empty_state, info_banner, pill, primary_button, secondary_button};

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    // Load the policy once per shell session; a failed attempt waits out the
    // backoff before another request is spawned (see POLICY_RETRY_BACKOFF).
    let retry_gate_open = app
        .agent
        .policy_retry_at
        .is_none_or(|retry_at| retry_at <= Instant::now());
    if app.agent.policy.is_none()
        && app.agent.job_policy.is_none()
        && retry_gate_open
        && app.client.is_some()
        && let Some(client) = app.client.clone()
    {
        app.agent.job_policy = Some(app.runner.spawn(async move { client.agent_policy().await }));
    }

    ui.add_space(8.0);
    if let crate::state::Screen::Shell(route) = app.screen {
        let (_, name) = super::shell::route_meta(app.locale, route);
        card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(t!(app.locale, "Current page"))
                    .color(palette.secondary)
                    .size(11.0),
            );
            ui.label(RichText::new(name).color(palette.text).strong());
        });
        ui.add_space(8.0);
    }

    if let Some(policy) = &app.agent.policy {
        let used = atlas_core::format::integer(policy.daily_used as i64);
        let limit = atlas_core::format::integer(policy.daily_limit as i64);
        pill(
            ui,
            format!("{} {used}/{limit}", t!(app.locale, "Daily quota")),
            palette.surface_active,
            palette.secondary,
        );
        if policy.blocked {
            ui.add_space(8.0);
            info_banner(
                ui,
                t!(
                    app.locale,
                    "The AI agent is not available for this account."
                ),
                palette,
            );
        }
    }

    ui.add_space(12.0);
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let transcript_height = (ui.available_height() - 145.0).max(100.0);
        egui::ScrollArea::vertical()
            .max_height(transcript_height)
            .min_scrolled_height(transcript_height)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if app.agent.messages.is_empty() {
                    empty_state(
                        ui,
                        Icon::Bot,
                        t!(app.locale, "Ask anything about your shop"),
                        t!(
                            app.locale,
                            "Stock, sales and product questions are answered from your data."
                        ),
                        palette,
                    );
                } else {
                    for message in &app.agent.messages {
                        chat_bubble(ui, palette, message);
                        ui.add_space(8.0);
                    }
                }
                if app.agent.busy {
                    ui.add_space(4.0);
                    super::spinner(ui, palette);
                }
            });
    });

    ui.add_space(10.0);
    let blocked = app
        .agent
        .policy
        .as_ref()
        .is_some_and(|policy| policy.blocked);
    ui.horizontal(|ui| {
        let draft_width = (ui.available_width() - 100.0).max(40.0);
        let response = ui.add_sized(
            [draft_width, crate::theme::TARGET_MIN_HEIGHT],
            egui::TextEdit::singleline(&mut app.agent.draft)
                .hint_text(t!(app.locale, "Write a message…"))
                .margin(egui::Margin::symmetric(14, 10))
                .clip_text(true),
        );
        let enter = response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        let send = primary_button(
            ui,
            t!(app.locale, "Send"),
            palette,
            !blocked && !app.agent.busy && !app.agent.draft.trim().is_empty(),
        );
        if (send || enter) && !blocked && !app.agent.busy {
            send_message(app);
        }
    });

    let new_chat = secondary_button(ui, t!(app.locale, "New conversation"), palette);
    if new_chat && !app.agent.busy {
        app.agent.messages.clear();
        app.agent.thread_id = format!(
            "desktop-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        );
    }
}

fn chat_bubble(ui: &mut egui::Ui, palette: &Palette, message: &ChatMessage) {
    let fill = if message.from_user {
        palette.accent.gamma_multiply(0.18)
    } else {
        palette.surface
    };
    egui::Frame::NONE
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, palette.outline))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_min_width(120.0);
            ui.set_max_width(ui.available_width() * 0.92);
            ui.label(
                RichText::new(message.text.clone())
                    .color(palette.text)
                    .size(13.5),
            );
        });
}

fn send_message(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let text = app.agent.draft.trim().to_owned();
    if text.is_empty() {
        return;
    }
    if app.agent.thread_id.is_empty() {
        app.agent.thread_id = format!(
            "desktop-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        );
    }
    let thread_id = app.agent.thread_id.clone();
    app.agent.push_message(true, text.clone());
    app.agent.draft.clear();
    app.agent.busy = true;
    app.agent.job = Some(
        app.runner
            .spawn(async move { client.agent_chat(&text, &thread_id).await }),
    );
}
