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
use crate::screens::catalog::{CatalogTab, CategoryEditor, UnitEditor};
use crate::screens::clients::ClientEditor;
use crate::screens::delivery_notes::DeliveryNoteEditor;
use crate::screens::expenses::ExpenseEditor;
use crate::screens::products::ProductEditor;
use crate::screens::products::StockDialog;
use crate::screens::sales::QuickSaleDraft;
use crate::screens::scan::ScanState;
use crate::screens::vendors::VendorEditor;
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
    Catalog,
    Clients,
    Vendors,
    Expenses,
    DeliveryNotes,
    Sales,
    Agent,
    Notifications,
    Settings,
}

impl Route {
    pub const ALL: [Self; 11] = [
        Self::Dashboard,
        Self::Products,
        Self::Catalog,
        Self::Clients,
        Self::Vendors,
        Self::Expenses,
        Self::DeliveryNotes,
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
    /// Set when a check answered that the current build is the newest, so
    /// the settings card can show an explicit "up to date" status.
    pub up_to_date_at: Option<Instant>,
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
    /// Backoff gate opened once [`Self::job_policy`] has failed, so a failing
    /// endpoint is not queried again on every frame.
    pub policy_retry_at: Option<Instant>,
}

impl AgentState {
    /// Maximum messages kept in the local transcript; the worker-side thread
    /// retains the full conversation regardless.
    const MAX_TRANSCRIPT: usize = 500;

    /// Appends a message and trims the oldest entries past the cap, so a
    /// very long single conversation cannot grow memory without a bound.
    pub fn push_message(&mut self, from_user: bool, text: String) {
        self.messages.push(ChatMessage { from_user, text });
        if self.messages.len() > Self::MAX_TRANSCRIPT {
            let excess = self.messages.len() - Self::MAX_TRANSCRIPT;
            self.messages.drain(0..excess);
        }
    }
}

/// Notifications seen during the last alerts poll, for native toasts.
#[derive(Default)]
pub struct NotificationState {
    pub last_high_severity_count: usize,
}

/// Editable profile form (`PATCH /api/auth/me`).
#[derive(Default, Clone)]
pub struct ProfileForm {
    pub display_name: String,
    pub email: String,
    pub phone: String,
    pub notes: String,
    /// An https URL to an image; file upload would need a native picker.
    pub avatar_url: String,
    /// Filled from `GET /api/auth/me` at least once this session.
    pub loaded: bool,
}

/// The explicit scope choice for a new public API key. The worker accepts
/// `read` or `read,write`; nothing is preselected so the owner chooses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiKeyScope {
    Read,
    ReadWrite,
}

impl ApiKeyScope {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::ReadWrite => "read,write",
        }
    }
}

/// Open create-key dialog state.
#[derive(Default, Clone)]
pub struct ApiKeyEditor {
    pub name: String,
    pub scopes: Option<ApiKeyScope>,
}

