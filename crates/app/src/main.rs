//! Process entry point.
//!
//! Order of operations, following the fastframe app pattern:
//! 1. the self-update helper intercept (`--apply-update`) runs and exits on
//!    its own, before any lock or log file is touched;
//! 2. a second launch hands its `atlascommerce://` link to the running
//!    instance through the single-instance slot and exits;
//! 3. logging, folders and persisted state come up;
//! 4. the fastframe [`Shell`] owns the [`AtlasApp`] and recreates the
//!    eframe window whenever the tray asks for it after a hide-to-tray.

// Release builds on Windows are pure GUI programs: no console window opens
// alongside the app (debug builds keep it for logs). Same attribute as the
// Spotifast entry point.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};

use atlas_core::session::load_session;
use atlas_core::{
    APP_ID, APP_NAME, AccountStore, AppPaths, DeviceIdentity, JobRunner, SLUG, ServerStore,
    Settings,
};
use atlas_ui::tray::TrayHandle;
use atlas_ui::{AtlasApp, EframeShell};
use clap::Parser;
use fastframe_autostart::{Autostart, State as AutostartState};
use fastframe_instance::{Claim, Slot};
use fastframe_notifications::App as NotificationApp;
use fastframe_shell::{Shell, Waker};
use fastframe_update::Launch;

/// Command line. The OS protocol handler launches the binary with the
/// `atlascommerce://…` URL as its sole positional argument.
#[derive(Debug, Parser)]
#[command(name = "atlas-commerce-desktop", version, about)]
struct Cli {
    /// A verify-link or QR deep link (`atlascommerce://…` or an https URL).
    #[arg(value_name = "LINK")]
    link: Option<String>,

    /// Start without a window, in the notification area. Used by the
    /// login item; mirrors `fastframe_autostart::START_HIDDEN`.
    #[arg(long = "start-hidden")]
    start_hidden: bool,

