//! System tray: keeps the app running with the window closed and offers
//! Show / Quick stats / Quit. French labels are rebuilt when the language
//! changes via [`TrayHandle::set_labels`].

use fastframe_tray::{Config, Event, MenuItem, Tray};

use crate::i18n::Locale;
use crate::t;

/// Commands the tray sends to the UI thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    Show,
    QuickStats,
    Quit,
}

/// Menu item ids; stable strings the tray keeps referencing.
const ID_SHOW: &str = "show";
const ID_STATS: &str = "stats";
const ID_QUIT: &str = "quit";

/// Owns the fastframe tray and translates its events into commands.
pub struct TrayHandle {
    tray: Tray,
}

impl TrayHandle {
    /// Spawns the tray icon. Returns `None` on platforms/sessions without a
    /// tray host, which the app treats as "close quits". The fastframe tray
    /// keeps its own event channel; [`Self::drain`] reads it on the UI
    /// thread, so the handle must live inside the app, not a worker thread.
    #[must_use]
    pub fn spawn(locale: Locale, wake: impl Fn() + Send + Sync + 'static) -> Option<Self> {
        let config = config_for(locale);
        Some(Self {
            tray: Tray::spawn(config, wake)?,
        })
    }

    /// Whether a panel or notification area currently shows the item.
    #[must_use]
    pub fn is_shown(&self) -> bool {
        self.tray.is_shown()
    }

    /// Creates the macOS menu-bar item while no window exists yet (a hidden
    /// start at login). No-op on the other platforms.
    pub fn create_item(&mut self) {
        self.tray.create_item();
    }

    /// (Re)attaches the macOS status item once a window exists. No-op
    /// elsewhere.
    pub fn attach(&mut self) {
        self.tray.attach();
    }

    /// Drains events that arrived since the last frame.
    pub fn drain(&mut self) -> Vec<TrayCommand> {
        self.tray
            .events()
            .into_iter()
            .filter_map(|event| match event {
                Event::Show | Event::Toggle => Some(TrayCommand::Show),
                Event::Menu(ID_SHOW) => Some(TrayCommand::Show),
                Event::Menu(ID_STATS) => Some(TrayCommand::QuickStats),
                Event::Menu(ID_QUIT) => Some(TrayCommand::Quit),
                Event::Menu(_) => None,
            })
            .collect()
    }

    /// Refreshes the menu labels after a language change.
    pub fn set_labels(&mut self, locale: Locale) {
        self.tray
            .set_label(ID_SHOW, t!(locale, "Show window").into_owned());
        self.tray
            .set_label(ID_STATS, t!(locale, "Quick stats").into_owned());
        self.tray
            .set_label(ID_QUIT, t!(locale, "Quit").into_owned());
        self.tray
            .set_tooltip(t!(locale, "Atlas Commerce Desktop").into_owned());
    }

    /// Enables or disables the quick-stats entry (no session = no stats).
    pub fn set_stats_enabled(&mut self, enabled: bool) {
        self.tray.set_enabled(ID_STATS, enabled);
    }
}

fn config_for(locale: Locale) -> Config {
    Config {
        id: "atlas-commerce-desktop",
        title: t!(locale, "Atlas Commerce Desktop").into_owned(),
        icon: draw_icon,
        template_icon: Some(draw_icon),
        themed_icon: true,
        menu: vec![
            MenuItem::action(ID_SHOW, t!(locale, "Show window").into_owned()),
            MenuItem::action(ID_STATS, t!(locale, "Quick stats").into_owned()),
            MenuItem::Separator,
            MenuItem::action(ID_QUIT, t!(locale, "Quit").into_owned()),
        ],
    }
}

/// The window and taskbar icon: the same monochrome brand mark the tray
/// uses, as a 256 px RGBA bitmap. Windows scales it for the title bar,
/// taskbar and Alt+Tab; the embedded `.ico` resource (build.rs) is what
/// Explorer shows for the file itself.
#[must_use]
pub fn window_icon() -> egui::IconData {
    const SIZE: usize = 256;
    egui::IconData {
        rgba: draw_icon(SIZE),
        width: SIZE as u32,
        height: SIZE as u32,
    }
}

/// Draws the monochrome brand mark: a rounded tile with the "A" carved out
/// of its alpha channel. The tray framework repaints the shape white or
/// black to match the panel (see fastframe-tray's `IconColour`), so only the
/// silhouette matters — no brand colour lives in this icon.
#[must_use]
pub fn draw_icon(size: usize) -> Vec<u8> {
    let mut rgba = vec![0u8; size * size * 4];
    let scale = size as f32 / 256.0;
    let half = 128.0 * scale;
    let tile_radius = 40.0 * scale;
    let stroke_half = 8.0 * scale;
    // The A's three strokes, in 256-unit coordinates: two legs and the bar.
    let strokes: [((f32, f32), (f32, f32)); 3] = [
        ((86.0, 176.0), (128.0, 76.0)),
        ((128.0, 76.0), (170.0, 176.0)),
        ((100.0, 146.0), (156.0, 146.0)),
    ];
    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let dx = (px - half).abs();
            let dy = (py - half).abs();
            // Inside a rounded rectangle: inset the corner radius.
            let corner = (dx - (half - tile_radius)).max(0.0);
            let inside_tile = dx <= half
                && dy <= half
                && corner * corner + (dy - (half - tile_radius)).max(0.0).powi(2)
                    <= tile_radius * tile_radius;
            if !inside_tile {
                continue;
            }
            // Carve the A where the pixel sits on one of the strokes.
            let carved = strokes.iter().any(|(start, end)| {
                let start = (start.0 * scale, start.1 * scale);
                let end = (end.0 * scale, end.1 * scale);
                distance_to_segment(px, py, start, end) <= stroke_half
            });
            if !carved {
                let index = (y * size + x) * 4;
                // Neutral white: IconColour::White keeps it, Black inverts it.
                rgba[index] = 0xfa;
                rgba[index + 1] = 0xfa;
                rgba[index + 2] = 0xfa;
                rgba[index + 3] = 255;
            }
        }
    }
    rgba
}

/// Shortest distance from point (`px`, `py`) to the segment joining `start`
/// and `end`.
fn distance_to_segment(
    px: f32,
    py: f32,
    start: (f32, f32),
    end: (f32, f32),
) -> f32 {
    let vx = end.0 - start.0;
    let vy = end.1 - start.1;
    let length_sq = vx * vx + vy * vy;
    let t = if length_sq == 0.0 {
        0.0
    } else {
        (((px - start.0) * vx + (py - start.1) * vy) / length_sq).clamp(0.0, 1.0)
    };
    let closest_x = start.0 + t * vx;
    let closest_y = start.1 + t * vy;
    ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt()
}
