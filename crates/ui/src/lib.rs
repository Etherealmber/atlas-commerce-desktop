//! Atlas Commerce Desktop — the egui + fastframe interface crate.
//!
//! The binary in `crates/app` owns process wiring (single instance, deep
//! links, the update intercept); this crate owns every pixel and all of the
//! authenticated application state.

#![forbid(unsafe_code)]

pub mod app;
pub mod branding;
mod browser_auth;
mod business_search;
pub mod buttons;
mod client_invoices;
mod client_report;
pub mod dialogs;
pub mod employees;
pub mod fonts;
pub mod i18n;
pub mod icons;
pub mod motion;
mod native_auth_v2;
mod product_images;
pub mod screens;
pub mod server_probe;
pub mod shortcuts;
pub mod state;
pub mod theme;
pub mod tray;
pub mod update;
#[cfg(target_os = "windows")]
mod webview_auth;
#[cfg(target_os = "linux")]
mod webview_auth_linux;
pub mod widgets;

pub use app::EframeShell;
pub use state::AtlasApp;

/// Queue marker handed over by a second launch with no deep link: the running
/// instance must leave the tray and show its window. Travels through the same
/// queue as `open-link:` handoffs, so it needs no extra shared state.
pub const SHOW_REQUEST: &str = "atlas-desktop:show";
