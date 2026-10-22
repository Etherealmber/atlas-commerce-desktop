//! Central application state: services, navigation, caches and forms.
//!
//! The struct is owned by the fastframe shell and borrowed by one window at a
//! a time. All network work runs on the [`atlas_core::JobRunner`]; the UI
//! thread only polls [`atlas_core::Job`] handles and never blocks.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use atlas_api::models::*;
use atlas_api::{ApiClient, ApiError};
use atlas_core::jobs::Job;
use atlas_core::settings::Settings;
use atlas_core::*;
use egui::Context;

use crate::i18n::Locale;
use crate::screens::products::ProductEditor;
use crate::screens::products::StockDialog;
use crate::screens::sales::QuickSaleDraft;
use crate::theme::Palette;

/// A pollable network result with loading/error bookkeeping.
pub struct Cache<T> {
    pub data: Option<T>,
    pub loading: bool,
    pub error: Option<String>,
    /// The whole last failure, so a dead session token can force a re-login.
    pub last_error: Option<ApiError>,
    pub job: Option<Job<Result<T, ApiError>>>,
    pub last_updated: Option<Instant>,
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self {
            data: None,
            loading: false,
            error: None,
            last_error: None,
            job: None,
            last_updated: None,
        }
    }
}

impl<T> Cache<T> {
    pub fn start(&mut self, job: Job<Result<T, ApiError>>) {
        self.job = Some(job);
        self.loading = true;
        self.error = None;
    }

    /// Polls the job once; returns true when it finished this frame.
    pub fn poll(&mut self) -> bool {
        if let Some(job) = &self.job
            && let Some(result) = job.poll()
        {
            self.job = None;
            self.loading = false;
            self.last_updated = Some(Instant::now());
            match result {
                Ok(data) => {
                    self.data = Some(data);
                    self.last_error = None;
                    self.error = None;
                }
                Err(error) => {
                    self.error = Some(error.message.clone());
                    self.last_error = Some(error);
                }
            }
            return true;
        }
        false
    }

    #[must_use]
    pub fn stale_for(&self, interval: Duration) -> bool {
        match self.last_updated {
            Some(at) => at.elapsed() >= interval,
            None => true,
        }
    }
}

/// Top-level routes inside the shell's left rail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Dashboard,
    Products,
    Sales,
    Agent,
    Notifications,
    Settings,
}

impl Route {
    pub const ALL: [Self; 6] = [
        Self::Dashboard,
        Self::Products,
        Self::Sales,
        Self::Agent,
        Self::Notifications,
        Self::Settings,
    ];
}

/// The boot/bootstrapping screens before the shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    Splash,
    Onboarding,
    ServerSetup,
    Login,
    /// Blocking mandatory-update screen reached before the shell.
    UpdateGate,
    Shell(Route),
}

/// QR pairing state for the PC side.
#[derive(Default)]
pub struct QrPairing {
    pub active: bool,
    pub token: String,
    pub qr_url: String,
    pub status: String,
    pub account: String,
    pub expired: bool,
    pub job_status: Option<Job<Result<QrSessionStatus, ApiError>>>,
    pub job_create: Option<Job<Result<QrSessionCreated, ApiError>>>,
    pub job_complete: Option<Job<Result<QrComplete, ApiError>>>,
    pub next_poll: Option<Instant>,
}

/// Server-setup form and its live probe.
#[derive(Default)]
pub struct ServerSetupForm {
    pub custom_url: String,
    pub custom_label: String,
    pub probe_report: Option<atlas_api::probe::ProbeReport>,
    pub probe_job: Option<Job<atlas_api::probe::ProbeReport>>,
    pub saved_error: Option<String>,
}

/// Login form and its in-flight calls.
#[derive(Default)]
pub struct LoginState {
    pub username: String,
    pub password: String,
    pub otp: String,
    pub show_password: bool,
    pub needs_otp: bool,
    pub notice: Option<String>,
    pub error: Option<String>,
    pub busy: bool,
    pub job_login: Option<Job<Result<LoginResponse, ApiError>>>,
    pub job_otp: Option<Job<Result<LoginResponse, ApiError>>>,
    pub job_link: Option<Job<Result<LoginResponse, ApiError>>>,
    /// A verify-link deep link consumed once the login screen appears.
    pub pending_link: Option<VerifyLink>,
}

/// Self-update state.
#[derive(Default)]
pub struct UpdateState {
    pub last_manifest: Option<UpdateManifest>,
    pub mandatory: Option<UpdateManifest>,
    pub checking: bool,
    pub job: Option<Job<Result<Option<UpdateManifest>, String>>>,
    pub progress: Option<(u64, u64)>,
    pub stage: String,
    pub error: Option<String>,
}

