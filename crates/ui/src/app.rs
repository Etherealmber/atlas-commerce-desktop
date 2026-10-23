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
use crate::theme::Palette;
use crate::tray::TrayCommand;

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
        self.session = Some(record.clone());
        self.account_store.upsert(&record);
        self.save_session();
        self.save_accounts();
        self.rebuild_client();
        self.screen = Screen::Shell(Route::Dashboard);
        self.bootstrap_shell();
    }

    /// First loads after entering the shell or restoring a session.
    pub fn bootstrap_shell(&mut self) {
        self.load_dashboard();
        self.load_products();
        self.load_alerts();
        self.load_nav_badges();
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

    pub fn load_devices(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.devices().await });
        self.devices.start(job);
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
        self.update.stage = "Téléchargement…".to_owned();
        let job: atlas_core::Job<Result<(), String>> = self.runner.spawn(async move {
            tokio::task::spawn_blocking(move || {
                let updater = crate::update::build_updater().map_err(|error| error.to_string())?;
                let release = updater
                    .check()
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "Aucune version publiée trouvée.".to_owned())?;
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
            .map_err(|error| error.to_string())?
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
                        self.update.stage = "Installation…".to_owned();
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
            UpdateDecision::UpToDate => {}
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
            {
                // A verify link only makes sense on the login screen.
                self.login.pending_link = Some(link);
                self.screen = Screen::Login;
            } else if let Some(token) = atlas_core::QrTokenParser::extract(&raw)
                && self.session.is_none()
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
        // A queued show request while a window already exists: bring it back.
        // With no viewport (hidden to tray) headless_frame returns
        // Headless::Show from the same flag instead.
        if self.show_requested
            && ctx.input(|input| input.viewport().visible()).unwrap_or(false)
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
        self.poll_agent_jobs();
        self.poll_update_jobs(ctx);
        self.enforce_session_binding();

        // Periodic refresh while a session is live.
        if self.session.is_some() && !self.power_paused {
            let interval = Duration::from_secs(self.settings.poll_interval_secs.max(15));
            if self.dashboard.stale_for(interval) {
                self.load_dashboard();
            }
            if self.products.stale_for(interval) && self.product_search.is_empty() {
                self.load_products();
            }
            if self.alerts.stale_for(interval) {
                self.load_alerts();
            }
            if self.nav_badges.stale_for(interval) {
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
                    self.agent.messages.push(crate::state::ChatMessage {
                        from_user: false,
                        text: if reply.reply.is_empty() {
                            "Aucune réponse de l'agent.".to_owned()
                        } else {
                            reply.reply
                        },
                    });
                }
                Err(error) => {
                    self.agent.messages.push(crate::state::ChatMessage {
                        from_user: false,
                        text: format!("Erreur : {}", error.message),
                    });
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
        self.dashboard.last_updated = None;
        self.products.last_updated = None;
        self.alerts.last_updated = None;
    }

    /// Called by the binary when the system is about to sleep.
    pub fn handle_suspending(&mut self) {
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
        self.show_requested = false;
    }

    fn headless_frame(&mut self, ctx: &Context) -> Headless {
        self.poll_background(ctx);
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
        let held = lease.take(ctx);
        crate::theme::install_fonts_and_icons(ctx);
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
        // No white flash while the first frame builds: the dark theme's
        // `--ac-bg` token (#111113), shared with the web and Android apps.
        egui::Color32::from_rgb(0x11, 0x11, 0x13).to_normalized_gamma_f32()
    }

    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if self.first_frame {
            self.first_frame = false;
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
        self.held.scrolling.apply(ctx);
        let palette = self.held.palette(ctx);
        crate::theme::apply(ctx, &palette);

        self.held.poll_background(ctx);

        // Keep repainting while any job runs, so spinners and poll results
        // appear without user input.
        if self.held.busy() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let palette = self.held.palette(&ctx);
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

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.held.save_settings();
    }
}
