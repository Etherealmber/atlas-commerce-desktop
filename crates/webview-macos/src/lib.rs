//! Narrow WKWebView navigation boundary for Atlas browser authentication.
//! The rest of the application forbids unsafe code; Objective-C delegate
//! registration is isolated here and must happen on the main thread.
#![cfg(target_os = "macos")]

use atlas_core::webview_policy::{Frame, NavigationPolicy};
use objc2::{
    DeclaredClass, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{NSObject, ProtocolObject},
};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol};
use objc2_web_kit::{
    WKFrameInfo, WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate,
    WKNavigationResponse, WKNavigationResponsePolicy, WKWebView,
};
use wry::{WebView, WebViewBuilder, WebViewExtMacOS};

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = NavigationPolicy]
    struct LoginNavigationDelegate;

    unsafe impl NSObjectProtocol for LoginNavigationDelegate {}
    unsafe impl WKNavigationDelegate for LoginNavigationDelegate {
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        fn decide_action(
            &self,
            _webview: &WKWebView,
            action: &WKNavigationAction,
            handler: &block2::Block<dyn Fn(WKNavigationActionPolicy)>,
        ) {
            // The target frame is supplied by WebKit, not inferred from a URL.
            // Missing targets include popup requests and are rejected.
            let allowed = unsafe {
                let frame: Option<Retained<WKFrameInfo>> = msg_send![action, targetFrame];
                let Some(frame) = frame else {
                    handler.call((WKNavigationActionPolicy::Cancel,));
                    return;
                };
                let is_main: bool = msg_send![&*frame, isMainFrame];
                let request = action.request();
                let uri = request.URL().and_then(|url| url.absoluteString());
                uri.is_some_and(|uri| {
                    self.ivars().allows(
                        if is_main {
                            Frame::Main
                        } else {
                            Frame::Subframe
                        },
                        &uri.to_string(),
                    )
                })
            };
            handler.call((if allowed {
                WKNavigationActionPolicy::Allow
            } else {
                WKNavigationActionPolicy::Cancel
            },));
        }
        #[unsafe(method(webView:decidePolicyForNavigationResponse:decisionHandler:))]
        fn decide_response(
            &self,
            _webview: &WKWebView,
            response: &WKNavigationResponse,
            handler: &block2::Block<dyn Fn(WKNavigationResponsePolicy)>,
        ) {
            let allowed = unsafe { response.canShowMIMEType() };
            handler.call((if allowed {
                WKNavigationResponsePolicy::Allow
            } else {
                WKNavigationResponsePolicy::Cancel
            },));
        }
    }
);

/// Child WebView and retained navigation delegate, both bound to the main thread.
pub struct LoginView {
    view: WebView,
    delegate: Retained<LoginNavigationDelegate>,
}

impl LoginView {
    /// Open an ephemeral WebKit context, after installing the frame-aware policy.
    pub fn open(
        parent: &impl wry::raw_window_handle::HasWindowHandle,
        size: (u32, u32, f64),
        url: &url::Url,
        server: &str,
    ) -> Result<Self, &'static str> {
        let policy = NavigationPolicy::new(server).ok_or("Serveur HTTPS invalide.")?;
        if !policy.allows(Frame::Main, url.as_str()) {
            return Err("Adresse d’autorisation invalide.");
        }
        let mtm =
            MainThreadMarker::new().ok_or("WebView indisponible hors du thread principal.")?;
        let view = WebViewBuilder::new()
            .with_incognito(true)
            .with_devtools(false)
            .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
            .with_download_started_handler(|_, _| false)
            .with_permission_handler(|_| wry::PermissionResponse::Deny)
            .with_bounds(bounds(size.0, size.1, size.2))
            .build_as_child(parent)
            .map_err(|_| "WKWebView indisponible. Utilisez le navigateur système.")?;
        let delegate = mtm.alloc::<LoginNavigationDelegate>().set_ivars(policy);
        // WKWebView keeps its delegate weakly, so LoginView retains it until
        // the child is destroyed. Install before issuing any remote request.
        let delegate: Retained<LoginNavigationDelegate> =
            unsafe { msg_send![super(delegate), init] };
        unsafe {
            view.webview()
                .setNavigationDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        }
        view.load_url(url.as_str())
            .map_err(|_| "Impossible d’ouvrir la connexion intégrée.")?;
        Ok(Self { view, delegate })
    }
    /// Keep the remote document below the native server/code/cancel header.
    pub fn resize(&self, width: u32, height: u32, scale: f64) {
        let _ = self.view.set_bounds(bounds(width, height, scale));
    }
}
impl Drop for LoginView {
    fn drop(&mut self) {
        // Clear the weak delegate before releasing our strong reference.
        unsafe {
            self.view.webview().setNavigationDelegate(None);
        }
        let _ = &self.delegate;
    }
}
fn bounds(width: u32, height: u32, scale: f64) -> wry::Rect {
    let header = ((168.0 * scale).round() as u32).min(height);
    wry::Rect {
        position: wry::dpi::PhysicalPosition::new(0, header as i32).into(),
        size: wry::dpi::PhysicalSize::new(width, height.saturating_sub(header)).into(),
    }
}
