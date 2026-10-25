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

/// Draws the monochrome brand mark from its embedded coverage mask
/// (`assets/branding/atlas-mark.mask`: `u32` width, `u32` height, then one
/// coverage byte per pixel), area-averaged down to the requested size. The
/// tray framework repaints the silhouette white or black to match the panel
/// (see fastframe-tray's `IconColour`), so every pixel stays neutral
/// `#fafafa` with only its alpha carrying the shape.
#[must_use]
pub fn draw_icon(size: usize) -> Vec<u8> {
    let bytes = include_bytes!("../assets/branding/atlas-mark.mask");
    let mask_width = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let mask_height = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let mask = &bytes[8..];
    debug_assert_eq!(mask.len(), mask_width * mask_height);

    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        // Map each destination pixel onto its source rectangle, so a 256 px
        // mask stays anti-aliased at the tray's 32/36/64 px sizes.
        let src_y0 = y * mask_height / size;
        let src_y1 = ((y + 1) * mask_height / size).max(src_y0 + 1);
        for x in 0..size {
            let src_x0 = x * mask_width / size;
            let src_x1 = ((x + 1) * mask_width / size).max(src_x0 + 1);
            let mut total = 0u32;
            let mut count = 0u32;
            for src_y in src_y0..src_y1 {
                let row = src_y * mask_width;
                for src_x in src_x0..src_x1 {
                    total += u32::from(mask[row + src_x]);
                    count += 1;
                }
            }
            let index = (y * size + x) * 4;
            // Neutral white: IconColour::White keeps it, Black inverts it.
            rgba[index] = 0xfa;
            rgba[index + 1] = 0xfa;
            rgba[index + 2] = 0xfa;
            rgba[index + 3] = (total / count) as u8;
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    #[test]
    fn draw_icon_keeps_the_tile_silhouette_only() {
        let size = 64_usize;
        let rgba = super::draw_icon(size);
        assert_eq!(rgba.len(), size * size * 4);
        // Outside the rounded tile the mask is fully transparent.
        assert_eq!(rgba[3], 0);
        // The sunburst carries opaque, neutral-white pixels, and every red
        // channel is either the paper white or the zeroed transparent fill.
        assert!(rgba.chunks_exact(4).any(|pixel| pixel[3] > 200));
        assert!(rgba.chunks_exact(4).all(|pixel| pixel[0] == 0xfa || pixel[0] == 0));
    }
}
