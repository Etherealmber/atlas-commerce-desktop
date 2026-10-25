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

/// The app palette: the sixteen fastframe base colours, the financial
/// semantics shared with Android (`AtlasPalette` in `Theme.kt`) and the
/// dark flag.
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
    /// Saffron reserved to badges and counters
    /// (SOURCED-FROM: atlas-commerce-android `Theme.kt:28`). Never used on
    /// brand chrome or buttons; the ink on top of it is near-black.
    pub badge: Color32,
    pub on_badge: Color32,
    /// Elevation colour. Fully transparent in dark mode, matching the web's
    /// `--ac-shadow: none`; in light mode it is the web's
    /// `0 1px 2px rgba(15, 23, 42, 0.04)` token.
    pub shadow: Color32,
    // Financial semantics — tiles and statuses only, never brand chrome.
    /// SOURCED-FROM: atlas-commerce-android `Theme.kt` `AtlasPalette`.
    pub finance_positive: Color32,
    pub finance_positive_soft: Color32,
    pub finance_warning: Color32,
    pub finance_warning_soft: Color32,
    pub finance_danger: Color32,
    pub finance_danger_soft: Color32,
    pub finance_info: Color32,
    pub finance_info_soft: Color32,
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
            // SOURCED-FROM: Theme.kt badge saffron, counters only.
            badge: hex(0xf5, 0x9e, 0x0b),
            on_badge: hex(0x0a, 0x0a, 0x0a),
            // The web dark theme sets --ac-shadow: none.
            shadow: Color32::TRANSPARENT,
            // SOURCED-FROM: atlas-commerce-android Theme.kt DarkPalette.
            finance_positive: hex(0x34, 0xd3, 0x99),
            finance_positive_soft: hex(0x10, 0x36, 0x2a),
            finance_warning: hex(0xfb, 0xbf, 0x24),
            finance_warning_soft: hex(0x3a, 0x2a, 0x0b),
            finance_danger: hex(0xf8, 0x71, 0x71),
            finance_danger_soft: hex(0x3a, 0x14, 0x14),
            finance_info: hex(0x93, 0xc5, 0xfd),
            finance_info_soft: hex(0x16, 0x23, 0x3f),
        }
    }

    /// The light counterpart, using the web `theme.css` tokens exactly:
    /// `#f4f5f7` background, white paper, `#0f172a` ink, `#64748b` muted,
    /// `#e8eaef` lines and the near-black `#0a0a0a` primary with
    /// `#fafafa` text.
    #[must_use]
    pub fn light() -> Self {
        Self {
            dark: false,
            window: hex(0xf4, 0xf5, 0xf7),
            panel: hex(0xff, 0xff, 0xff),
            surface: hex(0xff, 0xff, 0xff),
            surface_hover: hex(0xf1, 0xf5, 0xf9),
            surface_active: hex(0xf1, 0xf5, 0xf9),
            outline: hex(0xe8, 0xea, 0xef),
            text: hex(0x0f, 0x17, 0x2a),
            secondary: hex(0x64, 0x74, 0x8b),
            dim: hex(0x71, 0x71, 0x7a),
            accent: hex(0x0a, 0x0a, 0x0a),
            accent_hover: hex(0x17, 0x17, 0x17),
            on_accent: hex(0xfa, 0xfa, 0xfa),
            danger: hex(0xdc, 0x26, 0x26),
            warning: hex(0xd9, 0x77, 0x06),
            overlay: hex(0xff, 0xff, 0xff),
            // SOURCED-FROM: Theme.kt badge saffron, counters only.
            badge: hex(0xf5, 0x9e, 0x0b),
            on_badge: hex(0x0a, 0x0a, 0x0a),
            // Web light elevation: 0 1px 2px rgba(15, 23, 42, 0.04).
            shadow: Color32::from_rgba_unmultiplied(15, 23, 42, 10),
            // SOURCED-FROM: atlas-commerce-android Theme.kt LightPalette.
            finance_positive: hex(0x0f, 0x7b, 0x4f),
            finance_positive_soft: hex(0xe6, 0xf6, 0xec),
            finance_warning: hex(0xb4, 0x53, 0x09),
            finance_warning_soft: hex(0xfe, 0xf3, 0xc7),
            finance_danger: hex(0xb9, 0x1c, 0x1c),
            finance_danger_soft: hex(0xfe, 0xe2, 0xe2),
            finance_info: hex(0x1d, 0x4e, 0xd8),
            finance_info_soft: hex(0xe0, 0xea, 0xff),
        }
    }
}