/// Agent conversation.
#[derive(Default, Clone)]
pub struct ChatMessage {
    pub from_user: bool,
    pub text: String,
}

#[derive(Default)]
pub struct AgentState {
    pub messages: Vec<ChatMessage>,
    pub draft: String,
    pub busy: bool,
    /// Persistent worker-side conversation id for this account.
    pub thread_id: String,
    pub policy: Option<AgentPolicy>,
    pub job: Option<Job<Result<AgentReply, ApiError>>>,
    pub job_policy: Option<Job<Result<AgentPolicy, ApiError>>>,
}

/// Notifications seen during the last alerts poll, for native toasts.
#[derive(Default)]
pub struct NotificationState {
    pub last_high_severity_count: usize,
}

/// The whole application.
pub struct AtlasApp {
    // ── services ──
    pub paths: AppPaths,
    pub runner: JobRunner,
    pub identity: DeviceIdentity,
    pub client: Option<Arc<ApiClient>>,
    pub waker: fastframe_shell::Waker,
    pub power: fastframe_power::Power,
    /// Deep links (`atlascommerce://…`) handed over by a second launch.
    pub incoming_links: Arc<Mutex<Vec<String>>>,
    /// System-tray handle, drained on the UI thread each frame/tick.
    pub tray: Option<crate::tray::TrayHandle>,
    pub notifications: Option<fastframe_notifications::Client>,
    pub scrolling: fastframe_scroll::Scrolling,
    pub power_paused: bool,
    /// A tray host exists, so closing may hide instead of quitting.
    pub tray_available: bool,
    /// Platform login-item registration, read off the UI thread at startup.
    pub autostart_state: fastframe_autostart::State,
    /// The user asked the whole process to end.
    pub wants_quit: bool,
    /// A headless tick or tray event asked the window to reappear.
    pub show_requested: bool,

    // ── persisted state ──
    pub settings: Settings,
    pub locale: Locale,
    pub server_store: ServerStore,
    pub session: Option<SessionRecord>,
    pub account_store: AccountStore,

    // ── navigation / boot ──
    pub screen: Screen,
    pub started_at: Instant,
    pub pending_deep_links: Vec<String>,
    pub force_server_setup: bool,

    // ── auth ──
    pub server_form: ServerSetupForm,
    pub login: LoginState,
    pub qr: QrPairing,
    pub devices: Cache<Vec<TrustedDevice>>,

    // ── commerce caches ──
    pub dashboard: Cache<Dashboard>,
    pub products: Cache<Vec<Product>>,
    pub product_search: String,
    pub invoices: Cache<Vec<Invoice>>,
    pub alerts: Cache<Vec<Alert>>,
    pub nav_badges: Cache<NavBadges>,

    // ── editors / dialogs ──
    pub product_editor: Option<ProductEditor>,
    pub stock_dialog: Option<StockDialog>,
    pub quick_sale: Option<QuickSaleDraft>,
    pub confirm_dialog: Option<crate::dialogs::Confirm>,

    // ── agent / notifications ──
    pub agent: AgentState,
    pub notification_state: NotificationState,

    // ── updates ──
    pub update: UpdateState,
    /// Live download progress shared with the blocking updater thread.
    pub update_progress: Arc<Mutex<(u64, u64)>>,
    /// The download-and-handoff job, distinct from the manifest check.
    pub update_download_job: Option<Job<Result<(), String>>>,

    // ── transient notices ──
    pub notices: Vec<String>,
}

