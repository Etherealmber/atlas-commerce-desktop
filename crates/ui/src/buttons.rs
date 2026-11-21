//! Shared Fluent-inspired controls used by every native screen.
//! Colours are derived from the current palette so dark, light and the
//! Windows system accent use the same geometry and accessible states.
use crate::theme::{self, Palette};
use egui::{Color32, RichText, Stroke, TextWrapMode};

struct FilledStyle {
    fill: Color32,
    hover: Color32,
    foreground: Color32,
    disabled_text: Color32,
    height: f32,
}

fn filled_button(ui: &mut egui::Ui, text: &str, style: FilledStyle, enabled: bool) -> bool {
    ui.scope(|ui| {
        let visual = &mut ui.style_mut().visuals.widgets;
        for state in [&mut visual.inactive, &mut visual.active, &mut visual.open] {
            state.bg_fill = style.fill;
            state.weak_bg_fill = style.fill;
            state.bg_stroke = Stroke::NONE;
        }
        let hover = theme::readable_hover(style.fill, style.hover, style.foreground);
        visual.hovered.bg_fill = hover;
        visual.hovered.weak_bg_fill = hover;
        visual.hovered.bg_stroke = Stroke::NONE;
        let label_color = if enabled {
            style.foreground
        } else {
            style.disabled_text
        };
        let button = egui::Button::new(RichText::new(text).color(label_color).strong())
            .wrap_mode(TextWrapMode::Extend)
            .min_size(egui::vec2(0.0, style.height))
            .corner_radius(theme::RADIUS_CONTROL as f32);
        let response = ui.add_enabled(enabled, button);
        #[cfg(feature = "visual-fixtures")]
        if std::env::var_os("ATLAS_FIXTURE_WIZARD").is_some() && matches!(text, "Suivant" | "Next")
        {
            let center = response.rect.center();
            let id = egui::Id::new("fixture-wizard-next-position");
            if ui.ctx().data(|data| data.get_temp::<egui::Pos2>(id)) != Some(center) {
                ui.ctx().data_mut(|data| data.insert_temp(id, center));
                println!("FIXTURE_NEXT_POSITION {:.0} {:.0}", center.x, center.y);
            }
        }
        response.clicked()
    })
    .inner
}

/// Primary action using the current accent, including hover and disabled states.
pub fn primary_button(
    ui: &mut egui::Ui,
    text: impl AsRef<str>,
    palette: &Palette,
    enabled: bool,
) -> bool {
    let fill = if enabled {
        palette.accent
    } else {
        palette.surface_active
    };
    filled_button(
        ui,
        text.as_ref(),
        FilledStyle {
            fill,
            hover: palette.accent_hover,
            foreground: palette.on_accent,
            disabled_text: palette.secondary,
            height: theme::PRIMARY_BUTTON_HEIGHT,
        },
        enabled,
    )
}

/// Neutral control; the centralized egui theme supplies hover and pressed fills.
pub fn secondary_button(ui: &mut egui::Ui, text: impl AsRef<str>, palette: &Palette) -> bool {
    let button = egui::Button::new(RichText::new(text.as_ref()).color(palette.text))
        .wrap_mode(TextWrapMode::Extend)
        .min_size(egui::vec2(0.0, theme::TARGET_MIN_HEIGHT))
        .stroke(Stroke::new(1.0, palette.outline))
        .corner_radius(theme::RADIUS_CONTROL as f32);
    ui.add(button).clicked()
}

/// Destructive action with text chosen for the danger fill in either mode.
pub fn danger_button(
    ui: &mut egui::Ui,
    text: impl AsRef<str>,
    palette: &Palette,
    enabled: bool,
) -> bool {
    let fill = if enabled {
        palette.danger
    } else {
        palette.surface_active
    };
    let foreground = theme::contrasting_text(palette.danger);
    filled_button(
        ui,
        text.as_ref(),
        FilledStyle {
            fill,
            hover: fill,
            foreground,
            disabled_text: palette.secondary,
            height: theme::TARGET_MIN_HEIGHT,
        },
        enabled,
    )
}
