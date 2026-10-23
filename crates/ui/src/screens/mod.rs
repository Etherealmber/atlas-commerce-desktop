//! Every full screen the app draws.

pub mod agent;
pub mod auth;
pub mod clients;
pub mod dashboard;
pub mod notifications;
pub mod products;
pub mod sales;
pub mod settings;
pub mod shell;

use std::time::Instant;

use egui::{Color32, Response, RichText, Sense, Ui};

use crate::state::AtlasApp;
use crate::theme::Palette;

/// A small spinning indicator (three pulsing dots, no textures).
pub fn spinner(ui: &mut Ui, palette: &Palette) {
    let time = ui.input(|input| input.time) as f32;
    ui.horizontal(|ui| {
        for index in 0..3 {
            let phase = (time * 4.0 + index as f32 * 0.9).sin() * 0.5 + 0.5;
            let color = palette.secondary.gamma_multiply(0.45 + 0.55 * phase);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
            ui.painter().circle_filled(rect.center(), 3.5, color);
        }
    });
}

/// Renders the transient notice stack (bottom-left toasts).
pub fn notices(app: &mut AtlasApp, ui: &mut Ui, palette: &Palette) {
    if app.notices.is_empty() {
        return;
    }
    // The stack has a bounded lifetime: clear it once the deadline set by
    // [`AtlasApp::notice`] passes, rather than on the first frame drawn.
    if app.notices_expire_at.is_some_and(|at| Instant::now() >= at) {
        app.notices.clear();
        app.notices_expire_at = None;
        return;
    }
    let screen = ui.ctx().input(|i| i.viewport_rect());
    let max_width = (screen.width() * 0.42).clamp(220.0, 420.0);
    let mut y = screen.bottom() - 16.0;
    let toasts: Vec<String> = app.notices.clone();
    for toast in toasts.iter().rev() {
        let frame = egui::Frame::NONE
            .fill(palette.overlay)
            .stroke(egui::Stroke::new(1.0, palette.outline))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::symmetric(14, 9));
        // Measure the toast first, then place it over the screen's
        // bottom-left corner without touching the in-flow cursor.
        let galley = ui.painter().layout_no_wrap(
            toast.clone(),
            egui::FontId::proportional(13.0),
            palette.text,
        );
        let width = galley.size().x.min(max_width);
        let size = egui::vec2(width + 28.0, galley.size().y + 18.0);
        y -= size.y;
        let rect = egui::Rect::from_min_size(egui::pos2(screen.left() + 16.0, y), size);
        crate::widgets::overlay_at(ui, rect, |ui| {
            frame.show(ui, |ui| {
                ui.label(RichText::new(toast).color(palette.text).size(13.0));
            });
        });
        y -= 8.0;
    }
    // Wake the UI exactly when the stack should disappear, even if no
    // other input or job requests a repaint.
    if let Some(at) = app.notices_expire_at {
        ui.ctx()
            .request_repaint_after(at.saturating_duration_since(Instant::now()));
    }
}

/// A header row with an optional back chevron.
pub fn back_header(ui: &mut Ui, title: impl AsRef<str>, palette: &Palette) -> Option<Response> {
    ui.horizontal(|ui| {
        let back = ui
            .add(
                egui::Button::new(egui::RichText::new("‹").color(palette.secondary).size(22.0))
                    .fill(Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE),
            )
            .on_hover_text("Retour");
        ui.label(
            RichText::new(title.as_ref())
                .color(palette.text)
                .size(18.0)
                .strong(),
        );
        Some(back)
    })
    .inner
}

/// French label for a worker severity.
#[must_use]
pub fn severity_label(severity: Option<&str>) -> &'static str {
    match severity {
        Some("high") => "Haute",
        Some("medium") => "Moyenne",
        Some("low") => "Basse",
        _ => "Info",
    }
}
