//! Windows embedded v1 authorization, hosted by the installed WebView2.
//! No injected script, token access, IPC bridge, or network interception.

use std::path::{Path, PathBuf};
use std::time::Duration;

use atlas_core::webview_policy::{Frame, NavigationPolicy};
use wry::{WebContext, WebView, WebViewBuilder, WebViewBuilderExtWindows};

const HEADER_POINTS: f64 = 168.0;

/// The native child and its isolated profile. Dropping it starts local cleanup.
pub(crate) struct LoginView {
    view: Option<WebView>,
    context: Option<WebContext>,
    profile: Option<tempfile::TempDir>,
}

fn bounds(width: u32, height: u32, scale: f64) -> wry::Rect {
    let header = ((HEADER_POINTS * scale).round() as u32).min(height);
    wry::Rect {
        position: wry::dpi::PhysicalPosition::new(0, header as i32).into(),
        size: wry::dpi::PhysicalSize::new(width, height.saturating_sub(header)).into(),
    }
}

fn delete_later(path: PathBuf) {
    std::thread::spawn(move || {
        // WebView2's child processes can release their files after the host.
        for _ in 0..60 {
            match std::fs::remove_dir_all(&path) {
                Ok(()) => return,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
                Err(_) => std::thread::sleep(Duration::from_millis(250)),
            }
        }
        log::warn!(
            "Nettoyage du profil WebView temporaire incomplet ; nouvel essai au prochain lancement de connexion."
        );
    });
}

fn clean_old_profiles(root: &Path) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            // Never recurse through a symlink or touch unrelated profile names.
            if entry.file_name().to_string_lossy().starts_with("login-")
                && entry
                    .file_type()
                    .is_ok_and(|kind| kind.is_dir() && !kind.is_symlink())
            {
                delete_later(entry.path());
            }
        }
    }
}

impl LoginView {
    pub(crate) fn open(
        parent: &impl wry::raw_window_handle::HasWindowHandle,
        size: (u32, u32, f64),
        url: &url::Url,
        server: &str,
        cache: &Path,
    ) -> Result<Self, &'static str> {
        let policy = NavigationPolicy::new(server).ok_or("Serveur HTTPS invalide.")?;
        if !policy.allows(Frame::Main, url.as_str()) {
            return Err("Adresse d’autorisation invalide.");
        }
        let root = cache.join("isolated-login-webviews");
        std::fs::create_dir_all(&root).map_err(|_| "Profil de connexion indisponible.")?;
        clean_old_profiles(&root);
        let profile = tempfile::Builder::new()
            .prefix("login-")
            .tempdir_in(&root)
            .map_err(|_| "Profil de connexion indisponible.")?;
        let mut context = WebContext::new(Some(profile.path().to_owned()));
        let view = WebViewBuilder::new_with_web_context(&mut context)
            .with_url(url.as_str())
            .with_incognito(true)
            .with_devtools(false)
            // Wry's defaults disable SmartScreen; preserve the runtime defaults.
            .with_additional_browser_args("")
            .with_navigation_handler(move |target| policy.allows(Frame::Main, &target))
            .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
            .with_download_started_handler(|_, _| false)
            .with_permission_handler(|_| wry::PermissionResponse::Deny)
            .with_bounds(bounds(size.0, size.1, size.2))
            .build_as_child(parent)
            .map_err(|_| "WebView2 indisponible. Utilisez le navigateur système.")?;
        Ok(Self {
            view: Some(view),
            context: Some(context),
            profile: Some(profile),
        })
    }

    pub(crate) fn resize(&self, width: u32, height: u32, scale: f64) {
        if let Some(view) = &self.view {
            let _ = view.set_bounds(bounds(width, height, scale));
        }
    }
}

impl Drop for LoginView {
    fn drop(&mut self) {
        drop(self.view.take());
        drop(self.context.take());
        if let Some(profile) = self.profile.take() {
            delete_later(profile.keep());
        }
    }
}