/// Which settings section an external entry point (avatar menu) asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsFocus {
    #[default]
    General,
    Profile,
    Integrations,
    /// Jumps to the updates/about area (release notes and repository links).
    About,
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
    /// Update-helper receipt, acknowledged on the first frame after relaunch.
    pub update_receipt: Option<fastframe_update::Receipt>,
    /// Last clear colour handed to eframe, so a light theme never flashes
    /// the hard-coded dark background on its first frames.
    pub last_clear_color: Option<[f32; 4]>,
    /// Dark flag the global egui style was last built for; skips the
    /// per-frame global-style rebuild (a flicker/recompute source).
    pub theme_applied_dark: Option<bool>,

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
    /// Back/forward stacks of shell routes, browser-style.
    pub nav_back: Vec<Route>,
    pub nav_forward: Vec<Route>,
    /// Bumped on every navigation so entrance animations retrigger.
    pub anim_token: u64,
    /// When the current route first appeared, for the entrance animation.
    pub route_entered_at: Instant,
    /// The top-bar search pill's text (submitted into the products search).
    pub top_search: String,
    /// Account popover under the title-bar avatar.
    pub profile_menu_open: bool,
    /// Ctrl+K command palette overlay.
    pub palette_open: bool,
    /// Palette filter text, reset every time the palette opens.
    pub palette_query: String,
    /// Cursor inside the filtered command list (clamped at draw time).
    pub palette_selected: usize,
    /// Section an external entry point (avatar menu) wants to highlight.
    pub settings_focus: SettingsFocus,

    // ── auth ──
    pub server_form: ServerSetupForm,
    pub login: LoginState,
    pub qr: QrPairing,
    pub devices: Cache<Vec<TrustedDevice>>,

    // ── profile / integrations ──
    pub profile: Cache<Me>,
    pub profile_form: ProfileForm,
    pub profile_save_job: Option<Job<Result<Me, ApiError>>>,
    pub api_keys: Cache<Vec<ApiKey>>,
    /// True between a revoke request and the refreshed list completing, so
    /// the confirmation notice only appears on a successful deletion.
    pub api_keys_revoke_pending: bool,
    pub key_editor: Option<ApiKeyEditor>,
    /// The full secret, kept only between creation and the modal's dismissal.
    pub created_key: Option<CreatedApiKey>,
    pub key_create_job: Option<Job<Result<CreatedApiKey, ApiError>>>,
    /// In-flight report link: mints a short-lived ticket and resolves to the
    /// printable browser URL that must be handed to the OS opener.
    pub report_job: Option<Job<Result<String, ApiError>>>,
    pub assist: Cache<AssistProvider>,

    // ── commerce caches ──
    pub dashboard: Cache<Dashboard>,
    pub products: Cache<Vec<Product>>,
    pub product_search: String,
    pub clients: Cache<Vec<Client>>,
    pub client_search: String,
    pub selected_client: Option<i64>,
    pub vendors: Cache<Vec<Vendor>>,
    pub vendor_search: String,
    pub selected_vendor: Option<i64>,
    pub expenses: Cache<Vec<Expense>>,
    pub expense_search: String,
    pub expense_editor: Option<ExpenseEditor>,
    pub delivery_notes: Cache<Vec<DeliveryNote>>,
    pub delivery_note_editor: Option<DeliveryNoteEditor>,
    pub scan: ScanState,
    pub categories: Cache<Vec<Category>>,
    pub units: Cache<Vec<Unit>>,
    pub catalog_tab: CatalogTab,
    pub category_editor: Option<CategoryEditor>,
    pub unit_editor: Option<UnitEditor>,
    pub invoices: Cache<Vec<Invoice>>,
    pub alerts: Cache<Vec<Alert>>,
    pub nav_badges: Cache<NavBadges>,

    // ── editors / dialogs ──
    pub product_editor: Option<ProductEditor>,
    pub client_editor: Option<ClientEditor>,
    pub vendor_editor: Option<VendorEditor>,
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
    /// When the current notice stack expires and should be cleared.
    pub notices_expire_at: Option<Instant>,
}

/// How long a toast stays on screen before the stack is cleared.
pub const NOTICE_TTL: Duration = Duration::from_secs(4);

