//! Atlas Commerce Desktop — the egui + fastframe interface crate.
//!
//! The binary in `crates/app` owns process wiring (single instance, deep
//! links, the update intercept); this crate owns every pixel and all of the
//! authenticated application state.

#![forbid(unsafe_code)]

pub mod app;
pub mod dialogs;
pub mod i18n;
pub mod icons;
pub mod screens;
pub mod state;
pub mod theme;
pub mod tray;
pub mod update;
pub mod widgets;

pub use app::EframeShell;
pub use state::AtlasApp;

use theme::Palette;

/// The brand mark: a rounded amber square carrying a dark "A". Drawn with
/// the painter so no image asset is needed before fonts are ready.
pub fn brand(ui: &mut egui::Ui, palette: &Palette, size: f32) {
    let (rect, _) =
        ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, size * 0.24, palette.accent);
    let galley = ui.painter().layout_no_wrap(
        "A".to_owned(),
        egui::FontId::proportional(size * 0.62),
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
}