impl AtlasApp {
    /// Builds the app from already-loaded on-disk state.
    #[must_use]
    #[allow(
        clippy::too_many_arguments,
        reason = "single composition root wiring every process service into the app"
    )]
    pub fn new(
        paths: AppPaths,
        runner: JobRunner,
        identity: DeviceIdentity,
        settings: Settings,
        server_store: ServerStore,
        session: Option<SessionRecord>,
        account_store: AccountStore,
        waker: fastframe_shell::Waker,
        power: fastframe_power::Power,
        incoming_links: Arc<Mutex<Vec<String>>>,
        tray: Option<crate::tray::TrayHandle>,
    ) -> Self {
        let locale = Locale::from_tag(&settings.language);
        let screen = if session.is_some() {
            Screen::Splash
        } else if !server_store.onboarding_done || server_store.selected_url().is_none() {
            Screen::Onboarding
        } else {
            Screen::Login
        };
        Self {
            paths,
            runner,
            identity,
            client: None,
            waker,
            power,
            incoming_links,
            tray,
            notifications: None,
            scrolling: fastframe_scroll::Scrolling::default(),
            power_paused: false,
            tray_available: false,
            autostart_state: fastframe_autostart::State::Off,
            wants_quit: false,
            show_requested: false,
            locale,
            settings,
            server_store,
            session,
            account_store,
            screen,
            started_at: Instant::now(),
            pending_deep_links: Vec::new(),
            force_server_setup: false,
            server_form: ServerSetupForm::default(),
            login: LoginState::default(),
            qr: QrPairing::default(),
            devices: Cache::default(),
            dashboard: Cache::default(),
            products: Cache::default(),
            product_search: String::new(),
            invoices: Cache::default(),
            alerts: Cache::default(),
            nav_badges: Cache::default(),
            product_editor: None,
            stock_dialog: None,
            quick_sale: None,
            confirm_dialog: None,
            agent: AgentState::default(),
            notification_state: NotificationState::default(),
            update: UpdateState::default(),
            update_progress: Arc::new(Mutex::new((0, 0))),
            update_download_job: None,
            notices: Vec::new(),
        }
    }

    // ── palette ──────────────────────────────────────────────────────────

    #[must_use]
    pub fn palette(&self, ctx: &Context) -> Palette {
        let dark = match self.settings.theme {
            atlas_core::settings::ThemeMode::Dark => true,
            atlas_core::settings::ThemeMode::Light => false,
            atlas_core::settings::ThemeMode::System => ctx.theme() == egui::Theme::Dark,
        };
        crate::theme::resolve(dark)
    }

    // ── client helpers ───────────────────────────────────────────────────

    #[must_use]
    pub fn client(&self) -> Option<&Arc<ApiClient>> {
        self.client.as_ref()
    }

    /// (Re)builds the API client for the selected server and restores its
    /// token, if any.
    pub fn rebuild_client(&mut self) {
        let Some(base) = self.server_store.selected_url().map(str::to_owned) else {
            self.client = None;
            return;
        };
        match ApiClient::new(
            &base,
            self.identity.fingerprint(),
            &DeviceIdentity::user_agent(),
        ) {
            Ok(client) => {
                if let Some(session) = &self.session {
                    client.set_token(Some(session.token.clone()));
                }
                self.client = Some(Arc::new(client));
            }
            Err(error) => self.notices.push(error.message),
        }
    }

    // ── persistence, always off the UI thread ────────────────────────────

    pub fn save_settings(&self) {
        let path = self.paths.settings_file();
        let json = serde_json::to_vec_pretty(&self.settings).unwrap_or_default();
        persist(path, json);
    }

    pub fn save_servers(&self) {
        let path = self.paths.servers_file();
        let json = serde_json::to_vec_pretty(&self.server_store).unwrap_or_default();
        persist(path, json);
    }

    pub fn save_session(&self) {
        let path = self.paths.session_file();
        match &self.session {
            Some(session) => {
                let json = serde_json::to_vec_pretty(session).unwrap_or_default();
                persist_private(path, json);
            }
            None => {
                std::thread::spawn(move || {
                    let _ = std::fs::remove_file(path);
                });
            }
        };
    }

    pub fn save_accounts(&self) {
        let path = self.paths.accounts_file();
        let store = self.account_store.clone();
        std::thread::spawn(move || {
            let _ = store.save(&path);
        });
    }

    // ── navigation ───────────────────────────────────────────────────────

    pub fn go(&mut self, route: Route) {
        self.screen = Screen::Shell(route);
    }

    pub fn sign_out(&mut self) {
        if let Some(session) = self.session.take() {
            self.account_store.remove(&session);
            self.save_accounts();
        }
        self.save_session();
        self.client = None;
        self.session = None;
        self.login = LoginState::default();
        self.qr = QrPairing::default();
        self.dashboard = Cache::default();
        self.products = Cache::default();
        self.invoices = Cache::default();
        self.alerts = Cache::default();
        self.nav_badges = Cache::default();
        self.agent = AgentState::default();
        self.screen = Screen::Login;
    }

    pub fn notice(&mut self, message: impl Into<String>) {
        self.notices.push(message.into());
        if self.notices.len() > 4 {
            self.notices.remove(0);
        }
    }
}

/// Writes a file on a background thread (the shell marks the UI thread).
pub fn persist(path: std::path::PathBuf, bytes: Vec<u8>) {
    std::thread::spawn(move || {
        if let Err(error) = fastframe_file::write(&path, bytes) {
            log::warn!("could not write {}: {error}", path.display());
        }
    });
}

/// Writes a private (0600) file on a background thread.
pub fn persist_private(path: std::path::PathBuf, bytes: Vec<u8>) {
    std::thread::spawn(move || {
        if let Err(error) = fastframe_file::write_private(&path, bytes) {
            log::warn!("could not write {}: {error}", path.display());
        }
    });
}