/// Wait between two agent-policy fetches after a failed attempt.
pub const POLICY_RETRY_BACKOFF: Duration = Duration::from_secs(30);

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
            profile: Cache::default(),
            profile_form: ProfileForm::default(),
            profile_save_job: None,
            api_keys: Cache::default(),
            api_keys_revoke_pending: false,
            key_editor: None,
            created_key: None,
            key_create_job: None,
            report_job: None,
            assist: Cache::default(),
            nav_back: Vec::new(),
            nav_forward: Vec::new(),
            anim_token: 0,
            route_entered_at: Instant::now(),
            top_search: String::new(),
            profile_menu_open: false,
            palette_open: false,
            palette_query: String::new(),
            palette_selected: 0,
            settings_focus: SettingsFocus::General,
            dashboard: Cache::default(),
            products: Cache::default(),
            product_search: String::new(),
            clients: Cache::default(),
            client_search: String::new(),
            selected_client: None,
            vendors: Cache::default(),
            vendor_search: String::new(),
            selected_vendor: None,
            expenses: Cache::default(),
            expense_search: String::new(),
            expense_editor: None,
            delivery_notes: Cache::default(),
            delivery_note_editor: None,
            scan: ScanState::default(),
            categories: Cache::default(),
            units: Cache::default(),
            catalog_tab: CatalogTab::default(),
            invoices: Cache::default(),
            alerts: Cache::default(),
            nav_badges: Cache::default(),
            product_editor: None,
            client_editor: None,
            vendor_editor: None,
            category_editor: None,
            unit_editor: None,
            stock_dialog: None,
            quick_sale: None,
            confirm_dialog: None,
            agent: AgentState::default(),
            notification_state: NotificationState::default(),
            update: UpdateState::default(),
            update_progress: Arc::new(Mutex::new((0, 0))),
            update_download_job: None,
            update_receipt: None,
            last_clear_color: None,
            theme_applied_dark: None,
            notices: Vec::new(),
            notices_expire_at: None,
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
            Err(error) => self.notice(error.message),
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

    /// Opens a shell route, recording the current one for the back button.
    pub fn go(&mut self, route: Route) {
        if let Screen::Shell(current) = self.screen
            && current != route
        {
            self.nav_back.push(current);
            // Bound history so a long-lived session cannot grow it without
            // limit; the oldest entries fall off the back of the stack.
            const MAX_HISTORY: usize = 32;
            if self.nav_back.len() > MAX_HISTORY {
                let excess = self.nav_back.len() - MAX_HISTORY;
                self.nav_back.drain(0..excess);
            }
            // A new branch of history invalidates the forward stack.
            self.nav_forward.clear();
            self.begin_route_transition();
        }
        self.screen = Screen::Shell(route);
    }

    /// Browser-style back. Returns false when no history is available.
    pub fn go_back(&mut self) -> bool {
        let Some(previous) = self.nav_back.pop() else {
            return false;
        };
        if let Screen::Shell(current) = self.screen {
            self.nav_forward.push(current);
        }
        self.begin_route_transition();
        self.screen = Screen::Shell(previous);
        true
    }

    /// Browser-style forward.
    pub fn go_forward(&mut self) -> bool {
        let Some(next) = self.nav_forward.pop() else {
            return false;
        };
        if let Screen::Shell(current) = self.screen {
            self.nav_back.push(current);
        }
        self.begin_route_transition();
        self.screen = Screen::Shell(next);
        true
    }

    fn begin_route_transition(&mut self) {
        self.anim_token = self.anim_token.wrapping_add(1);
        self.route_entered_at = Instant::now();
    }

    // ── Keyboard-first navigation (Ctrl+K palette, Ctrl+R, Escape) ───────

    /// Opens the command palette with a fresh filter and selection.
    pub fn open_palette(&mut self) {
        self.palette_open = true;
        self.palette_query.clear();
        self.palette_selected = 0;
        self.profile_menu_open = false;
    }

    /// Closes the palette and forgets its transient text.
    pub fn close_palette(&mut self) {
        self.palette_open = false;
        self.palette_query.clear();
        self.palette_selected = 0;
    }

    /// Lazy-loads the data a route needs the first time it is opened. Shared
    /// by the rail click and the command palette so the two entry points can
    /// never drift apart.
    pub fn ensure_route_data(&mut self, route: Route) {
        match route {
            Route::Clients if self.clients.data.is_none() => self.load_clients(),
            Route::Vendors if self.vendors.data.is_none() => self.load_vendors(),
            Route::Expenses if self.expenses.data.is_none() => self.load_expenses(),
            Route::DeliveryNotes if self.delivery_notes.data.is_none() => {
                self.load_delivery_notes();
            }
            Route::Catalog => {
                if self.categories.data.is_none() {
                    self.load_categories();
                }
                if self.units.data.is_none() {
                    self.load_units();
                }
            }
            Route::Sales if self.invoices.data.is_none() => self.load_invoices(),
            Route::Notifications if self.alerts.data.is_none() => self.load_alerts(),
            _ => {}
        }
    }

    /// Reloads the current screen's data (Ctrl+R / F5), never blocking the
    /// UI thread: every loader only spawns a tokio job.
    pub fn refresh_current_route(&mut self) {
        let Screen::Shell(route) = self.screen else {
            return;
        };
        match route {
            Route::Dashboard => self.load_dashboard(),
            Route::Products => self.load_products(),
            Route::Catalog => {
                self.load_categories();
                self.load_units();
            }
            Route::Clients => self.load_clients(),
            Route::Vendors => self.load_vendors(),
            Route::Expenses => self.load_expenses(),
            Route::DeliveryNotes => self.load_delivery_notes(),
            Route::Sales => self.load_invoices(),
            Route::Agent => self.load_assist(),
            Route::Notifications => {
                self.load_alerts();
                self.load_nav_badges();
            }
            Route::Settings => {
                self.load_profile();
                self.load_devices();
                self.load_api_keys();
            }
        }
    }

    /// Whether a modal surface (dialog or editor overlay) is on screen.
    /// Global shortcuts except Escape stay inactive while this is true so a
    /// navigation, refresh or palette action cannot lose in-progress input.
    #[must_use]
    pub fn modal_open(&self) -> bool {
        self.confirm_dialog.is_some()
            || self.product_editor.is_some()
            || self.client_editor.is_some()
            || self.vendor_editor.is_some()
            || self.expense_editor.is_some()
            || self.delivery_note_editor.is_some()
            || self.category_editor.is_some()
            || self.unit_editor.is_some()
            || self.stock_dialog.is_some()
            || self.quick_sale.is_some()
            || self.key_editor.is_some()
    }

    /// Escape handling, topmost surface first. Returns true when something
    /// was dismissed. Dialogs cancel without persisting; the dirty product
    /// wizard routes through its existing discard confirmation, every other
    /// editor closes directly like its own Cancel button.
    pub fn dismiss_top_overlay(&mut self) -> bool {
        if self.confirm_dialog.take().is_some() {
            return true;
        }
        if self.palette_open {
            self.close_palette();
            return true;
        }
        if self.profile_menu_open {
            self.profile_menu_open = false;
            return true;
        }
        // The stock dialog paints above the product editor, so Escape must
        // dismiss it first when both are open and leave the editor alone.
        if self.stock_dialog.take().is_some() {
            return true;
        }
        if let Some(editor) = self.product_editor.take() {
            if editor.is_dirty() {
                self.product_editor = Some(editor);
                self.confirm_dialog = Some(crate::dialogs::Confirm {
                    title: crate::t!(self.locale, "Discard changes?").into_owned(),
                    body: crate::t!(self.locale, "The product information has not been saved.")
                        .into_owned(),
                    confirm_label: crate::t!(self.locale, "Discard").into_owned(),
                    danger: true,
                    action: crate::dialogs::ConfirmAction::DiscardEditor,
                });
            }
            return true;
        }
        if self.client_editor.take().is_some()
            || self.vendor_editor.take().is_some()
            || self.expense_editor.take().is_some()
            || self.delivery_note_editor.take().is_some()
            || self.category_editor.take().is_some()
            || self.unit_editor.take().is_some()
            || self.quick_sale.take().is_some()
            || self.key_editor.take().is_some()
        {
            return true;
        }
        false
    }

    /// Executes a command-palette entry. Returns true when the title-bar
    /// search field should be focused (the only entry needing the context).
    pub fn run_palette_target(&mut self, target: crate::shortcuts::PaletteTarget) -> bool {
        use crate::shortcuts::PaletteTarget;
        self.close_palette();
        match target {
            PaletteTarget::Route(route) => {
                self.go(route);
                self.ensure_route_data(route);
                false
            }
            PaletteTarget::Refresh => {
                self.refresh_current_route();
                false
            }
            PaletteTarget::NewProduct => {
                self.go(Route::Products);
                self.product_editor = Some(ProductEditor::new());
                false
            }
            PaletteTarget::NewClient => {
                self.go(Route::Clients);
                self.client_editor = Some(ClientEditor::new());
                false
            }
            PaletteTarget::NewVendor => {
                self.go(Route::Vendors);
                self.vendor_editor = Some(VendorEditor::new());
                false
            }
            PaletteTarget::NewExpense => {
                self.go(Route::Expenses);
                self.expense_editor = Some(ExpenseEditor::new());
                false
            }
            PaletteTarget::NewDelivery => {
                self.go(Route::DeliveryNotes);
                self.delivery_note_editor = Some(DeliveryNoteEditor::new());
                false
            }
            PaletteTarget::FocusSearch => true,
            PaletteTarget::SettingsProfile => {
                self.settings_focus = SettingsFocus::Profile;
                self.go(Route::Settings);
                false
            }
            PaletteTarget::SettingsGeneral => {
                self.settings_focus = SettingsFocus::General;
                self.go(Route::Settings);
                false
            }
            PaletteTarget::SignOut => {
                self.confirm_dialog = Some(crate::dialogs::Confirm {
                    title: crate::t!(self.locale, "Sign out").into_owned(),
                    body: crate::t!(
                        self.locale,
                        "You will need to sign in again to reach this server."
                    )
                    .into_owned(),
                    confirm_label: crate::t!(self.locale, "Sign out").into_owned(),
                    danger: true,
                    action: crate::dialogs::ConfirmAction::SignOut,
                });
                false
            }
        }
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
        self.clients = Cache::default();
        self.client_search = String::new();
        self.selected_client = None;
        self.client_editor = None;
        self.vendors = Cache::default();
        self.vendor_search = String::new();
        self.selected_vendor = None;
        self.vendor_editor = None;
        self.expenses = Cache::default();
        self.expense_search = String::new();
        self.expense_editor = None;
        self.delivery_notes = Cache::default();
        self.delivery_note_editor = None;
        self.scan = ScanState::default();
        self.categories = Cache::default();
        self.units = Cache::default();
        self.category_editor = None;
        self.unit_editor = None;
        self.invoices = Cache::default();
        self.alerts = Cache::default();
        self.nav_badges = Cache::default();
        // A stale 401 on the devices cache would otherwise sign out the next
        // session on its first frame: every session-scoped cache resets here.
        self.devices = Cache::default();
        self.profile = Cache::default();
        self.profile_form = ProfileForm::default();
        self.profile_save_job = None;
        self.api_keys = Cache::default();
        self.api_keys_revoke_pending = false;
        self.key_editor = None;
        self.created_key = None;
        self.key_create_job = None;
        self.report_job = None;
        self.assist = Cache::default();
        self.nav_back.clear();
        self.nav_forward.clear();
        self.profile_menu_open = false;
        self.settings_focus = SettingsFocus::General;
        self.agent = AgentState::default();
        self.screen = Screen::Login;
    }

    pub fn notice(&mut self, message: impl Into<String>) {
        self.notices.push(message.into());
        if self.notices.len() > 4 {
            self.notices.remove(0);
        }
        // Every push (including replacements when the stack is capped)
        // restarts the bounded visibility window.
        self.notices_expire_at = Some(Instant::now() + NOTICE_TTL);
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
