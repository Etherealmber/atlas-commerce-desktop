//! Folders for settings, state and caches.
//!
//! Wraps [`fastframe_dirs`] with the portable-install awareness the
//! foundation provides on Windows: an executable unpacked next to an
//! `atlas-commerce-desktop-portable.txt` marker keeps its data in
//! `atlas-commerce-desktop-data` beside it, so a shop can carry the whole
//! app on a USB key. Installed copies use the platform per-user folders.

use std::fs;
use std::path::PathBuf;

use fastframe_dirs::{App, Discovery, Mode};

/// The slug every file and marker derives from.
pub const SLUG: &str = "atlas-commerce-desktop";
/// Reverse-domain identifier used for the single-instance slot and autostart.
pub const APP_ID: &str = "com.etherealmber.AtlasCommerceDesktop";
/// The human name shown by the desktop, the tray and notifications.
pub const APP_NAME: &str = "Atlas Commerce Desktop";

/// The fastframe folder description, shared by the whole process.
pub const DIRS: App = App {
    // The instance lock and past run logs are not copied into a fresh
    // portable folder.
    not_copied: &["instance", "logs"],
    ..App::new(SLUG, "com", "etherealmber", "AtlasCommerceDesktop")
};

/// Concrete paths inside the discovered folders. `Mode` is not `Clone` in
/// fastframe, and the app owns one of these for its whole lifetime.
#[derive(Debug)]
pub struct AppPaths {
    pub dirs: fastframe_dirs::Dirs,
    pub mode: Mode,
}

impl AppPaths {
    /// Discovers the folders this copy uses. Call once the process is the
    /// running (single) instance, before the window opens.
    #[must_use]
    pub fn discover() -> Self {
        let Discovery { dirs, mode } = DIRS.discover(|_, _| Ok(()));
        let dirs = dirs.unwrap_or_else(|| {
            // No home folder: fall back to a folder beside the executable.
            let root = std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
                .unwrap_or_else(|| PathBuf::from("."));
            fastframe_dirs::Dirs::inside(&root.join(format!("{SLUG}-data")))
        });
        Self { dirs, mode }
    }

    /// Ensures every folder exists.
    pub fn ensure(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.dirs.config)?;
        fs::create_dir_all(&self.dirs.state)?;
        fs::create_dir_all(self.logs_dir())?;
        fs::create_dir_all(self.themes_dir())?;
        fs::create_dir_all(&self.dirs.cache)?;
        Ok(())
    }

    // ── config/ : user-edited configuration and saved servers ────────────

    #[must_use]
    pub fn settings_file(&self) -> PathBuf {
        self.dirs.config.join("settings.json")
    }

    #[must_use]
    pub fn servers_file(&self) -> PathBuf {
        self.dirs.config.join("servers.json")
    }

    #[must_use]
    pub fn themes_dir(&self) -> PathBuf {
        self.dirs.config.join("themes")
    }

    // ── state/ : sessions, identity and logs (never synced) ──────────────

    #[must_use]
    pub fn device_file(&self) -> PathBuf {
        self.dirs.state.join("device.json")
    }

    #[must_use]
    pub fn session_file(&self) -> PathBuf {
        self.dirs.state.join("session.json")
    }

    #[must_use]
    pub fn accounts_file(&self) -> PathBuf {
        self.dirs.state.join("accounts.json")
    }

    #[must_use]
    pub fn agent_thread_file(&self) -> PathBuf {
        self.dirs.state.join("agent-thread.json")
    }

    #[must_use]
    pub fn logs_dir(&self) -> PathBuf {
        self.dirs.state.join("logs")
    }

    #[must_use]
    pub fn log_file(&self) -> PathBuf {
        self.logs_dir().join("atlas-commerce-desktop.log")
    }

    #[must_use]
    pub fn panic_log(&self) -> PathBuf {
        self.logs_dir().join("panic.log")
    }
}
