//! Small reusable components: pills, cards, dialogs, buttons and banners.
//! They translate the web app's directives (compact pills, right-aligned
//! dialog actions, one zinc palette) into egui.

use egui::{Color32, RichText, Sense, Stroke, TextWrapMode, UiBuilder};

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

/// Runs `content` with text wrapping switched off. Action clusters on card
/// rows (price + button columns) keep their natural width this way instead
/// of being squeezed to a single character per line by a neighbouring label
/// that is allowed to wrap.
pub fn no_wrap_scope<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.scope(|ui| {
        // The fork's Style carries `wrap_mode` (no legacy `wrap: bool`).
        ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
        content(ui)
    })
    .inner
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
    // Extend instead of wrapping: a starved row used to collapse the label
    // one character per line ("Ajuster le stock" rendered vertically).
    .wrap_mode(TextWrapMode::Extend)
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
        .wrap_mode(TextWrapMode::Extend)
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
        .wrap_mode(TextWrapMode::Extend)
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

/// A circular, dark icon button in the Spotifast/Android style: 32 px disk,
/// dimmed when disabled. Used by the shell's top bar (back/forward, bell…).
pub fn icon_circle_button(
    ui: &mut egui::Ui,
    palette: &Palette,
    which: Icon,
    enabled: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::Vec2::splat(30.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let fill = if palette.dark {
        egui::Color32::from_black_alpha(90)
    } else {
        egui::Color32::from_black_alpha(18)
    };
    if enabled && response.hovered() {
        ui.painter()
            .circle_filled(rect.center(), 15.0, palette.surface_hover);
    } else {
        ui.painter().circle_filled(rect.center(), 15.0, fill);
    }
    let tint = if !enabled {
        palette.dim
    } else if response.hovered() {
        palette.text
    } else {
        palette.secondary
    };
    overlay_at(
        ui,
        egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(18.0)),
        |ui| icon(ui, which, tint, 17.0),
    );
    response
}

/// A filled circle showing the user's initial, the monochrome stand-in for
/// profile pictures (avatars are a URL the user provides in their profile).
pub fn avatar_circle(
    ui: &mut egui::Ui,
    palette: &Palette,
    initial: &str,
    size: f32,
    fill: egui::Color32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(size), Sense::click());
    ui.painter().circle_filled(rect.center(), size / 2.0, fill);
    let galley = ui.painter().layout_no_wrap(
        initial.to_owned(),
        egui::FontId::proportional(size * 0.42),
        palette.on_accent,
    );
    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        palette.on_accent,
    );
    response
}

/// An Android-style switch with an animated knob. Returns the click response;
/// the caller owns the bool.
pub fn switch(ui: &mut egui::Ui, value: &mut bool, palette: &Palette) -> egui::Response {
    const WIDTH: f32 = 42.0;
    const HEIGHT: f32 = 24.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(WIDTH, HEIGHT), Sense::click());
    if response.clicked() {
        *value = !*value;
    }
    let t = ui
        .ctx()
        .animate_bool_with_easing(response.id, *value, egui::emath::easing::cubic_out);
    let track_off = palette.surface_active;
    let track = track_off.lerp_to_gamma(palette.accent, t);
    ui.painter().rect_filled(rect, HEIGHT / 2.0, track);
    let knob_radius = HEIGHT / 2.0 - 3.0;
    let x = rect.left() + knob_radius + 3.0 + t * (WIDTH - 2.0 * (knob_radius + 3.0));
    let knob_center = egui::pos2(x, rect.center().y);
    ui.painter()
        .circle_filled(knob_center, knob_radius, egui::Color32::WHITE);
    response
}

/// A bold section header above a settings card, Spotifast-style.
pub fn section_header(ui: &mut egui::Ui, palette: &Palette, title: impl AsRef<str>) {
    ui.add_space(6.0);
    ui.label(
        RichText::new(title.as_ref())
            .color(palette.text)
            .strong()
            .size(19.0),
    );
    ui.add_space(8.0);
}

/// A settings row: bold title (+ optional dim description) on the left, a
/// custom control on the right.
pub fn setting_row<R>(
    ui: &mut egui::Ui,
    palette: &Palette,
    title: impl AsRef<str>,
    description: Option<&str>,
    control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let inner = ui
        .horizontal(|ui| {
            ui.vertical(|ui| {
                ui.add_space(2.0);
                ui.label(
                    RichText::new(title.as_ref())
                        .color(palette.text)
                        .size(13.5)
                        .strong(),
                );
                if let Some(description) = description {
                    ui.add_space(2.0);
                    ui.label(RichText::new(description).color(palette.dim).size(11.5));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control)
        })
        .inner;
    inner.inner
}

/// A title/description row with an animated switch on the right. Returns
/// whether the value changed this frame.
pub fn switch_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    title: impl AsRef<str>,
    description: Option<&str>,
    value: &mut bool,
) -> bool {
    let before = *value;
    setting_row(ui, palette, title, description, |ui| {
        switch(ui, value, palette)
    });
    *value != before
}

/// Segmented pill selector (language, theme, channel, scopes…). `selected`
/// is `None` when no choice exists yet (e.g. a required scope with no
/// preselected default); the clicked value is returned.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    palette: &Palette,
    options: &[(&str, T)],
    selected: Option<T>,
) -> Option<T> {
    let mut picked = None;
    ui.horizontal(|ui| {
        for (label, value) in options {
            let is_selected = selected == Some(*value);
            let button = egui::Button::new(
                RichText::new(*label)
                    .color(if is_selected {
                        palette.on_accent
                    } else {
                        palette.text
                    })
                    .size(12.5),
            )
            .fill(if is_selected {
                palette.accent
            } else {
                palette.surface
            })
            .stroke(Stroke::new(1.0, palette.outline))
            .corner_radius(999.0);
            if ui.add(button).clicked() {
                picked = Some(*value);
            }
        }
    });
    picked
}

/// A floating action button (Android FAB) anchored to the window's bottom
/// right: 56 px accent disk, scale-in entrance and hover lift.
pub fn fab(
    ctx: &egui::Context,
    palette: &Palette,
    which: Icon,
    id_salt: impl std::hash::Hash + Copy + std::fmt::Debug,
    tooltip: &str,
) -> egui::Response {
    let id = egui::Id::new(("atlas-fab", id_salt));
    // Android-style 220 ms scale-in, slower than egui's 83 ms default.
    let entrance = ctx.animate_bool_with_time_and_easing(
        egui::Id::new(("atlas-fab-in", id_salt)),
        true,
        0.22,
        egui::emath::easing::cubic_out,
    );
    egui::Area::new(id)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-24.0, -24.0))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            let full = 56.0;
            let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(full), Sense::click());
            let hover = ctx.animate_bool(
                egui::Id::new(("atlas-fab-hover", id_salt)),
                response.hovered(),
            );
            let scale = entrance * (1.0 + 0.06 * hover);
            let size = full * scale;
            let center = rect.center();
            let draw_rect = egui::Rect::from_center_size(center, egui::Vec2::splat(size));
            // Soft shadow, then the accent disk.
            ui.painter().circle_filled(
                draw_rect.center() + egui::vec2(0.0, 2.0 * scale),
                size / 2.0,
                palette.shadow,
            );
            ui.painter()
                .circle_filled(draw_rect.center(), size / 2.0, palette.accent);
            overlay_at(
                ui,
                egui::Rect::from_center_size(center, egui::Vec2::splat(26.0)),
                |ui| icon(ui, which, palette.on_accent, 24.0),
            );
            response.on_hover_text(tooltip)
        })
        .response
}
