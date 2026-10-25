//! Every full screen the app draws.

pub mod agent;
pub mod auth;
pub mod catalog;
pub mod clients;
pub mod dashboard;
pub mod delivery_notes;
pub mod expenses;
pub mod notifications;
pub mod products;
pub mod sales;
pub mod scan;
pub mod settings;
pub mod shell;
pub mod vendors;

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

/// A horizontally-centered loading block for page and card loading states,
/// so a fetch no longer renders as three dots stuck to the top-left corner.
/// The inline [`spinner`] stays available for chat-style flows.
pub fn centered_spinner(ui: &mut Ui, palette: &Palette) {
    ui.vertical_centered(|ui| {
        ui.add_space(28.0);
        spinner(ui, palette);
        ui.add_space(28.0);
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
    // A bottom-anchored toast must never grow past the window it lives in.
    let max_height = (screen.height() - 32.0).max(80.0);
    let mut y = screen.bottom() - 16.0;
    // Matches the theme's 9 px scrollbar plus a pixel of breathing room.
    const SCROLLBAR_RESERVE: f32 = 10.0;
    let toasts: Vec<String> = app.notices.clone();
    for (index, toast) in toasts.iter().rev().enumerate() {
        let frame = egui::Frame::NONE
            .fill(palette.overlay)
            .stroke(egui::Stroke::new(1.0, palette.outline))
            .corner_radius(10.0)
            .shadow(egui::Shadow {
                offset: [0, 4],
                blur: 16,
                spread: 0,
                color: palette.shadow,
            })
            .inner_margin(egui::Margin::symmetric(14, 9));
        // Measure the toast with the real wrap width first: a no-wrap
        // galley made long messages overflow the fixed-height frame and get
        // clipped. The same galley is painted back at the content origin so
        // wrapping cannot reflow differently inside the frame.
        let wrap_width = max_width - 28.0;
        let galley = ui.painter().layout(
            toast.clone(),
            egui::FontId::proportional(13.0),
            palette.text,
            wrap_width,
        );
        let inner_height = galley.size().y.min(max_height - 18.0);
        let scrolls = galley.size().y > inner_height + 0.5;
        // A scrolling toast spans the full allowed width; an ordinary one
        // hugs its measured galley.
        let frame_width = if scrolls {
            wrap_width + 28.0
        } else {
            galley.size().x + 28.0
        };
        let size = egui::vec2(frame_width, inner_height + 18.0);
        y -= size.y;
        let rect = egui::Rect::from_min_size(egui::pos2(screen.left() + 16.0, y), size);
        crate::widgets::overlay_at(ui, rect, |ui| {
            frame.show(ui, |ui| {
                if scrolls {
                    // Edge case: an unusually long notice stays reachable
                    // inside a bounded, wheel-scrollable toast. The salt
                    // keeps sibling toasts from sharing scroll memory, and
                    // the reserved width keeps text clear of the floating
                    // scrollbar at the right edge.
                    egui::ScrollArea::vertical()
                        .id_salt(egui::Id::new(("notice-scroll", index)))
                        .max_height(inner_height)
                        .show(ui, |ui| {
                            ui.set_width(wrap_width - SCROLLBAR_RESERVE);
                            ui.label(RichText::new(toast).color(palette.text).size(13.0));
                        });
                } else {
                    // The fork's allocate_exact_size returns (Rect, Response).
                    let (content_rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
                    ui.painter().galley(content_rect.min, galley, palette.text);
                }
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
