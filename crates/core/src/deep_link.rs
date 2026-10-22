//! Parsers for links handed to the app through the registered
//! `atlascommerce://` protocol (or a second launch on the command line).
//!
//! Two shapes exist, kept strictly separate as on Android:
//!
//! - [`VerifyLink`]: the one-tap e-mail verification link
//!   `atlascommerce://verify?user&code&sig&exp`. Only complete, signed links
//!   are accepted; the worker re-checks the signature, so this parser only
//!   guarantees that every field the `verify-link` endpoint needs is present.
//! - [`QrTokenParser`]: phone/PC pairing links and raw QR session tokens,
//!   distinguished from numeric product barcodes.

use serde::{Deserialize, Serialize};
use url::Url;

/// A complete one-tap verification link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyLink {
    pub username: String,
    pub otp: String,
    pub sig: String,
    pub exp: u64,
}

impl VerifyLink {
    /// Parses an `atlascommerce://verify?user=…&code=…&sig=…&exp=…` link.
    ///
    /// Accepts the custom scheme and, for the browser fallbacks the web app
    /// mails (`https://stock.afmarbre.com/verify?...`), any HTTP(S) URL whose
    /// path ends in `verify`. Anything missing a field is rejected, matching
    /// the worker's `LINK_MALFORMED` answer.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        let parsed = Url::parse(trimmed).ok()?;
        let is_scheme = parsed.scheme() == "atlascommerce";
        let is_https_verify = matches!(parsed.scheme(), "http" | "https")
            && parsed.path_segments()?.any(|segment| segment == "verify");
        if !is_scheme && !is_https_verify {
            return None;
        }
        if is_scheme
            && parsed
                .host_str()
                .is_some_and(|host| host != "verify" && !parsed.path().contains("verify"))
        {
            return None;
        }
        let query = parsed.query_pairs();
        let mut username = String::new();
        let mut otp = String::new();
        let mut sig = String::new();
        let mut exp: Option<u64> = None;
        for (key, value) in query {
            match key.as_ref() {
                "user" => username = value.to_string(),
                "code" | "otp" => otp = value.to_string(),
                "sig" => sig = value.to_string(),
                "exp" => exp = value.parse().ok(),
                _ => {}
            }
        }
        if username.trim().is_empty()
            || otp.trim().len() < 4
            || sig.trim().is_empty()
            || exp.is_none_or(|value| value == 0)
        {
            return None;
        }
        Some(Self {
            username: username.trim().to_lowercase(),
            otp: otp.trim().to_owned(),
            sig: sig.trim().to_owned(),
            exp: exp?,
        })
    }

    /// Whether the link's expiry timestamp has already passed (milliseconds).
    #[must_use]
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms > self.exp
    }
}

/// Distinguishes Atlas QR pairing links/tokens from product barcodes.
pub struct QrTokenParser;

impl QrTokenParser {
    /// Extracts a QR session token from:
    /// - `atlascommerce://qr?t=…` (and `token` / `session` parameter names),
    /// - `https://host/login/qr?t=…`, `/login-qr?token=…`,
    /// - `https://host/api/auth/qr/session/<token>`,
    /// - a raw token of 18–128 URL-safe characters.
    ///
    /// Returns `None` for EAN/UPC numeric product barcodes or unrelated URLs.
    #[must_use]
    pub fn extract(raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        if let Some(rest) = trimmed
            .strip_prefix("atlascommerce://")
            .or_else(|| trimmed.strip_prefix("atlascommerce:"))
        {
            let after = rest.trim_start_matches('/');
            if let Some(query) = after.find('?').map(|index| &after[index + 1..])
                && let Some(token) = query_token(query)
            {
                return Some(token);
            }
            let tail = after.split('?').next().unwrap_or("").trim_end_matches('/');
            let last = tail.rsplit('/').next().unwrap_or("");
            if !last.is_empty() && !last.eq_ignore_ascii_case("qr") && is_raw_token(last) {
                return Some(last.to_owned());
            }
            return None;
        }

        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            let parsed = Url::parse(trimmed).ok()?;
            for key in ["t", "token", "session"] {
                if let Some(value) = parsed.query_pairs().find(|(k, _)| k == key) {
                    let token = value.1.trim();
                    if !token.is_empty() {
                        return Some(token.to_owned());
                    }
                }
            }
            let segments: Vec<&str> = parsed
                .path_segments()
                .map(|iter| iter.collect())
                .unwrap_or_default();
            if let Some(index) = segments.iter().position(|segment| *segment == "session")
                && let Some(token) = segments.get(index + 1)
                && is_raw_token(token)
            {
                return Some((*token).to_owned());
            }
            if let Some(index) = segments.iter().position(|segment| {
                segment.eq_ignore_ascii_case("qr") || segment.eq_ignore_ascii_case("login-qr")
            }) && let Some(token) = segments.get(index + 1)
                && is_raw_token(token)
            {
                return Some((*token).to_owned());
            }
            return None;
        }

