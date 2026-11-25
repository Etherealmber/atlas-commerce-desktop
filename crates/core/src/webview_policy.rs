//! Navigation policy for a future embedded sign-in host.
//!
//! A host must obtain the frame type from the engine itself. An adapter that
//! cannot distinguish top-level navigations from subframes must not use this
//! policy as if every navigation were a top-level navigation: doing so would
//! either block the Turnstile iframe or permit a top-level origin escape.

use url::Url;

/// Height reserved for the window controls and secure pairing chrome above
/// the embedded login on Windows and macOS, in logical points.
pub const LOGIN_CHROME_HEIGHT_POINTS: f64 = 96.0;

/// Convert the reserved chrome height to physical pixels, bounded by the
/// actual window so tiny or high-DPI windows never produce negative bounds.
#[must_use]
pub fn login_chrome_height(height: u32, scale: f64) -> u32 {
    ((LOGIN_CHROME_HEIGHT_POINTS * scale).round() as u32).min(height)
}

/// The engine-reported frame being navigated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    /// The main document shown to the user.
    Main,
    /// A nested frame such as the Turnstile challenge.
    Subframe,
    /// The engine did not identify the frame.
    Unknown,
}

/// A policy bound to one validated HTTPS server origin.
pub struct NavigationPolicy {
    origin: url::Origin,
}

impl NavigationPolicy {
    /// Constructs a policy only for a bare HTTPS server origin.
    #[must_use]
    pub fn new(server: &str) -> Option<Self> {
        let url = Url::parse(server).ok()?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return None;
        }
        Some(Self {
            origin: url.origin(),
        })
    }

    /// Allows HTTPS on the selected origin in the main frame, or HTTPS
    /// subframes (and their initial `about:blank`) for web challenges.
    /// Unknown frames, credentials, insecure schemes and external top-level
    /// navigations are refused. This does not grant popups or downloads.
    #[must_use]
    pub fn allows(&self, frame: Frame, target: &str) -> bool {
        if frame == Frame::Unknown {
            return false;
        }
        if frame == Frame::Subframe && target == "about:blank" {
            return true;
        }
        let Ok(url) = Url::parse(target) else {
            return false;
        };
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return false;
        }
        frame == Frame::Subframe || url.origin() == self.origin
    }
}

