use std::fmt;
use thiserror::Error;

/// Secret-safe protocol error type.
///
/// Neither `Display` nor `Debug` ever prints passwords, OTPs, PKCE verifiers,
/// state secrets, refresh/access tokens, or private key material.
#[derive(Clone, PartialEq, Eq, Error)]
pub enum AuthCoreError {
    #[error(
        "server origin must be a bare HTTPS origin without path, query, fragment, or userinfo"
    )]
    InvalidOrigin,

    #[error("invalid target URL for DPoP proof")]
    InvalidTargetUrl,

    #[error("invalid pairing code format")]
    InvalidPairingCode,

    #[error("invalid nonce format")]
    InvalidNonce,

    #[error("invalid device label")]
    InvalidDeviceLabel,

    #[error("invalid state transition from {from} during {action}")]
    InvalidTransition {
        from: &'static str,
        action: &'static str,
    },

    #[error("server response origin or request_id mismatch")]
    ResponseMismatch,

    #[error("desktop v3 authentication is disabled or not ready on this server")]
    NotReady,

    #[error("automatic silent fallback between Turnstile WebView and system browser is forbidden")]
    SilentFallbackForbidden,

    #[error("cryptographic operation failed")]
    CryptoFailure,
}

impl fmt::Debug for AuthCoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AuthCoreError({})", self)
    }
}

pub type Result<T> = std::result::Result<T, AuthCoreError>;
