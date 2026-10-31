//! Application lifecycle: the eframe wrapper, the fastframe shell `Resident`
//! implementation, session bootstrap, background polling and self-update.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use atlas_core::update_feed::UpdateDecision;
use atlas_core::{SessionRecord, UpdateManifest};
use egui::{Context, Ui};
use fastframe_shell::{Headless, Held, Lease, Resident};

use crate::screens;
use crate::state::{AtlasApp, Route, Screen};
use crate::t;
use crate::theme::Palette;
use crate::tray::TrayCommand;

/// Fall back to egui's raw input focus when the native viewport does not
/// report its focus state. Unknown minimized state must not mask focus loss.
fn should_lock(focused: bool, minimized: Option<bool>, closing: bool) -> bool {
    !focused || minimized == Some(true) || closing
}

impl AtlasApp {
    /// Opens a session: persist it, remember the account, rebuild the
    /// authenticated client and load the first shell data.
    pub fn enter_session(
        &mut self,
        token: String,
        username: String,
        role: String,
        display_name: String,
        avatar_url: String,
    ) {
        // Fail closed if the next launch cannot be made to require a fresh
        // password. This marker contains no credentials or session data.
        if let Err(error) = std::fs::write(self.paths.auth_guard_file(), b"1") {
            log::warn!("could not persist authentication guard: {error}");
            self.login.error =
                Some(t!(self.locale, "Could not secure the session on this device.").into_owned());
            self.login.password.clear();
            return;
        }
        self.requires_password = false;
        self.login.password.clear();
        let server_url = self
            .server_store
            .selected_url()
            .unwrap_or_default()
            .to_owned();
        let record = SessionRecord {
            server_url,
            username: username.to_lowercase(),
            role: if role.is_empty() {
                "user".to_owned()
            } else {
                role
            },
            display_name: if display_name.is_empty() {
                username
            } else {
                display_name
            },
            email: String::new(),
            avatar_url,
            token,
        };
        // A password login and a QR approval can race. Once either succeeds,
        // discard all other pending authentication jobs and their secrets so
        // a late completion cannot replace the active account.
        self.login = Default::default();
        self.qr = Default::default();
        self.session = Some(record.clone());
        self.account_store.upsert(&record);
        self.save_session();
        self.save_accounts();
        self.rebuild_client();
        self.screen = Screen::Shell(Route::Dashboard);
        self.bootstrap_shell();
    }

    /// Drop the active session and all session-scoped caches on focus loss.
    /// Re-authentication uses the server password flow, not the saved token.
    fn lock_for_inactivity(&mut self) {
        if self.session.is_some() {
            self.sign_out();
            self.requires_password = true;
        }
    }

    /// First loads after entering the shell or restoring a session.
    ///
    /// On-demand only: the landing dashboard is the single automatic call,
    /// so startup does not fan out one request per screen. Every other cache
    /// is fetched the first time its route opens (`ensure_route_data`) or
    /// when a chrome surface needs it (bell badge, profile popover). The
    /// update check stays, gated by its own setting.
    pub fn bootstrap_shell(&mut self) {
        self.load_dashboard();
        // Native stock toasts need alert data even if the user never opens
        // the Notifications screen; only spend the request when the
        // persisted setting is enabled.
        if self.settings.notifications_enabled {
            self.load_alerts();
        }
        if self.settings.update_check_on_start {
            self.check_for_updates(false);
        }
    }

    // ── Data loaders ─────────────────────────────────────────────────────

