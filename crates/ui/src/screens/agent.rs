//! Agent IA: one persistent conversation per account, with the server-side
//! policy (quota, block state) shown above the transcript.

use egui::RichText;

use crate::icons::Icon;
use crate::state::{AtlasApp, ChatMessage};
use crate::t;
use crate::theme::Palette;
use crate::widgets::{card, empty_state, icon, info_banner, page_title, pill, primary_button};

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    // Load the policy once per shell session.
    if app.agent.policy.is_none()
        && app.agent.job_policy.is_none()
        && app.client.is_some()
        && let Some(client) = app.client.clone()
    {
        app.agent.job_policy =
            Some(app.runner.spawn(async move { client.agent_policy().await }));
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        page_title(ui, t!(app.locale, "AI Agent"), palette);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            icon(ui, Icon::Bot, palette.accent, 22.0);
        });
    });
    ui.add_space(12.0);

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
        ui.set_min_height(360.0);
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

    ui.add_space(10.0);
    let blocked = app
        .agent
        .policy
        .as_ref()
        .is_some_and(|policy| policy.blocked);
    ui.horizontal(|ui| {
        let response = ui.add(
            egui::TextEdit::singleline(&mut app.agent.draft)
                .hint_text(t!(app.locale, "Write a message…"))
                .margin(egui::Margin::symmetric(12, 10))
                .desired_width(ui.available_width() - 150.0),
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
    app.agent.messages.push(ChatMessage {
        from_user: true,
        text: text.clone(),
    });
    app.agent.draft.clear();
    app.agent.busy = true;
    app.agent.job = Some(
        app.runner
            .spawn(async move { client.agent_chat(&text, &thread_id).await }),
    );
}
