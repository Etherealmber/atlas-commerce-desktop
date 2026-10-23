//! The visual identity: one zinc-dark palette (the web app's design
//! directive — no white flashes, compact pills, a single neutral palette),
//! a light counterpart, and editable JSON palette files through fastframe.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use egui::{Color32, CornerRadius, Stroke, Style, Visuals};
use fastframe_theme::{Base, Palette as PaletteTrait, parse_palette};

/// The app palette: the sixteen fastframe base colours plus the dark flag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub window: Color32,
    pub panel: Color32,
    pub surface: Color32,
    pub surface_hover: Color32,
    pub surface_active: Color32,
    pub outline: Color32,
    pub text: Color32,
    pub secondary: Color32,
    pub dim: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub on_accent: Color32,
    pub danger: Color32,
    pub warning: Color32,
    pub overlay: Color32,
    pub shadow: Color32,
}

impl Palette {
    /// The product default: zinc-950 dark with an amber accent.
    #[must_use]
    pub fn dark() -> Self {
        Self {
            dark: true,
            window: hex(0x09, 0x09, 0x0b),
            panel: hex(0x13, 0x13, 0x16),
            surface: hex(0x18, 0x18, 0x1b),
            surface_hover: hex(0x27, 0x27, 0x2a),
            surface_active: hex(0x3f, 0x3f, 0x46),
            outline: hex(0x2a, 0x2a, 0x2e),
            text: hex(0xfa, 0xfa, 0xfa),
            secondary: hex(0xa1, 0xa1, 0xaa),
            dim: hex(0x71, 0x71, 0x7a),
            accent: hex(0xf5, 0x9e, 0x0b),
            accent_hover: hex(0xfb, 0xbf, 0x24),
            on_accent: hex(0x1c, 0x12, 0x05),
            danger: hex(0xf8, 0x71, 0x71),
            warning: hex(0xfb, 0xbf, 0x24),
            overlay: hex(0x18, 0x18, 0x1b),
            shadow: Color32::from_black_alpha(160),
        }
    }

    /// The light counterpart, zinc-50 based.
    #[must_use]
    pub fn light() -> Self {
        Self {
            dark: false,
            window: hex(0xfa, 0xfa, 0xfa),
            panel: hex(0xff, 0xff, 0xff),
            surface: hex(0xf4, 0xf4, 0xf5),
            surface_hover: hex(0xe4, 0xe4, 0xe7),
            surface_active: hex(0xd4, 0xd4, 0xd8),
            outline: hex(0xe4, 0xe4, 0xe7),
            text: hex(0x18, 0x18, 0x1b),
            secondary: hex(0x52, 0x52, 0x5b),
            dim: hex(0x71, 0x71, 0x7a),
            accent: hex(0xd9, 0x77, 0x06),
            accent_hover: hex(0xb4, 0x53, 0x09),
            on_accent: hex(0xff, 0xff, 0xff),
            danger: hex(0xdc, 0x26, 0x26),
            warning: hex(0xd9, 0x77, 0x06),
            overlay: hex(0xff, 0xff, 0xff),
            shadow: Color32::from_black_alpha(28),
        }
    }
}

