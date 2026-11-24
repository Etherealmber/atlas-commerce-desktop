//! Central desktop palette and widget tokens. The default light palette uses
//! Bento slate surfaces; dark and custom palettes remain available.

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
    /// Copper is a decorative accent, not the primary action color.
    pub copper: Color32,
    pub copper_soft: Color32,
    pub copper_border: Color32,
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
    /// Default dark slate surfaces, with high-contrast ink and neutral actions.
    #[must_use]
    pub fn dark() -> Self {
        Self {
            dark: true,
            window: hex(0x12, 0x12, 0x15),
            panel: hex(0x18, 0x18, 0x1b),
            surface: hex(0x18, 0x18, 0x1b),
            surface_hover: hex(0x29, 0x29, 0x2f),
            surface_active: hex(0x29, 0x29, 0x2f),
            outline: Color32::from_rgba_unmultiplied(255, 255, 255, 22),
            text: hex(0xf4, 0xf4, 0xf6),
            secondary: hex(0xa1, 0xa1, 0xaa),
            dim: hex(0x71, 0x71, 0x7a),
            accent: hex(0xfa, 0xfa, 0xfa),
            accent_hover: hex(0xe4, 0xe4, 0xe7),
            on_accent: hex(0x12, 0x12, 0x15),
            copper: hex(0xea, 0xb3, 0x7d),
            copper_soft: Color32::from_rgba_unmultiplied(212, 165, 116, 31),
            copper_border: Color32::from_rgba_unmultiplied(212, 165, 116, 66),
            danger: hex(0xf8, 0x71, 0x71),
            warning: hex(0xfb, 0xbf, 0x24),
            overlay: hex(0x18, 0x18, 0x1b),
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

    /// Light slate workspace with neutral primary actions and semantic status colours.
    #[must_use]
    pub fn light() -> Self {
        Self {
            dark: false,
            window: hex(0xf8, 0xfa, 0xfc),
            panel: hex(0xff, 0xff, 0xff),
            surface: hex(0xff, 0xff, 0xff),
            surface_hover: hex(0xf1, 0xf5, 0xf9),
            surface_active: hex(0xe2, 0xe8, 0xf0),
            outline: hex(0xe2, 0xe8, 0xf0),
            text: hex(0x0f, 0x17, 0x2a),
            secondary: hex(0x64, 0x74, 0x8b),
            dim: hex(0x94, 0xa3, 0xb8),
            accent: hex(0x0f, 0x17, 0x2a),
            accent_hover: hex(0x1e, 0x29, 0x3b),
            on_accent: hex(0xff, 0xff, 0xff),
            copper: hex(0xb8, 0x73, 0x33),
            copper_soft: Color32::from_rgba_unmultiplied(184, 115, 51, 26),
            copper_border: Color32::from_rgba_unmultiplied(184, 115, 51, 56),
            danger: hex(0xa4, 0x26, 0x2c),
            warning: hex(0x8a, 0x53, 0x00),
            overlay: hex(0xff, 0xff, 0xff),
            // SOURCED-FROM: Theme.kt badge saffron, counters only.
            badge: hex(0xf5, 0x9e, 0x0b),
            on_badge: hex(0x0a, 0x0a, 0x0a),
            // Web light elevation: 0 1px 2px rgba(15, 23, 42, 0.04).
            shadow: Color32::from_rgba_unmultiplied(15, 23, 42, 10),
            // Restrained semantic colours for the light workspace.
            finance_positive: hex(0x0e, 0x70, 0x0e),
            finance_positive_soft: hex(0xdf, 0xf6, 0xdd),
            finance_warning: hex(0x8a, 0x53, 0x00),
            finance_warning_soft: hex(0xff, 0xf4, 0xce),
            finance_danger: hex(0xa4, 0x26, 0x2c),
            finance_danger_soft: hex(0xfd, 0xe7, 0xe9),
            finance_info: hex(0x0f, 0x6c, 0xbd),
            finance_info_soft: hex(0xeb, 0xf3, 0xfc),
        }
    }
}

const fn hex(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Desktop radius scale for controls, cards and larger panels.
pub const RADIUS_CONTROL: u8 = 10;
pub const RADIUS_CARD: u8 = 16;
pub const RADIUS_LARGE: u8 = 16;
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
            "copper" => &mut self.copper,
            "copper_soft" => &mut self.copper_soft,
            "copper_border" => &mut self.copper_border,
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

/// WCAG relative luminance for an opaque sRGB colour.
fn luminance(color: Color32) -> f64 {
    fn linear(component: u8) -> f64 {
        let value = f64::from(component) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
}

fn contrast(a: Color32, b: Color32) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Choose the stronger black or white foreground for a filled control.
#[must_use]
pub(crate) fn contrasting_text(background: Color32) -> Color32 {
    if contrast(background, Color32::BLACK) >= contrast(background, Color32::WHITE) {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// Keep a hover fill only when the existing label remains readable.
#[must_use]
pub(crate) fn readable_hover(base: Color32, hover: Color32, foreground: Color32) -> Color32 {
    if contrast(hover, foreground) >= 4.5 {
        hover
    } else {
        base
    }
}

/// Adapt a system accent to the active surfaces and keep button text legible.
/// Windows can return very pale or very dark colours, so use the nearest
/// incremental shade that meets the text contrast target on both surfaces.
#[must_use]
pub fn with_system_accent(mut palette: Palette, source: Color32) -> Palette {
    let target = if palette.dark {
        Color32::WHITE
    } else {
        Color32::BLACK
    };
    let mut chosen = source;
    for step in 0..=100 {
        let mix = step as f32 / 100.0;
        let c = source.lerp_to_gamma(target, mix);
        if contrast(c, palette.surface) >= 4.5 && contrast(c, palette.window) >= 4.5 {
            chosen = c;
            break;
        }
    }
    palette.accent = chosen;
    palette.accent_hover = chosen.lerp_to_gamma(target, 0.12);
    palette.on_accent = if contrast(chosen, Color32::BLACK) >= contrast(chosen, Color32::WHITE) {
        Color32::BLACK
    } else {
        Color32::WHITE
    };
    palette
}

/// Observe the Windows accent away from the render thread. Failure, high
/// contrast, or an unavailable WinRT service leaves the solid base palette.
#[cfg(target_os = "windows")]
#[must_use]
pub fn windows_accent() -> Option<Color32> {
    let value = windows_appearance().0;
    (value & 0x0100_0000 != 0)
        .then(|| Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8))
}

/// Whether the system permits visual effects; unknown policy fails closed.
#[cfg(target_os = "windows")]
#[must_use]
pub fn windows_material_allowed() -> bool {
    windows_appearance().1
}

#[cfg(target_os = "windows")]
fn windows_appearance() -> (u32, bool) {
    use std::sync::{
        OnceLock,
        atomic::{AtomicU32, Ordering},
    };
    static ACCENT: AtomicU32 = AtomicU32::new(0);
    static MATERIAL: AtomicU32 = AtomicU32::new(0);
    static START: OnceLock<()> = OnceLock::new();
    START.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("atlas-windows-accent".into())
            .spawn(|| {
                use windows::UI::ViewManagement::{AccessibilitySettings, UIColorType, UISettings};
                loop {
                    let value = (|| {
                        if AccessibilitySettings::new().ok()?.HighContrast().ok()? {
                            return None;
                        }
                        let color = UISettings::new()
                            .ok()?
                            .GetColorValue(UIColorType::Accent)
                            .ok()?;
                        Some(
                            0x0100_0000
                                | (u32::from(color.R) << 16)
                                | (u32::from(color.G) << 8)
                                | u32::from(color.B),
                        )
                    })()
                    .unwrap_or(0);
                    ACCENT.store(value, Ordering::Relaxed);
                    let effects = (|| {
                        if AccessibilitySettings::new().ok()?.HighContrast().ok()? {
                            return None;
                        }
                        UISettings::new().ok()?.AdvancedEffectsEnabled().ok()
                    })()
                    .unwrap_or(false);
                    MATERIAL.store(u32::from(effects), Ordering::Relaxed);
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            });
    });
    (
        ACCENT.load(Ordering::Relaxed),
        MATERIAL.load(Ordering::Relaxed) != 0,
    )
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
    visuals.extreme_bg_color = if palette.dark {
        hex(0x20, 0x20, 0x24)
    } else {
        palette.surface
    };
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
    // Crisp outlined windows and popups: no blurred halo in either theme.
    visuals.window_corner_radius = CornerRadius::same(RADIUS_LARGE);
    visuals.menu_corner_radius = CornerRadius::same(RADIUS_CONTROL);
    visuals.window_shadow = egui::Shadow::NONE;
    visuals.popup_shadow = egui::Shadow::NONE;
    visuals.striped = false;
    // A 2 px accent caret reads better than the default 1 px muted bar.
    visuals.text_cursor.stroke = Stroke::new(2.0, palette.accent);
    visuals.slider_trailing_fill = true;
    visuals.handle_shape = egui::style::HandleShape::Circle;

    // Buttons and inputs share the desktop control radius.
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
    // their trigger; the popup border separates them.
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
    fn default_tokens_match_the_slate_palette_in_both_modes() {
        // Keep the selected light surfaces opaque and high contrast.
        let dark = Palette::dark();
        let light = Palette::light();
        assert_eq!(dark.window, hex(0x12, 0x12, 0x15));
        assert_eq!(dark.panel, hex(0x18, 0x18, 0x1b));
        assert_eq!(dark.surface, hex(0x18, 0x18, 0x1b));
        assert_eq!(dark.overlay, hex(0x18, 0x18, 0x1b));
        assert_eq!(dark.surface_hover, hex(0x29, 0x29, 0x2f));
        assert_eq!(dark.surface_active, hex(0x29, 0x29, 0x2f));
        assert_eq!(dark.accent, hex(0xfa, 0xfa, 0xfa));
        assert_eq!(dark.accent_hover, hex(0xe4, 0xe4, 0xe7));
        assert_eq!(dark.on_accent, dark.window);
        assert_eq!(
            dark.outline,
            Color32::from_rgba_unmultiplied(255, 255, 255, 22)
        );
        assert_eq!(dark.copper, hex(0xea, 0xb3, 0x7d));
        assert_eq!(light.copper, hex(0xb8, 0x73, 0x33));
        assert_eq!(dark.text, hex(0xf4, 0xf4, 0xf6));
        assert_eq!(dark.secondary, hex(0xa1, 0xa1, 0xaa));
        // Dark mode has no shadow at all (--ac-shadow: none).
        assert_eq!(dark.shadow, Color32::TRANSPARENT);
        assert_eq!(light.window, hex(0xf8, 0xfa, 0xfc));
        assert_eq!(light.surface, hex(0xff, 0xff, 0xff));
        assert_eq!(light.panel, hex(0xff, 0xff, 0xff));
        assert_eq!(light.accent, hex(0x0f, 0x17, 0x2a));
        for color in [
            light.window,
            light.panel,
            light.surface,
            light.overlay,
            light.surface_active,
        ] {
            assert_eq!(color.a(), 255);
        }
        assert_eq!(light.text, hex(0x0f, 0x17, 0x2a));
        assert_eq!(light.secondary, hex(0x64, 0x74, 0x8b));
        assert_eq!(light.outline, hex(0xe2, 0xe8, 0xf0));
        assert_eq!(
            light.shadow,
            Color32::from_rgba_unmultiplied(15, 23, 42, 10)
        );
        // Light Fluent semantics and existing dark semantics.
        assert_eq!(light.finance_positive, hex(0x0e, 0x70, 0x0e));
        assert_eq!(light.finance_positive_soft, hex(0xdf, 0xf6, 0xdd));
        assert_eq!(light.finance_danger, hex(0xa4, 0x26, 0x2c));
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

#[cfg(test)]
mod system_accent_tests {
    use super::{Palette, contrast, with_system_accent};
    use egui::Color32;

    #[test]
    fn danger_and_hover_text_remain_legible_in_both_modes() {
        for palette in [Palette::light(), Palette::dark()] {
            let text = super::contrasting_text(palette.danger);
            assert!(contrast(text, palette.danger) >= 4.5);
            let hover =
                super::readable_hover(palette.accent, palette.accent_hover, palette.on_accent);
            assert!(contrast(hover, palette.on_accent) >= 4.5);
        }
    }

    #[test]
    fn system_colors_keep_text_contrast_in_both_modes() {
        for base in [Palette::light(), Palette::dark()] {
            for color in [
                Color32::WHITE,
                Color32::BLACK,
                Color32::from_rgb(255, 230, 40),
                Color32::from_rgb(0, 120, 212),
            ] {
                let palette = with_system_accent(base, color);
                assert!(contrast(palette.accent, palette.surface) >= 4.5);
                assert!(contrast(palette.accent, palette.window) >= 4.5);
                assert!(contrast(palette.on_accent, palette.accent) >= 4.5);
            }
        }
    }
}
