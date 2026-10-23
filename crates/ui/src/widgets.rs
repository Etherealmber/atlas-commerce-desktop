//! Small reusable components: pills, cards, dialogs, buttons and banners.
//! They translate the web app's directives (compact pills, right-aligned
//! dialog actions, one zinc palette) into egui.

use egui::{Color32, RichText, Sense, Stroke, UiBuilder};

use crate::icons::Icon;
use crate::theme::Palette;

/// Draws content inside a fixed rectangle without moving the parent's
/// cursor: overlays, toasts and the custom title bar live outside the flow.
pub fn overlay_at<R>(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect));
    content(&mut child)
}

/// A compact status pill such as the web app's stock badges.
pub fn pill(ui: &mut egui::Ui, text: impl AsRef<str>, fill: Color32, text_color: Color32) {
    egui::Frame::NONE
        .fill(fill)
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(10, 3))
        .stroke(Stroke::new(1.0, text_color.gamma_multiply(0.25)))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text.as_ref())
                    .color(text_color)
                    .size(12.0)
                    .strong(),
            );
        });
}

/// Draws a small square icon tinted for the current palette.
pub fn icon(ui: &mut egui::Ui, which: Icon, tint: Color32, size: f32) {
    ui.add(which.image(tint, size));
}

/// A primary accent button (right-aligned in dialogs by convention).
pub fn primary_button(
    ui: &mut egui::Ui,
    text: impl AsRef<str>,
    palette: &Palette,
    enabled: bool,
) -> bool {
    let button = egui::Button::new(
        RichText::new(text.as_ref())
            .color(palette.on_accent)
            .strong(),
    )
    .fill(if enabled {
        palette.accent
    } else {
        palette.surface_active
    })
    .corner_radius(8.0);
    ui.add_enabled(enabled, button).clicked()
}

/// A neutral secondary button.
pub fn secondary_button(ui: &mut egui::Ui, text: impl AsRef<str>, palette: &Palette) -> bool {
    let button = egui::Button::new(RichText::new(text.as_ref()).color(palette.text))
        .fill(palette.surface)
        .stroke(Stroke::new(1.0, palette.outline))
        .corner_radius(8.0);
    ui.add(button).clicked()
}

/// A destructive button.
pub fn danger_button(
    ui: &mut egui::Ui,
    text: impl AsRef<str>,
    palette: &Palette,
    enabled: bool,
) -> bool {
    let button = egui::Button::new(RichText::new(text.as_ref()).color(Color32::WHITE).strong())
        .fill(if enabled {
            palette.danger
        } else {
            palette.surface_active
        })
        .corner_radius(8.0);
    ui.add_enabled(enabled, button).clicked()
}

/// A surface card with the palette's outline.
pub fn card(palette: &Palette) -> egui::Frame {
    egui::Frame::NONE
        .fill(palette.surface)
        .stroke(Stroke::new(1.0, palette.outline))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::same(16))
}

/// A banner showing a verbatim worker error (or a local message).
pub fn error_banner(ui: &mut egui::Ui, message: impl AsRef<str>, palette: &Palette) {
    egui::Frame::NONE
        .fill(palette.danger.gamma_multiply(0.14))
        .stroke(Stroke::new(1.0, palette.danger.gamma_multiply(0.6)))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icon(ui, Icon::CircleAlert, palette.danger, 18.0);
                ui.label(
                    RichText::new(message.as_ref())
                        .color(palette.text)
                        .size(13.5),
                );
            });
        });
}

/// A neutral information banner.
pub fn info_banner(ui: &mut egui::Ui, message: impl AsRef<str>, palette: &Palette) {
    egui::Frame::NONE
        .fill(palette.accent.gamma_multiply(0.12))
        .stroke(Stroke::new(1.0, palette.accent.gamma_multiply(0.55)))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icon(ui, Icon::Info, palette.accent, 18.0);
                ui.label(
                    RichText::new(message.as_ref())
                        .color(palette.text)
                        .size(13.5),
                );
            });
        });
}

/// A page/section title with an optional icon.
pub fn page_title(ui: &mut egui::Ui, title: impl AsRef<str>, palette: &Palette) {
    ui.heading(
        RichText::new(title.as_ref())
            .color(palette.text)
            .size(22.0)
            .strong(),
    );
}

/// A labelled single-line field.
pub fn labelled_field(
    ui: &mut egui::Ui,
    label: impl AsRef<str>,
    value: &mut String,
    palette: &Palette,
    password: bool,
) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(label.as_ref())
                .color(palette.secondary)
                .size(12.5),
        );
        let edit = egui::TextEdit::singleline(value)
            .margin(egui::Margin::symmetric(10, 8))
            .clip_text(true);
        let edit = if password { edit.password(true) } else { edit };
        ui.add_sized([ui.available_width(), 34.0], edit);
    });
}

/// Right-aligned dialog action row (the web app always right-aligns them).
pub fn dialog_actions(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.add_space(4.0);
        content(ui);
    });
}

/// A modal-ish centered frame used by every dialog.
pub fn dialog_frame(palette: &Palette) -> egui::Frame {
    egui::Frame::NONE
        .fill(palette.overlay)
        .stroke(Stroke::new(1.0, palette.outline))
        .corner_radius(16.0)
        .inner_margin(egui::Margin::same(22))
}

/// An empty-state block (icon + title + hint).
pub fn empty_state(
    ui: &mut egui::Ui,
    which: Icon,
    title: impl AsRef<str>,
    hint: impl AsRef<str>,
    palette: &Palette,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(28.0);
        icon(ui, which, palette.dim, 40.0);
        ui.add_space(10.0);
        ui.label(
            RichText::new(title.as_ref())
                .color(palette.secondary)
                .strong(),
        );
        ui.add_space(4.0);
        ui.label(RichText::new(hint.as_ref()).color(palette.dim).size(13.0));
    });
}

/// A clickable row (used by lists and menus).
pub fn row_button(ui: &mut egui::Ui, palette: &Palette, height: f32) -> egui::Response {
    let response = ui.allocate_response(egui::vec2(ui.available_width(), height), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(response.rect, 8.0, palette.surface_hover);
    }
    response
}