const fn hex(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

impl PaletteTrait for Palette {
    fn base(base: Base) -> Self {
        match base {
            Base::Dark => Self::dark(),
            Base::Light => Self::light(),
        }
    }

    fn set(&mut self, name: &str, color: Color32) -> bool {
        let slot = match name {
            "window" => &mut self.window,
            "panel" => &mut self.panel,
            "surface" => &mut self.surface,
            "surface_hover" => &mut self.surface_hover,
            "surface_active" => &mut self.surface_active,
            "outline" => &mut self.outline,
            "text" => &mut self.text,
            "secondary" => &mut self.secondary,
            "dim" => &mut self.dim,
            "accent" => &mut self.accent,
            "accent_hover" => &mut self.accent_hover,
            "on_accent" => &mut self.on_accent,
            "danger" => &mut self.danger,
            "warning" => &mut self.warning,
            "overlay" => &mut self.overlay,
            "shadow" => &mut self.shadow,
            _ => return false,
        };
        *slot = color;
        true
    }
}

/// Resolves a mode against the operating-system appearance.
#[must_use]
pub fn resolve(dark: bool) -> Palette {
    if dark {
        Palette::dark()
    } else {
        Palette::light()
    }
}

/// Reads a custom JSON palette file, or explains why it is invalid.
///
/// # Errors
/// Returns the parser's message (unknown colour, bad hex…).
pub fn load_custom(text: &str) -> Result<Palette, String> {
    parse_palette::<Palette>(text)
}

/// Installs Inter (with the platform text rendering tweaks) and the icon
/// loaders once per context.
pub fn install_fonts_and_icons(ctx: &egui::Context) {
    let mut fonts = fastframe_fonts::FontSetup::default().definitions();
    fastframe_text::detect().apply_to(&mut fonts);
    ctx.set_fonts(fonts);
    egui_extras::install_image_loaders(ctx);
    fastframe_icons::install::<crate::icons::Icon>(ctx);
    fastframe_titlebar::install(ctx);
}

/// Applies the palette to every egui widget so dialogs, menus and inputs
/// agree with the custom views.
pub fn apply(ctx: &egui::Context, palette: &Palette) {
    let mut style = (*ctx.global_style()).clone();
    apply_to_style(&mut style, palette);
    ctx.set_global_style(style);
}

fn apply_to_style(style: &mut Style, palette: &Palette) {
    let visuals = &mut style.visuals;
    *visuals = if palette.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    visuals.dark_mode = palette.dark;
    fastframe_text::detect().apply_to_visuals(visuals);

    visuals.panel_fill = palette.window;
    visuals.window_fill = palette.overlay;
    visuals.extreme_bg_color = palette.surface;
    visuals.faint_bg_color = palette.surface;
    visuals.code_bg_color = palette.surface;
    visuals.override_text_color = Some(palette.text);
    visuals.weak_text_color = Some(palette.secondary);
    visuals.hyperlink_color = palette.accent;
    visuals.selection.bg_fill = palette.accent.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, palette.accent);
    visuals.window_stroke = Stroke::new(1.0, palette.outline);
    visuals.window_shadow.color = palette.shadow;
    visuals.popup_shadow.color = palette.shadow;
    visuals.striped = false;

    let corner = CornerRadius::same(8);
    visuals.widgets.noninteractive.corner_radius = corner;
    visuals.widgets.noninteractive.bg_fill = palette.surface;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text);

    visuals.widgets.inactive.corner_radius = corner;
    visuals.widgets.inactive.bg_fill = palette.surface;
    visuals.widgets.inactive.weak_bg_fill = palette.surface;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, palette.secondary);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, palette.outline);

    visuals.widgets.hovered.corner_radius = corner;
    visuals.widgets.hovered.bg_fill = palette.surface_hover;
    visuals.widgets.hovered.weak_bg_fill = palette.surface_hover;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.outline);

    visuals.widgets.active.corner_radius = corner;
    visuals.widgets.active.bg_fill = palette.surface_active;
    visuals.widgets.active.weak_bg_fill = palette.surface_active;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, palette.outline);

    visuals.widgets.open.bg_fill = palette.surface_hover;
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, palette.text);

    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    style.spacing.window_margin = egui::Margin::same(18);
}

/// The names a JSON palette may set, for the settings documentation.
#[must_use]
pub fn editable_color_names() -> BTreeSet<&'static str> {
    fastframe_theme::BASE_COLORS.iter().copied().collect()
}

/// Serializes a palette back to the editable JSON shape.
#[must_use]
pub fn palette_to_json(palette: &Palette) -> serde_json::Value {
    let mut colors = BTreeMap::new();
    colors.insert("window", palette.window);
    colors.insert("panel", palette.panel);
    colors.insert("surface", palette.surface);
    colors.insert("surface_hover", palette.surface_hover);
    colors.insert("surface_active", palette.surface_active);
    colors.insert("outline", palette.outline);
    colors.insert("text", palette.text);
    colors.insert("secondary", palette.secondary);
    colors.insert("dim", palette.dim);
    colors.insert("accent", palette.accent);
    colors.insert("accent_hover", palette.accent_hover);
    colors.insert("on_accent", palette.on_accent);
    colors.insert("danger", palette.danger);
    colors.insert("warning", palette.warning);
    colors.insert("overlay", palette.overlay);
    colors.insert("shadow", palette.shadow);
    serde_json::json!({
        "base": if palette.dark { "dark" } else { "light" },
        "colors": colors.into_iter().map(|(key, value)| (key.to_owned(), color_hex(value))).collect::<BTreeMap<_,_>>(),
    })
}

#[must_use]
fn color_hex(color: Color32) -> String {
    let [r, g, b, _] = color.to_array();
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_is_the_default() {
        let palette = Palette::dark();
        assert!(palette.dark);
        assert_ne!(palette.accent, palette.text);
    }

    #[test]
    fn json_palettes_round_trip_through_the_fastframe_parser() {
        let json = r##"{
            "base": "dark",
            "colors": { "accent": "#14b8a6", "panel": "#101014" }
        }"##;
        let palette = load_custom(json).expect("the palette parses");
        assert!(palette.dark);
        assert_eq!(palette.accent, hex(0x14, 0xb8, 0xa6));
        assert_eq!(palette.panel, hex(0x10, 0x10, 0x14));
        // Unset colours keep the dark base.
        assert_eq!(palette.window, Palette::dark().window);
    }

    #[test]
    fn invalid_palette_files_are_rejected() {
        assert!(load_custom(r##"{"colors":{"nope":"#fff"}}"##).is_err());
        assert!(load_custom(r##"{"colors":{"accent":"blue"}}"##).is_err());
    }

    #[test]
    fn every_base_color_is_editable() {
        let names = editable_color_names();
        assert!(names.contains("accent") && names.contains("window"));
        assert_eq!(names.len(), 16);
    }
}