    pub fn load_dashboard(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.dashboard().await });
        self.dashboard.start(job);
    }

    pub fn load_products(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let query = self.product_search.clone();
        let job = self
            .runner
            .spawn(async move { client.products(&query, 300).await });
        self.products.start(job);
    }

    pub fn load_clients(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.clients().await });
        self.clients.start(job);
    }

    pub fn load_vendors(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.vendors().await });
        self.vendors.start(job);
    }

    pub fn load_expenses(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.expenses().await });
        self.expenses.start(job);
    }

    pub fn load_delivery_notes(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self
            .runner
            .spawn(async move { client.delivery_notes().await });
        self.delivery_notes.start(job);
    }

    pub fn load_categories(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.categories().await });
        self.categories.start(job);
    }

    pub fn load_units(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.units().await });
        self.units.start(job);
    }

    pub fn load_invoices(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self
            .runner
            .spawn(async move { client.invoices("all").await });
        self.invoices.start(job);
    }

    pub fn load_alerts(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.alerts().await });
        self.alerts.start(job);
    }

    pub fn load_nav_badges(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.nav_badges().await });
        self.nav_badges.start(job);
    }

    /// Lazy fetch of the title-bar badge count. The first call starts a
    /// request immediately; after a failure (`data` still `None`) retries are
    /// rate-limited by [`crate::state::NAV_BADGES_RETRY_BACKOFF`] so a
    /// persistently failing endpoint cannot trigger one request per frame.
    pub fn ensure_nav_badges(&mut self) {
        let backoff_open = self
            .nav_badges_retry_at
            .is_none_or(|at| Instant::now() >= at);
        if self.session.is_some()
            && self.nav_badges.data.is_none()
            && !self.nav_badges.loading
            && backoff_open
        {
            self.load_nav_badges();
            self.nav_badges_retry_at =
                Some(Instant::now() + crate::state::NAV_BADGES_RETRY_BACKOFF);
        }
    }

    pub fn load_devices(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.devices().await });
        self.devices.start(job);
    }

    // ── Profile / integrations ───────────────────────────────────────────

    pub fn load_profile(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.me().await });
        self.profile.start(job);
    }

    /// Pushes the form through `PATCH /api/auth/me`.
    pub fn save_profile(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let patch = atlas_api::models::MeUpdate {
            display_name: Some(self.profile_form.display_name.trim().to_owned()),
            email: Some(self.profile_form.email.trim().to_owned()),
            phone: Some(self.profile_form.phone.trim().to_owned()),
            notes: Some(self.profile_form.notes.trim().to_owned()),
            avatar_url: Some(self.profile_form.avatar_url.trim().to_owned()),
        };
        let job = self
            .runner
            .spawn(async move { client.update_me(&patch).await });
        self.profile_save_job = Some(job);
    }

    pub fn load_api_keys(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self
            .runner
            .spawn(async move { client.list_api_keys().await });
        self.api_keys.start(job);
    }

    /// Creates a key from the open editor; the raw secret comes back once and
    /// is kept for the reveal modal.
    pub fn create_api_key(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let Some(editor) = self.key_editor.clone() else {
            return;
        };
        let Some(scopes) = editor.scopes else {
            return;
        };
        let name = editor.name.trim().to_owned();
        if name.is_empty() {
            return;
        }
        let job = self
            .runner
            .spawn(async move { client.create_api_key(&name, scopes.as_str()).await });
        self.key_create_job = Some(job);
    }

    /// Open the web app's filtered invoice list for one client in the system
    /// browser. The browser keeps its own web session (same route the web
    /// client navigates to from its Clients table).
    pub fn open_client_invoices(&mut self, client: &atlas_api::models::Client) {
        let Some(api) = self.client.clone() else {
            return;
        };
        // Only the stable client ID goes in the query: a `q` name search
        // would hide older invoices after the client is renamed (the web
        // page treats `q` as an additional text filter, not part of the key).
        let url = match url::Url::parse_with_params(
            &format!("{}/invoices", api.base_url()),
            &[("client", client.id.to_string())],
        ) {
            Ok(url) => url,
            Err(error) => {
                log::warn!("invalid invoices link: {error}");
                self.notice(t!(self.locale, "Invalid report link.").into_owned());
                return;
            }
        };
        if let Err(error) = open::that(url.as_str()) {
            log::warn!("open browser invoices: {error}");
            self.notice(t!(self.locale, "The system could not open the browser.").into_owned());
        }
    }

    /// Mint a 30-minute report ticket and open the printable client
    /// situation report (`Synthèse` or `Détail complet`) in the browser.
    pub fn open_client_situation(&mut self, client: &atlas_api::models::Client, full: bool) {
        let Some(api) = self.client.clone() else {
            return;
        };
        if self.report_job.is_some() {
            return;
        }
        let client_id = client.id;
        let name = client.name.clone();
        let ice = client.ice.clone();
        let locale = self.locale;
        let job = self.runner.spawn(async move {
            let ticket = api.report_link_ticket().await?;
            let mut url =
                url::Url::parse(&format!("{}/api/reports/client-situation", api.base_url()))
                    .map_err(|error| {
                        log::warn!("invalid report link: {error}");
                        atlas_api::ApiError::client(t!(locale, "Invalid report link.").into_owned())
                    })?;
            url.query_pairs_mut()
                .append_pair("rt", &ticket.rt)
                .append_pair("client_id", &client_id.to_string())
                .append_pair("name", &name)
                .append_pair("ice", &ice);
            if full {
                url.query_pairs_mut().append_pair("detail", "full");
            }
            Ok(url.to_string())
        });
        self.report_job = Some(job);
    }

    pub fn revoke_api_key(&mut self, id: i64) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move {
            client.delete_api_key(id).await?;
            client.list_api_keys().await
        });
        self.api_keys.start(job);
        // The confirmation notice waits for the job to succeed (see
        // poll_background); a failed deletion stays on the cache error path.
        self.api_keys_revoke_pending = true;
    }

    pub fn load_assist(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self
            .runner
            .spawn(async move { client.assist_provider().await });
        self.assist.start(job);
    }

    /// Copies a freshly loaded profile into the editable form once.
    fn fill_profile_form(&mut self, me: &atlas_api::models::Me) {
        if self.profile_form.loaded {
            return;
        }
        self.profile_form = crate::state::ProfileForm {
            display_name: me.display_name.clone().unwrap_or_default(),
            email: me.email.clone().unwrap_or_default(),
            phone: me.phone.clone().unwrap_or_default(),
            notes: me.notes.clone().unwrap_or_default(),
            avatar_url: me.avatar_url.clone().unwrap_or_default(),
            loaded: true,
        };
    }

    fn poll_profile_jobs(&mut self) {
        if let Some(job) = &self.profile_save_job
            && let Some(result) = job.poll()
        {
            self.profile_save_job = None;
            match result {
                Ok(me) => {
                    // The PATCH response only carries the editable profile
                    // fields; preserve identity fields from the loaded Me
                    // instead of replacing them with serde defaults.
                    let merged = if let Some(existing) = self.profile.data.as_mut() {
                        existing.display_name = me.display_name.clone();
                        existing.email = me.email.clone();
                        existing.phone = me.phone.clone();
                        existing.notes = me.notes.clone();
                        existing.avatar_url = me.avatar_url.clone();
                        existing.clone()
                    } else {
                        me.clone()
                    };
                    self.profile.data = Some(merged.clone());
                    self.profile_form.loaded = true;
                    self.fill_profile_form(&merged);
                    if let Some(session) = self.session.as_mut() {
                        // Drop the previous picture from the texture cache
                        // when the avatar URL actually changes.
                        let new_url = merged.avatar_url.clone().unwrap_or_default();
                        if !session.avatar_url.is_empty() && session.avatar_url != new_url {
                            self.avatar_evict = Some(session.avatar_url.clone());
                        }
                        session.display_name = merged.display_name.clone().unwrap_or_default();
                        session.avatar_url = new_url;
                        session.email = merged.email.clone().unwrap_or_default();
                    }
                    self.save_session();
                    self.notice(t!(self.locale, "Profile updated.").into_owned());
                }
                Err(error) => self.notice(error.message),
            }
        }
        if let Some(job) = &self.key_create_job
            && let Some(result) = job.poll()
        {
            self.key_create_job = None;
            match result {
                Ok(created) => {
                    self.created_key = Some(created);
                    self.key_editor = None;
                    self.load_api_keys();
                }
                Err(error) => self.notice(error.message),
            }
        }
        if let Some(job) = &self.report_job
            && let Some(result) = job.poll()
        {
            self.report_job = None;
            match result {
                Ok(url) => {
                    if let Err(error) = open::that(url.as_str()) {
                        log::warn!("open browser report: {error}");
                        self.notice(
                            t!(self.locale, "The system could not open the browser.").into_owned(),
                        );
                    }
                }
                Err(error) => self.notice(error.message),
            }
        }
    }

    // ── Updates ──────────────────────────────────────────────────────────

    pub fn check_for_updates(&mut self, prerelease_override: bool) {
        if self.update.checking {
            return;
        }
        let prerelease = prerelease_override
            || matches!(
                self.settings.update_channel,
                atlas_core::settings::UpdateChannel::Prerelease
            );
        self.update.checking = true;
        self.update.error = None;
        let job = self
            .runner
            .spawn(async move { crate::update::fetch_manifest(prerelease).await });
        self.update.job = Some(job);
    }

    /// Downloads and hands off the update through the fastframe helper.
    pub fn start_update_download(&mut self) {
        if self.update.progress.is_some() {
            return;
        }
        let progress = Arc::new(Mutex::new((0_u64, 0_u64)));
        self.update_progress = progress.clone();
        self.update.progress = Some((0, 0));
        self.update.stage = t!(self.locale, "Downloading…").into_owned();
        let locale = self.locale;
        let job: atlas_core::Job<Result<(), String>> = self.runner.spawn(async move {
            // Pin the blocking closure's error type: without this, inference
            // starts from the &str guard below and resolves it to unsized str.
            tokio::task::spawn_blocking(move || -> Result<(), String> {
                let updater = crate::update::build_updater().map_err(|error| error.to_string())?;
                // Refuse early with a clear message when this copy has no
                // portable/installer marker (e.g. a dev build): the helper
                // cannot replace an executable it cannot identify.
                updater.installation().map_err(|_| {
                    t!(
                        locale,
                        "This build cannot update itself. Download the latest installer from the releases page."
                    )
                    .into_owned()
                })?;
                let release = updater
                    .check()
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| t!(locale, "No published release found.").into_owned())?;
                let prepared = updater
                    .download(&release, |received, total| {
                        if let Ok(mut guard) = progress.lock() {
                            *guard = (received, total);
                        }
                    })
                    .map_err(|error| error.to_string())?;
                updater
                    .handoff(prepared, Vec::new())
                    .map_err(|error| error.to_string())?;
                Ok(())
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|job_outcome: Result<(), String>| {
                // Classify after the blocking work unwinds: a 404 / missing
                // asset in the release's publish window is a "try again
                // shortly", not a permanent fault.
                match job_outcome {
                    Err(message)
                        if crate::update::is_release_not_ready(message.as_str()) =>
                    {
                        Err(t!(
                            locale,
                            "The new release is still being published on GitHub. Try again in a few minutes."
                        )
                        .into_owned())
                    }
                    other => other,
                }
            })
        });
        self.update_download_job = Some(job);
    }

    fn poll_update_jobs(&mut self, ctx: &Context) {
        if let Some(job) = &self.update.job
            && let Some(result) = job.poll()
        {
            self.update.job = None;
            self.update.checking = false;
            match result {
                Ok(Some(manifest)) => {
                    self.apply_manifest(manifest);
                }
                Ok(None) => {}
                Err(error) => self.update.error = Some(error),
            }
        }
        if let Some(job) = &self.update_download_job {
            if let Ok(guard) = self.update_progress.lock() {
                self.update.progress = Some(*guard);
            }
            if let Some(result) = job.poll() {
                self.update_download_job = None;
                match result {
                    Ok(()) => {
                        self.update.stage = t!(self.locale, "Installing…").into_owned();
                        self.wants_quit = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    Err(error) => {
                        self.update.progress = None;
                        self.update.error = Some(error);
                    }
                }
            }
        }
    }

    fn apply_manifest(&mut self, manifest: UpdateManifest) {
        let decision = manifest.decision_for(env!("CARGO_PKG_VERSION"));
        match decision {
            UpdateDecision::UpToDate => {
                // Surface an explicit status in the About card instead of
                // silently swallowing the answer.
                self.update.up_to_date_at = Some(Instant::now());
                self.update.error = None;
            }
            UpdateDecision::Mandatory { .. } => {
                self.update.mandatory = Some(manifest.clone());
                self.update.last_manifest = Some(manifest);
                self.screen = Screen::UpdateGate;
            }
            UpdateDecision::Optional { .. } => {
                self.notice(format!("Mise à jour disponible : v{}", manifest.version));
                self.update.last_manifest = Some(manifest);
            }
        }
    }

    // ── Cross-thread events ──────────────────────────────────────────────

    fn drain_tray(&mut self, ctx: &Context) {
        // The fastframe tray reports on its own channel; drain it here, on
        // the UI thread, both for windowed frames and headless ticks.
        let commands: Vec<TrayCommand> = self
            .tray
            .as_mut()
            .map_or_else(Vec::new, crate::tray::TrayHandle::drain);
        for command in commands {
            match command {
                TrayCommand::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    self.show_requested = true;
                }
                TrayCommand::QuickStats => {
                    if self.session.is_some() {
                        self.screen = Screen::Shell(Route::Dashboard);
                        self.show_requested = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    }
                }
                TrayCommand::Quit => {
                    self.wants_quit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn consume_deep_links(&mut self) {
        if let Ok(mut queue) = self.incoming_links.lock() {
            self.pending_deep_links.append(&mut queue);
        }
        let links = std::mem::take(&mut self.pending_deep_links);
        for raw in links {
            if raw == crate::SHOW_REQUEST {
                // A second launch asked this instance to come forward. The
                // headless loop turns this flag into Headless::Show; when a
                // window already exists, poll_background makes it visible.
                self.show_requested = true;
                continue;
            }
            if let Some(link) = atlas_core::VerifyLink::parse(&raw)
                && self.session.is_none()
                && !self.requires_password
            {
                // A verify link only makes sense on the login screen.
                self.login.pending_link = Some(link);
                self.screen = Screen::Login;
            } else if let Some(token) = atlas_core::QrTokenParser::extract(&raw)
                && self.session.is_none()
                && !self.requires_password
            {
                self.qr.active = true;
                self.qr.token = token;
                self.qr.status = "pending".to_owned();
                self.qr.next_poll = None;
                self.screen = Screen::Login;
            }
        }
    }

    /// One frame's worth of background work. Called from both the windowed
    /// eframe loop and the headless shell loop.
    pub fn poll_background(&mut self, ctx: &Context) {
        self.drain_power_events();
        self.drain_tray(ctx);
        self.consume_deep_links();
        // Release HTTP-loaded avatar textures queued by state changes.
        if let Some(uri) = self.avatar_evict.take() {
            ctx.forget_image(&uri);
        }
        // A queued show request while a window already exists: bring it back.
        // With no viewport (hidden to tray) headless_frame returns
        // Headless::Show from the same flag instead.
        if self.show_requested
            && ctx
                .input(|input| input.viewport().visible())
                .unwrap_or(false)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        }
        screens::auth::poll_login_jobs(self);

        // Caches.
        if self.dashboard.poll() {
            ctx.request_repaint();
        }
        if self.products.poll() {
            ctx.request_repaint();
        }
        if self.clients.poll() {
            ctx.request_repaint();
        }
        if self.vendors.poll() {
            ctx.request_repaint();
        }
        if self.expenses.poll() {
            ctx.request_repaint();
        }
        if self.delivery_notes.poll() {
            ctx.request_repaint();
        }
        if self.categories.poll() {
            ctx.request_repaint();
        }
        if self.units.poll() {
            ctx.request_repaint();
        }
        if self.invoices.poll() {
            ctx.request_repaint();
        }
        if self.alerts.poll() {
            self.maybe_notify_alerts();
            ctx.request_repaint();
        }
        if self.devices.poll() {
            ctx.request_repaint();
        }
        if self.nav_badges.poll() {
            ctx.request_repaint();
        }
        if self.profile.poll() {
            if let Some(me) = self.profile.data.clone() {
                self.fill_profile_form(&me);
            }
            ctx.request_repaint();
        }
        if self.assist.poll() {
            ctx.request_repaint();
        }
        if self.api_keys.poll() {
            if self.api_keys_revoke_pending {
                self.api_keys_revoke_pending = false;
                if self.api_keys.error.is_none() {
                    self.notice(t!(self.locale, "API key revoked.").into_owned());
                }
            }
            ctx.request_repaint();
        }
        self.poll_profile_jobs();
        self.poll_agent_jobs();
        self.poll_update_jobs(ctx);
        self.enforce_session_binding();

        // Periodic refresh while a session is live. A cache only enters the
        // rotation once its screen has actually been opened (last_updated is
        // Some): an unvisited route never generates background traffic, so
        // the app no longer fans out one request per screen on a timer.
        if self.session.is_some() && !self.power_paused {
            let interval = Duration::from_secs(self.settings.poll_interval_secs.max(15));
            if self.dashboard.last_updated.is_some() && self.dashboard.stale_for(interval) {
                self.load_dashboard();
            }
            if self.products.last_updated.is_some()
                && self.products.stale_for(interval)
                && self.product_search.is_empty()
            {
                self.load_products();
            }
            if self.alerts.last_updated.is_some() && self.alerts.stale_for(interval) {
                self.load_alerts();
            }
            if self.nav_badges.last_updated.is_some() && self.nav_badges.stale_for(interval) {
                self.load_nav_badges();
            }
        }
    }

    /// A 401 on an authenticated call whose code is not a user mistake means
    /// the server-side (IP, fingerprint) binding changed: the token is dead
    /// and the worker demands a fresh login.
    fn enforce_session_binding(&mut self) {
        if self.session.is_none() {
            return;
        }
        let expired = [
            &self.dashboard.last_error,
            &self.products.last_error,
            &self.invoices.last_error,
            &self.alerts.last_error,
            &self.devices.last_error,
            &self.nav_badges.last_error,
            &self.profile.last_error,
            &self.api_keys.last_error,
        ]
        .iter()
        .any(|error| {
            error
                .as_ref()
                .is_some_and(|error| error.is_session_expired(true))
        });
        if expired {
            self.notice("Votre session a expiré. Veuillez vous reconnecter.".to_owned());
            self.sign_out();
        }
    }

    fn maybe_notify_alerts(&mut self) {
        if !self.settings.notifications_enabled {
            return;
        }
        let Some(alerts) = &self.alerts.data else {
            return;
        };
        let high = alerts
            .iter()
            .filter(|alert| alert.severity.as_deref() == Some("high"))
            .count();
        if high > self.notification_state.last_high_severity_count
            && let Some(client) = &self.notifications
        {
            let newest = alerts
                .iter()
                .find(|alert| alert.severity.as_deref() == Some("high"));
            if let Some(alert) = newest {
                let notification =
                    fastframe_notifications::Notification::new("Alerte stock Atlas Commerce")
                        .body(alert.label.clone())
                        .category("stock");
                if let Err(error) = client.show(notification) {
                    log::warn!("notification failed: {error:?}");
                }
            }
        }
        self.notification_state.last_high_severity_count = high;
    }

    fn poll_agent_jobs(&mut self) {
        if let Some(job) = &self.agent.job_policy
            && let Some(result) = job.poll()
        {
            self.agent.job_policy = None;
            if let Ok(policy) = result {
                self.agent.policy = Some(policy);
                self.agent.policy_retry_at = None;
            } else {
                // Back off: without this gate the next frame would spawn a
                // new request while the previous error is still on screen.
                self.agent.policy_retry_at =
                    Some(Instant::now() + crate::state::POLICY_RETRY_BACKOFF);
            }
        }
        if let Some(job) = &self.agent.job
            && let Some(result) = job.poll()
        {
            self.agent.job = None;
            self.agent.busy = false;
            match result {
                Ok(reply) => {
                    let text = if reply.reply.is_empty() {
                        "Aucune réponse de l'agent.".to_owned()
                    } else {
                        reply.reply
                    };
                    self.agent.push_message(false, text);
                }
                Err(error) => {
                    self.agent
                        .push_message(false, format!("Erreur : {}", error.message));
                }
            }
        }
    }

    fn drain_power_events(&mut self) {
        for event in self.power.events() {
            match event {
                fastframe_power::Event::Suspending(ack) => {
                    self.handle_suspending();
                    self.save_settings();
                    self.save_servers();
                    self.save_session();
                    ack.done();
                }
                fastframe_power::Event::Resumed => self.handle_resume(),
                fastframe_power::Event::EndingSession(ack) => {
                    self.handle_suspending();
                    self.save_settings();
                    self.save_servers();
                    self.save_session();
                    ack.done();
                }
            }
        }
    }

    /// Called by the binary when the system resumes from sleep.
    pub fn handle_resume(&mut self) {
        self.power_paused = false;
        // Reload every cache the user has actually opened. Clearing the
        // timestamps instead would make the periodic rotation skip them,
        // since it only refreshes caches whose last_updated is Some.
        if self.session.is_some() {
            if self.dashboard.last_updated.is_some() {
                self.load_dashboard();
            }
            if self.products.last_updated.is_some() && self.product_search.is_empty() {
                self.load_products();
            }
            if self.alerts.last_updated.is_some() {
                self.load_alerts();
            }
            if self.nav_badges.last_updated.is_some() {
                self.load_nav_badges();
            }
        }
    }

    /// Called by the binary when the system is about to sleep.
    pub fn handle_suspending(&mut self) {
        self.lock_for_inactivity();
        self.power_paused = true;
    }

    // ── Frame helpers ────────────────────────────────────────────────────

    /// Whether any background work is in flight and the frame should keep
    /// repainting on its own.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.login.busy
            || self.qr.job_status.is_some()
            || self.qr.job_create.is_some()
            || self.qr.job_complete.is_some()
            || self.server_form.probe_job.is_some()
            || self.dashboard.loading
            || self.products.loading
            || self.invoices.loading
            || self.alerts.loading
            || self.devices.loading
            || self.profile.loading
            || self.api_keys.loading
            || self.assist.loading
            || self.profile_save_job.is_some()
            || self.key_create_job.is_some()
            || self.agent.busy
            || self.update.checking
            || self.update.progress.is_some()
    }

    /// Draws the whole window for the current screen.
    pub fn draw(&mut self, ui: &mut Ui, ctx: &Context, palette: &Palette) {
        match self.screen {
            Screen::Splash => screens::auth::draw_splash(self, ui, palette),
            Screen::Onboarding => screens::auth::draw_onboarding(self, ui, palette),
            Screen::ServerSetup => {
                screens::auth::draw_server_setup(self, ui, palette, false);
            }
            Screen::Login => screens::auth::draw_login(self, ui, palette),
            Screen::UpdateGate => screens::auth::draw_update_gate(self, ui, palette),
            Screen::Shell(_) => screens::shell::draw_shell(self, ui, ctx, palette),
        }
        screens::notices(self, ui, palette);
    }
}

// ── fastframe shell Resident ────────────────────────────────────────────────

impl Resident for AtlasApp {
    fn closed(&self) -> fastframe_shell::Closed {
        if self.wants_quit || !self.tray_available || !self.settings.close_to_tray {
            fastframe_shell::Closed::Quit
        } else {
            fastframe_shell::Closed::Hide
        }
    }

    fn window_gone(&mut self) {
        self.lock_for_inactivity();
        self.show_requested = false;
    }

    fn headless_frame(&mut self, ctx: &Context) -> Headless {
        self.poll_background(ctx);
        self.lock_for_inactivity();
        if self.wants_quit {
            return Headless::Quit;
        }
        if self.show_requested {
            self.show_requested = false;
            return Headless::Show;
        }
        // Stay asleep between polls; the waker cuts the wait for tray events.
        Headless::WaitFor(Duration::from_secs(
            self.settings.poll_interval_secs.max(15),
        ))
    }

    fn start_hidden(&mut self) -> bool {
        if self.tray_available && self.settings.close_to_tray {
            // macOS builds its menu-bar item on demand while no window
            // exists; the other platforms no-op here.
            if let Some(tray) = self.tray.as_mut() {
                tray.create_item();
            }
            true
        } else {
            false
        }
    }

    fn shutdown(&mut self) {
        self.save_settings();
        self.save_servers();
        self.save_session();
        self.save_accounts();
    }
}

// ── eframe wrapper ──────────────────────────────────────────────────────────

/// The eframe app owning the shell-held [`AtlasApp`].
pub struct EframeShell {
    pub held: Held<AtlasApp>,
    first_frame: bool,
}

impl EframeShell {
    #[must_use]
    pub fn new(lease: Lease<AtlasApp>, ctx: &Context) -> Self {
        // `take` attaches the shell waker to this context itself.
        let mut held = lease.take(ctx);
        // Hiding to the tray destroys the eframe window while the app state
        // survives; showing it again builds a brand-new egui Context with
        // stock styles. The retained cache would otherwise match the
        // palette and make the first frame skip theme::apply on that fresh
        // context, so every new context starts with an empty cache.
        held.applied_palette = None;
        held.applied_reduced_motion = None;
        crate::theme::install_fonts_and_icons(ctx);
        // Make egui's own preference follow the app setting before the first
        // palette read, so a forced Light/Dark mode resolves correctly even
        // when the OS reports the opposite appearance.
        held.sync_egui_theme(ctx);
        // eframe asks for the clear colour before the first `logic` run, so
        // seed it at construction (the CreationContext already knows the
        // system theme) to avoid the dark fallback on even the first frame.
        held.last_clear_color = Some(held.palette(ctx).window.to_normalized_gamma_f32());
        Self {
            held,
            first_frame: true,
        }
    }
}

impl eframe::App for EframeShell {
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Match the active theme from the first frame on, so a light theme
        // never flashes the dark background before the first UI frame.
        // The dark token (#111113) is only the pre-first-frame fallback.
        self.held
            .last_clear_color
            .unwrap_or_else(|| egui::Color32::from_rgb(0x11, 0x11, 0x13).to_normalized_gamma_f32())
    }

    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if self.first_frame {
            self.first_frame = false;
            // The viewport was born hidden (see native_options): reveal it
            // only now that the first frame carries the real palette, fonts
            // and screen, so no unstyled white window ever reaches the
            // screen. The command applies after this frame presents.
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            // macOS moves the status item onto the main window's AppKit loop.
            if let Some(tray) = self.held.tray.as_mut() {
                tray.attach();
            }
            if let Some(receipt) = self.held.update_receipt.take() {
                // The helper stands ready to roll back until the new build
                // proves it can draw; answer off the UI thread.
                std::thread::spawn(move || {
                    if let Err(error) = receipt.acknowledge() {
                        log::warn!("update receipt not acknowledged: {error}");
                    }
                });
            }
        }
        // Keep egui's preference and the native window chrome aligned with
        // the app setting before resolving the palette for this frame.
        self.held.sync_egui_theme(ctx);
        self.held.scrolling.apply(ctx);
        let palette = self.held.palette(ctx);
        // Keep the framebuffer clear colour in sync, and rebuild the global
        // styles only when the palette actually changes instead of every
        // frame. Comparing the whole palette also catches token edits, not
        // just dark/light flips.
        self.held.last_clear_color = Some(palette.window.to_normalized_gamma_f32());
        let reduced_motion = self.held.settings.reduced_motion;
        if self.held.applied_palette != Some(palette)
            || self.held.applied_reduced_motion != Some(reduced_motion)
        {
            crate::theme::apply(ctx, &palette, reduced_motion);
            self.held.applied_palette = Some(palette);
            self.held.applied_reduced_motion = Some(reduced_motion);
        }

        // Window title contract: "Atlas Commerce — <serveur>" once a server
        // is selected, plain "Atlas Commerce" on the auth screens. Pushed
        // only when it changes, so no per-frame viewport commands.
        let window_title = match self.held.server_store.selected_url() {
            Some(base) => format!("Atlas Commerce — {}", atlas_core::ServerUrl::host(base)),
            None => "Atlas Commerce".to_owned(),
        };
        if self.held.applied_title.as_deref() != Some(window_title.as_str()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title.clone()));
            self.held.applied_title = Some(window_title);
        }

        self.held.poll_background(ctx);
        let lock = ctx.input(|input| {
            let viewport = input.viewport();
            should_lock(
                viewport.focused.unwrap_or(input.focused),
                viewport.minimized,
                viewport.close_requested(),
            )
        });
        if lock {
            self.held.lock_for_inactivity();
        }

        // Keep repainting while any job runs, so spinners and poll results
        // appear without user input.
        if self.held.busy() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let mut palette = self.held.palette(&ctx);
        if palette.dark && matches!(self.held.screen, Screen::Login | Screen::ServerSetup | Screen::Onboarding) {
            // SOURCED-FROM: supplied authentication layout specification.
            palette.window = egui::Color32::from_rgb(0x12, 0x13, 0x16);
            palette.surface = egui::Color32::from_rgb(0x1e, 0x20, 0x25);
        }
        if !matches!(self.held.screen, Screen::Shell(_)) && fastframe_titlebar::enabled() {
            egui::Panel::top(egui::Id::new("auth-titlebar"))
                .exact_size(48.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::NONE.fill(palette.window))
                .show(ui, |ui| {
                    let rect = ui.available_rect_before_wrap();
                    ui.allocate_exact_size(rect.size(), egui::Sense::hover());
                    fastframe_titlebar::drag(ui, rect);
                    let footprint = fastframe_titlebar::footprint(&ctx, 48.0);
                    let style = fastframe_titlebar::Style {
                        icon: palette.secondary,
                        button: palette.surface_hover,
                        header_height: 48.0,
                    };
                    let minimize = t!(self.held.locale, "Minimize").into_owned();
                    let maximize = t!(self.held.locale, "Maximize").into_owned();
                    let restore = t!(self.held.locale, "Restore").into_owned();
                    let close = t!(self.held.locale, "Close").into_owned();
                    let labels = fastframe_titlebar::Labels {
                        minimize: &minimize,
                        maximize: &maximize,
                        restore: &restore,
                        close: &close,
                    };
                    crate::widgets::overlay_at(
                        ui,
                        egui::Rect::from_min_size(
                            egui::pos2(rect.right() - footprint.right - 4.0, rect.top()),
                            egui::vec2(footprint.right.max(120.0), 48.0),
                        ),
                        |ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                fastframe_titlebar::controls(ui, &style, &labels);
                            });
                        },
                    );
                });
        }
        if matches!(self.held.screen, Screen::Shell(_)) {
            // The shell creates its own top/left/central panels; wrapping it
            // in another CentralPanel would nest panels and fight for the
            // available rect.
            self.held.draw(ui, &ctx, &palette);
        } else {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::NONE
                        .fill(palette.window)
                        .inner_margin(egui::Margin::same(0)),
                )
                .show(ui, |ui| {
                    self.held.draw(ui, &ctx, &palette);
                });
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.held.save_settings();
    }
}

#[cfg(test)]
mod security_tests {
    use super::should_lock;

    #[test]
    fn explicit_focus_loss_minimize_and_close_lock_the_session() {
        assert!(should_lock(false, Some(false), false));
        assert!(should_lock(true, Some(true), false));
        assert!(should_lock(true, Some(false), true));
        assert!(!should_lock(true, Some(false), false));
        assert!(should_lock(false, None, false));
    }
}