const fn hex(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Desktop radius scale (master parity spec §4): chips 999 live inline.
/// Controls (buttons, inputs): 12 px · cards and stat tiles: 14 px ·
/// large panels, windows and modals: 18 px · hero surfaces: 20 px.
pub const RADIUS_CONTROL: u8 = 12;
pub const RADIUS_CARD: u8 = 14;
pub const RADIUS_LARGE: u8 = 18;
pub const RADIUS_HERO: u8 = 20;

/// Minimum interactive target (master parity spec §4): every control is at
/// least 44 px tall; the primary action sits at 48 px (Android primary is
/// 50 dp, 48 is the desktop counterpart in the same reach range).
pub const TARGET_MIN_HEIGHT: f32 = 44.0;
pub const PRIMARY_BUTTON_HEIGHT: f32 = 48.0;

/// The single web elevation token: `0 1px 2px rgba(15,23,42,0.04)` in
/// light mode; the shadow colour is transparent in dark mode
/// (`--ac-shadow: none`), so nothing renders there.
#[must_use]
pub fn elevation_shadow(palette: &Palette) -> egui::Shadow {
    egui::Shadow {
        offset: [0, 1],
        blur: 2,
        spread: 0,
        color: palette.shadow,
    }
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

/// Installs the brand faces (DM Sans, Spline Sans Mono) with the platform
/// text rendering tweaks, plus the icon loaders, once per context.
pub fn install_fonts_and_icons(ctx: &egui::Context) {
    let mut fonts = crate::fonts::definitions();
    fastframe_text::detect().apply_to(&mut fonts);
    ctx.set_fonts(fonts);
    egui_extras::install_image_loaders(ctx);
    fastframe_icons::install::<crate::icons::Icon>(ctx);
    fastframe_titlebar::install(ctx);
}

/// Applies the palette to every egui widget so dialogs, menus and inputs
/// agree with the custom views.
///
/// Both theme slots are styled, not only the active one: egui keeps a dark
/// and a light [`egui::Style`] and swaps them instantly when the OS
/// appearance changes (or the user switches mode). A slot left at egui's
/// default style would show for a frame as stock grey widgets before the
/// next palette resolution caught up — the "theme detection" flash.
pub fn apply(ctx: &egui::Context, palette: &Palette, reduced_motion: bool) {
    let active_theme = if palette.dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    let mut active_style = (*ctx.style_of(active_theme)).clone();
    apply_to_style(&mut active_style, palette, reduced_motion);
    ctx.set_style_of(active_theme, active_style);

    let other_theme = if palette.dark {
        egui::Theme::Light
    } else {
        egui::Theme::Dark
    };
    let other_palette = resolve(!palette.dark);
    let mut other_style = (*ctx.style_of(other_theme)).clone();
    apply_to_style(&mut other_style, &other_palette, reduced_motion);
    ctx.set_style_of(other_theme, other_style);
}

fn apply_to_style(style: &mut Style, palette: &Palette, reduced_motion: bool) {
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
    // Text selection: the master parity spec fixes the dark token at
    // rgba(250,250,250,0.2); the light value is its neutral inverse
    // (SOURCED-FROM: same rule on the ink token, the web's copper selection
    // conflicts with the monochrome brand directive).
    visuals.selection.bg_fill = if palette.dark {
        Color32::from_rgba_unmultiplied(250, 250, 250, 51)
    } else {
        Color32::from_rgba_unmultiplied(15, 23, 42, 31)
    };
    visuals.selection.stroke = Stroke::NONE;
    visuals.window_stroke = Stroke::new(1.0, palette.outline);
    // One elevation token across windows, popups and menus: the web's
    // `0 1px 2px rgba(15,23,42,0.04)`, invisible in dark mode where
    // `--ac-shadow: none`.
    visuals.window_corner_radius = CornerRadius::same(RADIUS_LARGE);
    visuals.menu_corner_radius = CornerRadius::same(RADIUS_CONTROL);
    visuals.window_shadow = elevation_shadow(palette);
    visuals.popup_shadow = elevation_shadow(palette);
    visuals.striped = false;
    // A 2 px accent caret reads better than the default 1 px muted bar.
    visuals.text_cursor.stroke = Stroke::new(2.0, palette.accent);
    visuals.slider_trailing_fill = true;
    visuals.handle_shape = egui::style::HandleShape::Circle;

    // Buttons and inputs share the 12 px control radius.
    let corner = CornerRadius::same(RADIUS_CONTROL);
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

    // The web/Android type scale: 12 px labels, 14 px body and buttons in
    // SemiBold, 19 px Bold page titles, 13 px mono. Named DM Sans weights
    // and Spline Sans Mono come from the brand font registration.
    use egui::TextStyle;
    style.text_styles = [
        (TextStyle::Small, crate::fonts::regular(12.0)),
        (TextStyle::Body, crate::fonts::regular(14.0)),
        (TextStyle::Button, crate::fonts::semibold(14.0)),
        (TextStyle::Heading, crate::fonts::bold(19.0)),
        (TextStyle::Monospace, crate::fonts::mono(13.0)),
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

    // Built-in widget transitions (hover fades, menu reveals) use the same
    // FAST token as the custom animations; zero when motion is reduced.
    style.animation_time = crate::motion::duration(crate::motion::FAST, reduced_motion);
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
        apply_to_style(&mut style, &Palette::dark(), false);
        assert!(!style.interaction.selectable_labels);
        apply_to_style(&mut style, &Palette::light(), false);
        assert!(!style.interaction.selectable_labels);
    }

    #[test]
    fn parity_tokens_match_the_web_and_android_sources() {
        // Master parity spec §2/§7: exact hexes from web theme.css and
        // Android Theme.kt; guards against silent token drift.
        let dark = Palette::dark();
        let light = Palette::light();
        assert_eq!(dark.window, hex(0x11, 0x11, 0x13));
        assert_eq!(dark.surface, hex(0x1c, 0x1c, 0x1f));
        assert_eq!(dark.outline, hex(0x2a, 0x2a, 0x2e));
        assert_eq!(dark.text, hex(0xf4, 0xf4, 0xf5));
        assert_eq!(dark.secondary, hex(0xa1, 0xa1, 0xaa));
        // Dark mode has no shadow at all (--ac-shadow: none).
        assert_eq!(dark.shadow, Color32::TRANSPARENT);
        assert_eq!(light.window, hex(0xf4, 0xf5, 0xf7));
        assert_eq!(light.surface, hex(0xff, 0xff, 0xff));
        assert_eq!(light.text, hex(0x0f, 0x17, 0x2a));
        assert_eq!(light.secondary, hex(0x64, 0x74, 0x8b));
        assert_eq!(light.outline, hex(0xe8, 0xea, 0xef));
        assert_eq!(
            light.shadow,
            Color32::from_rgba_unmultiplied(15, 23, 42, 10)
        );
        // Financial semantics (Theme.kt), light then dark.
        assert_eq!(light.finance_positive, hex(0x0f, 0x7b, 0x4f));
        assert_eq!(light.finance_positive_soft, hex(0xe6, 0xf6, 0xec));
        assert_eq!(light.finance_danger, hex(0xb9, 0x1c, 0x1c));
        assert_eq!(dark.finance_positive, hex(0x34, 0xd3, 0x99));
        assert_eq!(dark.finance_positive_soft, hex(0x10, 0x36, 0x2a));
        assert_eq!(dark.finance_info, hex(0x93, 0xc5, 0xfd));
        assert_eq!(dark.finance_info_soft, hex(0x16, 0x23, 0x3f));
    }

    #[test]
    fn apply_styles_both_theme_slots_with_their_own_palette() {
        // A theme flip swaps egui's dark/light style slots instantly: both
        // must carry the Atlas style, or stock egui widgets would show for
        // a frame when the OS appearance changes.
        let ctx = egui::Context::default();
        apply(&ctx, &Palette::dark(), false);

        let dark = ctx.style_of(egui::Theme::Dark);
        let light = ctx.style_of(egui::Theme::Light);
        assert!(!dark.interaction.selectable_labels);
        assert!(!light.interaction.selectable_labels);
        assert_eq!(dark.visuals.panel_fill, Palette::dark().window);
        assert_eq!(light.visuals.panel_fill, Palette::light().window);
        assert_ne!(dark.visuals.panel_fill, light.visuals.panel_fill);
    }
}
