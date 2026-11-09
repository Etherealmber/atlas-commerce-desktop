//! Navigation policy for a future embedded sign-in host.
//!
//! A host must obtain the frame type from the engine itself. An adapter that
//! cannot distinguish top-level navigations from subframes must not use this
//! policy as if every navigation were a top-level navigation: doing so would
//! either block the Turnstile iframe or permit a top-level origin escape.

use url::Url;

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

#[cfg(test)]
mod tests {
    use super::*;

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
