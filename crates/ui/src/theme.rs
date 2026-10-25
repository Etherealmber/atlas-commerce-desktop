//! The visual identity: the Atlas Commerce design system shared with the
//! web ERP (`src/styles/theme.css`) and the Android app (`ui/theme/Theme.kt`).
//! The directive is monochrome: a black/white primary, zinc neutrals, no
//! teal and no saffron/orange brand accent — only the semantic warning and
//! danger tokens keep colour. A dark-first palette, its light counterpart,
//! and editable JSON palette files through fastframe.

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
    /// The product default. Tokens mirror the web app's dark scheme:
    /// `#111113` background, a subtly lifted `#17171a` chrome, `#1c1c1f`
    /// paper, `#27272a` hover, `#2a2a2e` lines, `#f4f4f5` ink, `#a1a1aa`
    /// muted and a white primary carrying near-black (`#0a0a0a`) text.
    #[must_use]
    pub fn dark() -> Self {
        Self {
            dark: true,
            window: hex(0x11, 0x11, 0x13),
            panel: hex(0x17, 0x17, 0x1a),
            surface: hex(0x1c, 0x1c, 0x1f),
            surface_hover: hex(0x27, 0x27, 0x2a),
            surface_active: hex(0x27, 0x27, 0x2a),
            outline: hex(0x2a, 0x2a, 0x2e),
            text: hex(0xf4, 0xf4, 0xf5),
            secondary: hex(0xa1, 0xa1, 0xaa),
            dim: hex(0x71, 0x71, 0x7a),
            accent: hex(0xfa, 0xfa, 0xfa),
            accent_hover: hex(0xe4, 0xe4, 0xe7),
            on_accent: hex(0x0a, 0x0a, 0x0a),
            danger: hex(0xf8, 0x71, 0x71),
            warning: hex(0xfb, 0xbf, 0x24),
            overlay: hex(0x1c, 0x1c, 0x1f),
            shadow: Color32::from_black_alpha(160),
        }
    }

    /// The light counterpart, on the same neutral zinc scale as the dark
    /// scheme: `#f4f4f5` background, white paper, zinc-900 ink, and the
    /// near-black `#0a0a0a` primary with `#fafafa` text.
    #[must_use]
    pub fn light() -> Self {
        Self {
            dark: false,
            window: hex(0xf4, 0xf4, 0xf5),
            panel: hex(0xff, 0xff, 0xff),
            surface: hex(0xff, 0xff, 0xff),
            surface_hover: hex(0xf4, 0xf4, 0xf5),
            surface_active: hex(0xe4, 0xe4, 0xe7),
            outline: hex(0xe4, 0xe4, 0xe7),
            text: hex(0x18, 0x18, 0x1b),
            secondary: hex(0x52, 0x52, 0x5b),
            dim: hex(0x71, 0x71, 0x7a),
            accent: hex(0x0a, 0x0a, 0x0a),
            accent_hover: hex(0x17, 0x17, 0x17),
            on_accent: hex(0xfa, 0xfa, 0xfa),
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
    // Layered elevation, the same recipe ZapFast and Spotifast use: dialogs
    // and windows cast a deep ambient shadow, popups and menus a shorter
    // one. The palette's shadow token is near-invisible on a light window
    // and soft black in the dark scheme, so this stays monochrome.
    visuals.window_corner_radius = CornerRadius::same(14);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.window_shadow = egui::Shadow {
        offset: [0, 12],
        blur: 40,
        spread: 0,
        color: palette.shadow,
    };
    visuals.popup_shadow = egui::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: palette.shadow,
    };
    visuals.striped = false;
    // A 2 px accent caret reads better than the default 1 px muted bar.
    visuals.text_cursor.stroke = Stroke::new(2.0, palette.accent);
    visuals.slider_trailing_fill = true;
    visuals.handle_shape = egui::style::HandleShape::Circle;

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
    // Open popups (combo boxes) must not paint a second box outline around
    // their trigger; the popup shadow already separates them.
    visuals.widgets.open.bg_stroke = Stroke::NONE;

    // No egui hover "grow": expansion makes every button and row look like
    // a web link. The fill change alone signals interactivity.
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.expansion = 0.0;
    }

    // One explicit type scale, matching the reference fastframe apps so the
    // interface does not depend on egui's per-platform defaults.
    use egui::{FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(11.5)),
        (TextStyle::Body, FontId::proportional(14.0)),
        (TextStyle::Button, FontId::proportional(14.0)),
        (TextStyle::Heading, FontId::proportional(22.0)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
    ]
    .into();

    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    style.spacing.window_margin = egui::Margin::same(18);
    style.spacing.menu_margin = egui::Margin::same(6);

    // Native-style scrollbars: invisible until hovered, then a thin floating
    // handle that expands under the cursor instead of a permanent gutter.
    let scroll = &mut style.spacing.scroll;
    scroll.floating = true;
    scroll.bar_width = 9.0;
    scroll.floating_width = 3.5;
    scroll.floating_allocated_width = 0.0;
    scroll.handle_min_length = 28.0;
    scroll.dormant_handle_opacity = 0.0;
    scroll.dormant_background_opacity = 0.0;
    scroll.active_background_opacity = 0.0;
    scroll.active_handle_opacity = 0.55;
    scroll.interact_handle_opacity = 0.85;
    scroll.foreground_color = true;

    // Quick, calm motion (egui default is 0.2 s): hover fades and the FAB
    // entrance share one short duration, respecting the brief's subtle
    // motion directive.
    style.animation_time = 0.12;
    style.interaction.tooltip_delay = 0.4;
    style.url_in_tooltip = false;

    // Native-app behaviour: chrome text (buttons, labels, pills, menu rows,
    // table cells, headings) must never behave like selectable web page text
    // — no highlight drag, no Ctrl+A/Ctrl+C selection rectangle, no I-beam
    // cursor over non-input surfaces. TextEdit fields keep their own text
    // selection independently of this flag, so inputs stay fully selectable.
    style.interaction.selectable_labels = false;
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
            "colors": { "accent": "#fafafa", "panel": "#101014" }
        }"##;
        let palette = load_custom(json).expect("the palette parses");
        assert!(palette.dark);
        assert_eq!(palette.accent, hex(0xfa, 0xfa, 0xfa));
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

    #[test]
    fn chrome_text_is_never_selectable() {
        // Native-app guarantee: labels, buttons and pills must not offer web
        // style text selection. TextEdit fields keep their own selection.
        let mut style = Style::default();
        apply_to_style(&mut style, &Palette::dark());
        assert!(!style.interaction.selectable_labels);
        apply_to_style(&mut style, &Palette::light());
        assert!(!style.interaction.selectable_labels);
    }
}