    /// Log more detail.
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> eframe::Result<()> {
    // 1. Update helper first: it may install a downloaded release and exit.
    let launch: Launch = fastframe_update::intercept(&atlas_ui::update::CONFIG);
    let cli = Cli::parse_from(&launch.arguments);
    let fastframe_update::Launch {
        arguments: _,
        receipt,
        error: launch_error,
    } = launch;

    // 2. Single instance. The running copy serves this slot on its own
    //    thread; a second launch only hands a request over and leaves. The
    //    waker must exist before the claim: the slot handler runs on another
    //    thread and has to wake a hidden instance out of its headless wait.
    let incoming = Arc::new(Mutex::new(Vec::<String>::new()));
    let queue = Arc::clone(&incoming);
    let waker = Waker::default();
    let handler_waker = waker.clone();
    let slot = Slot::new(APP_ID);
    let request = match cli.link.as_deref() {
        Some(link) => format!("open-link:{link}"),
        None => "show".to_owned(),
    };
    let _guard = match slot.claim(&request, move |request| {
        if let Some(link) = request.strip_prefix("open-link:")
            && let Ok(mut queue) = queue.lock()
        {
            queue.push(link.to_owned());
        } else if request == "show"
            && let Ok(mut queue) = queue.lock()
        {
            // Plain second launch: ask the running instance to unhide.
            queue.push(atlas_ui::SHOW_REQUEST.to_owned());
        }
        // Always wake the running instance; it decides what the request means.
        handler_waker.wake();
        Some("ok".to_owned())
    }) {
        Claim::First(guard) => guard,
        Claim::Running(_) | Claim::Declined | Claim::Unanswered => {
            log::info!("Atlas Commerce Desktop is already running");
            return Ok(());
        }
    };

    // 3. Folders and logging.
    let paths = AppPaths::discover();
    if let Err(error) = paths.ensure() {
        eprintln!("unable to create application folders: {error}");
    }
    let filter = if cli.verbose {
        "debug,atlas=debug,reqwest=info"
    } else {
        "warn,atlas=info,fastframe_fonts=info"
    };
    if let Err(error) = fastframe_log::Logging::new(SLUG, env!("CARGO_PKG_VERSION"))
        .filter(filter)
        .file(paths.log_file())
        .panic_log(paths.panic_log())
        .panic_message(fastframe_log::PanicMessage::Redacted(
            fastframe_log::redact::links,
        ))
        .init()
    {
        eprintln!("logging unavailable: {error}");
    }
    log::info!(
        "starting {} v{} on {}-{}",
        APP_NAME,
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    // 4. Persisted state. No password is ever read here — only the token.
    let settings = Settings::load(&paths.settings_file());
    let locale = atlas_ui::i18n::Locale::from_tag(&settings.language);
    let servers = ServerStore::load(&paths.servers_file());
    let identity = DeviceIdentity::load_or_create(&paths.device_file());
    let accounts = AccountStore::load(&paths.accounts_file());
    let session = servers
        .selected_url()
        .and_then(|base| load_session(&paths.session_file(), base));

    // 5. Process-wide services. The waker was created before the single-
    //    instance claim so the slot handler shares this exact instance.
    let runner = JobRunner::new();
    // Kept for the shell: the app receives one clone (moved in below), and
    // every clone shares the same context slot and headless alarm.
    let shell_waker = waker.clone();
    let power = fastframe_power::Power::spawn({
        let waker = waker.clone();
        move || waker.wake()
    });
    // The handle moves into the app, which drains it on the UI thread; on
    // macOS the status item is created/attached by the shell lifecycle.
    let tray = TrayHandle::spawn(locale, {
        let waker = waker.clone();
        move || waker.wake()
    });
    // On Linux a panel may register the item a moment later, so the mere
    // existence of a tray host decides whether close may hide to it.
    let tray_available = tray.is_some();

    let notifications = NotificationApp::new(APP_ID, APP_NAME)
        .desktop_entry(SLUG)
        .icon(SLUG);
    let notifications = fastframe_notifications::Client::new(notifications).ok();

    // Registry / file-system call: off the thread that will become the UI.
    let autostart = Autostart::new(SLUG, APP_NAME).hidden_argument("--start-hidden");
    let autostart_state = std::thread::scope(|scope| scope.spawn(|| autostart.state()).join())
        .unwrap_or(AutostartState::Off);

    // A link given to the first instance travels the same queue as handoffs.
    if let Some(link) = cli.link.clone()
        && let Ok(mut queue) = incoming.lock()
    {
        queue.push(link);
    }

    // Existing sessions are deliberately not restored into an unlocked UI.
    // The guard survives a tray close or crash even if token cleanup is late.
    let require_password = session.is_some() || paths.auth_guard_file().exists();
    if require_password
        && let Err(error) = std::fs::write(paths.auth_guard_file(), b"1")
    {
        log::warn!("could not persist authentication guard: {error}");
    }
    let mut app = AtlasApp::new(
        paths, runner, identity, settings, servers, session, accounts, waker, power, incoming, tray,
    );
    if require_password {
        if app.session.is_some() {
            app.sign_out();
        }
        app.requires_password = true;
    }
    app.tray_available = tray_available;
    app.autostart_state = autostart_state;
    app.notifications = notifications;
    // The receipt lives on the app (Spotifast's pattern): it is taken once
    // by the first window's first frame, surviving hide-to-tray recreations.
    app.update_receipt = receipt;
    if let Some(message) = launch_error {
        app.notice(format!("Mise à jour : {message}"));
    }
    // Restore the authenticated client before the splash gate decides where
    // to go. A dead token is caught later by the session-binding guard.
    app.rebuild_client();

    // Custom title bar: init reads the platform button layout, then the
    // user's setting decides whether the OS frame is removed.
    fastframe_titlebar::init();
    let titlebar_enabled = app.settings.custom_titlebar;
    fastframe_titlebar::set_enabled(titlebar_enabled);

    let start_hidden = cli.start_hidden && cli.link.is_none() && tray_available;
    // Only macOS mutates the shell afterwards (its AppKit idle callback).
    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
    let mut shell = Shell::new(app, &shell_waker).start_hidden(start_hidden);
    // AppKit only runs its status-item menu inside its own event loop.
    #[cfg(target_os = "macos")]
    {
        shell = shell.idle(fastframe_tray::idle);
    }

    shell.run(move |lease| {
        let custom_titlebar = lease.peek(|app| app.settings.custom_titlebar);
        let options = native_options(custom_titlebar);
        eframe::run_native(
            APP_NAME,
            options,
            Box::new(move |cc| Ok(Box::new(EframeShell::new(lease, &cc.egui_ctx)))),
        )
    })
}

/// Builds the main window's viewport: dark-safe defaults, a minimum size for
/// the two-column shell, and no native frame when the app draws its own.
fn native_options(custom_titlebar: bool) -> eframe::NativeOptions {
    #[cfg(target_os = "linux")]
    let app_id = fastframe_desktop::desktop_entry(SLUG);
    #[cfg(not(target_os = "linux"))]
    let app_id = SLUG;

    let viewport = egui::ViewportBuilder::default()
        // Pre-login base title; the app promotes it to
        // "Atlas Commerce — <serveur>" once a server is selected.
        .with_title("Atlas Commerce")
        .with_app_id(app_id)
        // Default 1280×800 keeps the rail above its 1180 px collapse
        // breakpoint at first launch; 1024×680 is the parity floor.
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([1024.0, 680.0])
        .with_decorations(!custom_titlebar)
        // Born hidden: the OS would otherwise paint the default white
        // window background for several frames before the first egui
        // present (the "Windows 7" white flash). EframeShell flips this
        // back on once the first fully styled frame has been rendered.
        .with_visible(false);
    // Runtime window/taskbar icon (Linux window managers and any Windows
    // shell path that ignores the embedded resource). macOS takes its Dock
    // icon from the bundle, so setting one there would only flatten it.
    #[cfg(not(target_os = "macos"))]
    let viewport = viewport.with_icon(atlas_ui::tray::window_icon());
    // With a drawn title bar the content runs to the top edge; the shell
    // leaves room for the platform's caption buttons itself. Keep the native
    // frame untouched when the user disabled the custom title bar.
    let viewport = if custom_titlebar {
        viewport
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false)
    } else {
        viewport
    };

    eframe::NativeOptions {
        viewport,
        ..Default::default()
    }
}
