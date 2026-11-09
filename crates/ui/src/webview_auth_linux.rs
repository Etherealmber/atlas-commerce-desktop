//! An ephemeral WebKitGTK window. No JavaScript injection or secret bridge.
//! A separate GTK window also works when the main winit window uses Wayland.
use atlas_core::webview_policy::{Frame, NavigationPolicy};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};
use webkit6::{glib, prelude::*};

pub(crate) struct LoginView {
    view: webkit6::WebView,
    window: gtk4::Window,
    display: gtk4::gdk::Display,
    closed: Rc<Cell<bool>>,
}

fn challenge_navigation(uri: &str) -> bool {
    if uri == "about:blank" {
        return true;
    }
    url::Url::parse(uri).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("challenges.cloudflare.com")
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none()
    })
}

impl LoginView {
    pub(crate) fn open(url: &url::Url, server: &str, code: &str) -> Result<Self, &'static str> {
        let policy = NavigationPolicy::new(server).ok_or("Serveur HTTPS invalide.")?;
        if !policy.allows(Frame::Main, url.as_str()) {
            return Err("Adresse d’autorisation invalide.");
        }
        gtk4::init().map_err(|_| "WebKitGTK indisponible. Utilisez le navigateur système.")?;
        let session = webkit6::NetworkSession::new_ephemeral();
        session.set_persistent_credential_storage_enabled(false);
        session.set_tls_errors_policy(webkit6::TLSErrorsPolicy::Fail);
        session.connect_download_started(|_, download| download.cancel());
        let settings = webkit6::Settings::new();
        settings.set_enable_developer_extras(false);
        settings.set_enable_write_console_messages_to_stdout(false);
        settings.set_allow_file_access_from_file_urls(false);
        settings.set_allow_universal_access_from_file_urls(false);
        settings.set_allow_top_navigation_to_data_urls(false);
        settings.set_javascript_can_open_windows_automatically(false);
        let view = webkit6::WebView::builder()
            .network_session(&session)
            .settings(&settings)
            .build();
        view.set_hexpand(true);
        view.set_vexpand(true);
        let policy = Rc::new(policy);
        view.connect_decide_policy(move |_, decision, kind| {
            let allowed = match kind {
                webkit6::PolicyDecisionType::NavigationAction => decision
                    .downcast_ref::<webkit6::NavigationPolicyDecision>()
                    .and_then(|d| d.navigation_action())
                    .and_then(|a| a.request())
                    .and_then(|r| r.uri())
                    .is_some_and(|uri| {
                        policy.allows(Frame::Main, &uri) || challenge_navigation(&uri)
                    }),
                webkit6::PolicyDecisionType::Response => decision
                    .downcast_ref::<webkit6::ResponsePolicyDecision>()
                    .is_some_and(|response| {
                        response.is_mime_type_supported()
                            && (!response.is_main_frame_main_resource()
                                || response
                                    .request()
                                    .and_then(|request| request.uri())
                                    .is_some_and(|uri| policy.allows(Frame::Main, &uri)))
                    }),
                _ => false,
            };
            if allowed {
                decision.use_();
            } else {
                decision.ignore();
            }
            true
        });
        view.connect_create(|_, _| None);
        view.connect_permission_request(|_, request| {
            request.deny();
            true
        });
        view.connect_query_permission_state(|_, query| {
            query.finish(webkit6::PermissionState::Denied);
            true
        });
        view.connect_run_file_chooser(|_, request| {
            request.cancel();
            true
        });
        view.connect_authenticate(|_, request| {
            request.cancel();
            true
        });
        view.connect_enter_fullscreen(|_| true);
        view.connect_print(|_, _| true);
        let column = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        let heading = gtk4::Label::new(Some(&format!(
            "Serveur : {}   •   Comparez le code : {}",
            atlas_core::ServerUrl::host(server),
            code
        )));
        heading.set_margin_top(12);
        heading.set_margin_bottom(4);
        column.append(&heading);
        column.append(&view);
        let window = gtk4::Window::builder()
            .title("Connexion · Atlas Commerce")
            .default_width(900)
            .default_height(700)
            .child(&column)
            .build();
        let closed = Rc::new(Cell::new(false));
        let close_flag = closed.clone();
        window.connect_close_request(move |_| {
            close_flag.set(true);
            glib::Propagation::Proceed
        });
        let crashed = closed.clone();
        view.connect_web_process_terminated(move |_, _| crashed.set(true));
        let display = gtk4::prelude::WidgetExt::display(&window);
        view.load_uri(url.as_str());
        window.present();
        Ok(Self {
            view,
            window,
            display,
            closed,
        })
    }

    pub(crate) fn pump(&self) -> bool {
        let context = glib::MainContext::default();
        let started = Instant::now();
        for _ in 0..16 {
            if started.elapsed() >= Duration::from_millis(2) || !context.pending() {
                break;
            }
            context.iteration(false);
        }
        self.display.flush();
        !self.closed.get()
    }
}
impl Drop for LoginView {
    fn drop(&mut self) {
        self.view.stop_loading();
        self.view.terminate_web_process();
        self.window.destroy();
        self.display.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::challenge_navigation;
    // Run only under an isolated virtual display. No credentials or real server.
    #[test]
    #[ignore = "requires a display and WebKitGTK runtime"]
    fn opens_and_destroys_an_ephemeral_window() {
        let url = url::Url::parse(
            "https://example.invalid/login/desktop#request_id=fixture&state=fixture",
        )
        .unwrap();
        let view = super::LoginView::open(&url, "https://example.invalid", "fixture").unwrap();
        for _ in 0..10 {
            assert!(view.pump());
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        drop(view);
    }

    #[test]
    fn challenge_is_limited_to_cloudflare_https() {
        assert!(challenge_navigation(
            "https://challenges.cloudflare.com/turnstile/v0/a"
        ));
        assert!(challenge_navigation("about:blank"));
        for value in [
            "http://challenges.cloudflare.com/",
            "https://challenges.cloudflare.com:444/",
            "https://challenges.cloudflare.com.evil.test/",
            "https://user@challenges.cloudflare.com/",
            "data:text/html,a",
        ] {
            assert!(!challenge_navigation(value));
        }
    }
}