        // Reject plain numeric EAN-8 / UPC-A / EAN-13 / ITF-14 barcodes.
        if !trimmed.is_empty() && trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        is_raw_token(trimmed).then(|| trimmed.to_owned())
    }
}

fn is_raw_token(value: &str) -> bool {
    (18..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn query_token(query: &str) -> Option<String> {
    for pair in query.split('&') {
        let mut split = pair.splitn(2, '=');
        let key = split.next()?.trim();
        let value = split.next()?.trim();
        if matches!(key, "t" | "token" | "session") && !value.is_empty() {
            return Some(value.to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINK: &str =
        "atlascommerce://verify?user=Alice&code=482913&sig=abc123def456&exp=1893456000000";

    #[test]
    fn parses_a_complete_signed_verify_link() {
        let link = VerifyLink::parse(LINK).expect("the link is complete");
        assert_eq!(link.username, "alice");
        assert_eq!(link.otp, "482913");
        assert_eq!(link.sig, "abc123def456");
        assert_eq!(link.exp, 1_893_456_000_000);
        assert!(!link.is_expired(1_000_000));
        assert!(link.is_expired(u64::MAX));
    }

    #[test]
    fn parses_https_verify_links() {
        let https =
            "https://stock.afmarbre.com/verify?user=bob&code=123456&sig=zz&exp=9999999999999";
        assert!(VerifyLink::parse(https).is_some());
    }

    #[test]
    fn rejects_incomplete_links() {
        assert!(VerifyLink::parse("atlascommerce://verify?user=alice&code=482913").is_none());
        assert!(
            VerifyLink::parse("atlascommerce://verify?user=&code=482913&sig=x&exp=1").is_none()
        );
        assert!(VerifyLink::parse("https://example.com/?user=a&code=482913&sig=x&exp=1").is_none());
        assert!(VerifyLink::parse("not a link").is_none());
        assert!(VerifyLink::parse("").is_none());
    }

    #[test]
    fn extracts_qr_tokens_from_every_supported_shape() {
        let token = "0123456789abcdef0123456789abcdef";
        assert_eq!(
            QrTokenParser::extract(&format!("atlascommerce://qr?t={token}")).as_deref(),
            Some(token)
        );
        assert_eq!(
            QrTokenParser::extract(&format!(
                "https://stock.afmarbre.com/login/qr?t={token}&mode=pc"
            ))
            .as_deref(),
            Some(token)
        );
        assert_eq!(
            QrTokenParser::extract(&format!(
                "https://stock.afmarbre.com/api/auth/qr/session/{token}"
            ))
            .as_deref(),
            Some(token)
        );
        assert_eq!(QrTokenParser::extract(token).as_deref(), Some(token));
    }

    #[test]
    fn rejects_product_barcodes_and_unrelated_links() {
        assert!(QrTokenParser::extract("6111234567892").is_none());
        assert!(QrTokenParser::extract("12345678").is_none());
        assert!(QrTokenParser::extract("short").is_none());
        assert!(QrTokenParser::extract("https://example.com/products").is_none());
    }
}
