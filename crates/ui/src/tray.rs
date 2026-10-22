//! System tray: keeps the app running with the window closed and offers
//! Show / Quick stats / Quit. French labels are rebuilt when the language
//! changes via [`Tray::set_labels`].

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
        self.tray.set_label(ID_SHOW, t!(locale, "Show window").into_owned());
        self.tray
            .set_label(ID_STATS, t!(locale, "Quick stats").into_owned());
        self.tray
            .set_label(ID_QUIT, t!(locale, "Quit").into_owned());
        self.tray.set_tooltip(t!(locale, "Atlas Commerce Desktop").into_owned());
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

/// Draws the tray icon: a rounded amber square with a white "A" mark.
#[must_use]
pub fn draw_icon(size: usize) -> Vec<u8> {
    let mut rgba = vec![0u8; size * size * 4];
    let center = size as f32 / 2.0;
    let radius = size as f32 * 0.22;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let inside = (dx * dx + dy * dy).sqrt() <= radius;
            let index = (y * size + x) * 4;
            if inside {
                // Amber mark.
                rgba[index] = 0xf5;
                rgba[index + 1] = 0x9e;
                rgba[index + 2] = 0x0b;
                rgba[index + 3] = 255;
            }
        }
    }
    rgba
}
