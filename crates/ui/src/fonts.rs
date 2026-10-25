//! Brand typefaces, bundled under the SIL Open Font License (see
//! `assets/fonts/*-OFL.txt`):
//!
//! - **DM Sans** (variable, `wght` axis) is the web and Android interface
//!   face, so the desktop draws every proportional text with it, at the
//!   four weights the design system uses (400/500/600/700).
//! - **Spline Sans Mono** (variable, `wght` axis) is the mono face for
//!   amounts and quantities; every monospace advance has the same width,
//!   which is the desktop equivalent of the web's
//!   `font-variant-numeric: tabular-nums`.
//!
//! The fastframe Inter setup stays registered behind both faces as the
//! fallback chain, together with the platform fonts for scripts DM Sans
//! does not cover (CJK, emoji). Weight instances reuse the same variable
//! bytes with `wght` variation coordinates, exactly as fastframe-fonts
//! does for its own Inter setup.

use std::sync::Arc;

use egui::epaint::text::VariationCoords;
use egui::{FontData, FontDefinitions, FontFamily, FontId};

/// DM Sans variable font bytes (`wght` 100–1000, `opsz` axis).
pub const DM_SANS: &[u8] = include_bytes!("../assets/fonts/DMSans.ttf");
/// Spline Sans Mono variable font bytes (`wght` axis).
pub const SPLINE_SANS_MONO: &[u8] = include_bytes!("../assets/fonts/SplineSansMono.ttf");

/// Family keys; the regular weight also leads [`FontFamily::Proportional`].
pub const DM_REGULAR: &str = "dm-sans";
pub const DM_MEDIUM: &str = "dm-sans-medium";
pub const DM_SEMIBOLD: &str = "dm-sans-semibold";
pub const DM_BOLD: &str = "dm-sans-bold";
/// Spline Sans Mono at 400, also leading [`FontFamily::Monospace`].
pub const MONO_REGULAR: &str = "spline-sans-mono";
/// Spline Sans Mono at 600, for bold amounts and table numbers.
pub const MONO_SEMIBOLD: &str = "spline-sans-mono-semibold";

/// The four interface weights as `(family key, wght coordinate)`.
const DM_WEIGHTS: [(&str, f32); 4] = [
    (DM_REGULAR, 400.0),
    (DM_MEDIUM, 500.0),
    (DM_SEMIBOLD, 600.0),
    (DM_BOLD, 700.0),
];

fn variable_face(bytes: &'static [u8], weight: f32) -> FontData {
    let mut data = FontData::from_static(bytes);
    data.tweak.coords = VariationCoords::new([(b"wght", weight)]);
    data
}

/// Registers the brand faces on top of fastframe's default definitions,
/// keeping every existing fallback (Inter, egui faces, system fonts).
#[must_use]
pub fn definitions() -> FontDefinitions {
    let mut fonts = fastframe_fonts::FontSetup::default().definitions();

    for (name, weight) in DM_WEIGHTS {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(variable_face(DM_SANS, weight)));
    }
    fonts.font_data.insert(
        MONO_REGULAR.to_owned(),
        Arc::new(variable_face(SPLINE_SANS_MONO, 400.0)),
    );
    fonts.font_data.insert(
        MONO_SEMIBOLD.to_owned(),
        Arc::new(variable_face(SPLINE_SANS_MONO, 600.0)),
    );

    // DM Sans regular leads the proportional family; Inter and the system
    // fallbacks stay behind it for missing glyphs.
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, DM_REGULAR.to_owned());

    // The named weights each reuse the matching Inter family's fallback
    // chain instead of rebuilding one.
    for (fastframe_weight, dm_name) in [
        (fastframe_fonts::Weight::Medium, DM_MEDIUM),
        (fastframe_fonts::Weight::SemiBold, DM_SEMIBOLD),
        (fastframe_fonts::Weight::Bold, DM_BOLD),
    ] {
        let mut chain = vec![dm_name.to_owned()];
        if let Some(behind) = fonts.families.get(&fastframe_weight.family()) {
            chain.extend(behind.iter().cloned());
        }
        fonts
            .families
            .insert(FontFamily::Name(dm_name.into()), chain);
    }

    // Spline Sans Mono leads the monospace family; the named semibold keeps
    // the same fallback chain.
    let mono_chain = fonts.families.entry(FontFamily::Monospace).or_default();
    mono_chain.insert(0, MONO_REGULAR.to_owned());
    let mut semibold_chain = vec![MONO_SEMIBOLD.to_owned()];
    semibold_chain.extend(mono_chain.iter().cloned());
    fonts
        .families
        .insert(FontFamily::Name(MONO_SEMIBOLD.into()), semibold_chain);

    fonts
}

/// Regular (400) interface face.
#[must_use]
pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}

/// Medium (500) interface face, for labels and inactive navigation.
#[must_use]
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(DM_MEDIUM.into()))
}

/// SemiBold (600) interface face, for buttons, section titles and rows.
#[must_use]
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(DM_SEMIBOLD.into()))
}

/// Bold (700) interface face, for page titles and hero numbers.
#[must_use]
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(DM_BOLD.into()))
}

/// Spline Sans Mono regular, for amounts and quantities.
#[must_use]
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// Spline Sans Mono SemiBold, for emphasized table numbers and KPIs.
#[must_use]
pub fn mono_semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MONO_SEMIBOLD.into()))
}