fn challenge_url_has_safe_authority(raw: &str) -> bool {
    if raw.contains('\\') {
        return false;
    }
    let Some((_, rest)) = raw.split_once("://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    !authority.contains('@')
}

/// Navigation rules for a challenge-only WebView. This policy is not sufficient
/// by itself: the host must install separate main-frame and child-frame
/// handlers before navigation and cancel on navigation failure.
pub struct EmbeddedChallengePolicy {
    origin: url::Origin,
    initial_fragment: String,
    path: &'static str,
}

impl EmbeddedChallengePolicy {
    /// Bind the policy to the selected origin and the server-issued request ID.
    #[must_use]
    pub fn new(server: &str, request_id: &str) -> Option<Self> {
        Self::for_path(server, request_id, "/login/desktop-challenge-embedded")
    }

    /// Bind to the v3 challenge document, never the full hosted login.
    #[must_use]
    pub fn v3(server: &str, request_id: &str) -> Option<Self> {
        Self::for_path(server, request_id, "/login/desktop-v3-challenge")
    }

    fn for_path(server: &str, request_id: &str, path: &'static str) -> Option<Self> {
        if !challenge_url_has_safe_authority(server) {
            return None;
        }
        if request_id.len() != 64
            || !request_id
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return None;
        }
        Some(Self {
            origin: NavigationPolicy::new(server)?.origin,
            initial_fragment: format!("request_id={request_id}"),
            path,
        })
    }

    /// Reject unknown frames; allow only the exact embedded document in the
    /// main frame and the provider's HTTPS origin (or initial blank) in a child.
    #[must_use]
    pub fn allows(&self, frame: Frame, target: &str) -> bool {
        if frame == Frame::Subframe && target == "about:blank" {
            return true;
        }
        if !challenge_url_has_safe_authority(target) {
            return false;
        }
        let Ok(url) = Url::parse(target) else {
            return false;
        };
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return false;
        }
        match frame {
            Frame::Main => {
                url.origin() == self.origin
                    && url.path() == self.path
                    && url.query().is_none()
                    && (url.fragment().is_none()
                        || url.fragment() == Some(self.initial_fragment.as_str()))
            }
            Frame::Subframe => {
                url.host_str() == Some("challenges.cloudflare.com")
                    && url.port_or_known_default() == Some(443)
            }
            Frame::Unknown => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_chrome_scales_and_stays_inside_small_windows() {
        assert_eq!(login_chrome_height(800, 1.0), 96);
        assert_eq!(login_chrome_height(800, 2.0), 192);
        assert_eq!(login_chrome_height(100, 2.0), 100);
        assert_eq!(login_chrome_height(0, 1.0), 0);
    }

    #[test]
    fn main_frame_is_bound_to_exact_https_origin_and_port() {
        let policy = NavigationPolicy::new("https://shop.example:8443").unwrap();
        assert!(policy.allows(
            Frame::Main,
            "https://shop.example:8443/login/desktop#state=example"
        ));
        for url in [
            "https://shop.example/login",
            "https://shop.example:8444/login",
            "https://other.example:8443/login",
            "http://shop.example:8443/login",
            "https://user@shop.example:8443/login",
            "javascript:alert(1)",
            "about:blank",
        ] {
            assert!(!policy.allows(Frame::Main, url), "{url}");
        }
    }

    #[test]
    fn challenge_iframe_does_not_open_top_level_navigation() {
        let policy = NavigationPolicy::new("https://shop.example").unwrap();
        let challenge = "https://challenges.cloudflare.com/turnstile/example";
        assert!(policy.allows(Frame::Subframe, challenge));
        assert!(policy.allows(Frame::Subframe, "about:blank"));
        assert!(!policy.allows(Frame::Main, challenge));
        assert!(!policy.allows(Frame::Unknown, challenge));
        assert!(!policy.allows(Frame::Subframe, "http://challenges.cloudflare.com/"));
    }

    #[test]
    fn embedded_challenge_is_limited_to_the_selected_document_and_provider_frame() {
        let id = "a".repeat(64);
        let policy = EmbeddedChallengePolicy::new("https://shop.example:8443", &id).unwrap();
        let page = "https://shop.example:8443/login/desktop-challenge-embedded";
        assert!(policy.allows(Frame::Main, page));
        assert!(policy.allows(Frame::Main, &format!("{page}#request_id={id}")));
        for target in [
            "https://shop.example/login/desktop-challenge-embedded",
            "https://shop.example:8443/login",
            "https://shop.example:8443/login/desktop-challenge-embedded?x=1",
            "https://shop.example:8443/login/desktop-challenge-embedded#request_id=bad",
            "https://challenges.cloudflare.com/turnstile/",
        ] {
            assert!(!policy.allows(Frame::Main, target), "{target}");
        }
        assert!(policy.allows(
            Frame::Subframe,
            "https://challenges.cloudflare.com/turnstile/"
        ));
        assert!(policy.allows(Frame::Subframe, "about:blank"));
        assert!(!policy.allows(Frame::Subframe, "https://challenges.cloudflare.com:8443/"));
        assert!(!policy.allows(Frame::Subframe, "https://other.example/"));
        assert!(!policy.allows(Frame::Unknown, page));
        assert!(EmbeddedChallengePolicy::new("https://shop.example", "INVALID").is_none());
    }

    #[test]
    fn v3_challenge_does_not_allow_the_v2_or_browser_page() {
        let id = "b".repeat(64);
        let policy = EmbeddedChallengePolicy::v3("https://shop.example:8443", &id).unwrap();
        let page = "https://shop.example:8443/login/desktop-v3-challenge";
        assert!(policy.allows(Frame::Main, page));
        assert!(policy.allows(Frame::Main, &format!("{page}#request_id={id}")));
        for target in [
            "https://shop.example:8443/login/desktop-v3-browser",
            "https://shop.example:8443/login/desktop-challenge-embedded",
            "https://shop.example:8443/login/desktop-v3-challenge?request_id=ignored",
            "https://@shop.example:8443/login/desktop-v3-challenge",
            "https://shop.example:8443\\@evil.example/login/desktop-v3-challenge",
            "https://shop.example/login/desktop-v3-challenge",
        ] {
            assert!(!policy.allows(Frame::Main, target), "{target}");
        }
        assert!(!policy.allows(Frame::Unknown, page));
    }

    #[test]
    fn rejects_invalid_server_origins() {
        for server in [
            "http://shop.example",
            "https://shop.example/login",
            "https://shop.example/?x=1",
            "https://user@shop.example",
        ] {
            assert!(NavigationPolicy::new(server).is_none(), "{server}");
        }
    }
}
