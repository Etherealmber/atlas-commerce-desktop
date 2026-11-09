//! Supervises the optional WebKitGTK helper without linking GTK into Atlas.
//! The authorization URL is sent only through a pipe, never command arguments.
use atlas_core::webview_policy::{Frame, NavigationPolicy};
use std::{
    io::Write,
    process::{Child, ChildStdin, Command, Stdio},
};

pub(crate) struct LoginView {
    child: Option<Child>,
    input: Option<ChildStdin>,
}

impl LoginView {
    pub(crate) fn open(url: &url::Url, server: &str, code: &str) -> Result<Self, &'static str> {
        let policy = NavigationPolicy::new(server).ok_or("Serveur HTTPS invalide.")?;
        if !policy.allows(Frame::Main, url.as_str()) {
            return Err("Adresse d’autorisation invalide.");
        }
        let executable = std::env::current_exe()
            .map_err(|_| "Connexion intégrée indisponible. Utilisez le navigateur système.")?;
        let helper = executable.with_file_name("atlas-commerce-webview");
        let mut child = Command::new(helper)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Connexion intégrée indisponible. Utilisez le navigateur système.")?;
        let Some(mut input) = child.stdin.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Connexion intégrée indisponible. Utilisez le navigateur système.");
        };
        let payload = serde_json::json!({ "url": url.as_str(), "server": server, "code": code });
        if serde_json::to_writer(&mut input, &payload).is_err()
            || input.write_all(b"\n").is_err()
            || input.flush().is_err()
        {
            drop(input);
            let _ = child.kill();
            let _ = child.wait();
            return Err("Connexion intégrée indisponible. Utilisez le navigateur système.");
        }
        Ok(Self {
            child: Some(child),
            input: Some(input),
        })
    }
    pub(crate) fn pump(&mut self) -> Result<bool, ()> {
        match self.child.as_mut().ok_or(())?.try_wait().map_err(|_| ())? {
            None => Ok(true),
            Some(status) if status.success() => Ok(false),
            Some(_) => Err(()),
        }
    }
}
impl Drop for LoginView {
    fn drop(&mut self) {
        drop(self.input.take());
        if let Some(mut child) = self.child.take() {
            // Reap without blocking the UI while WebKit tears down its process.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}
