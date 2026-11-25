//! Client-side authentication protocol engine for Atlas Commerce Desktop v3.
//!
//! Architecture boundary:
//! - The Cloudflare Worker remains the single source of truth for accounts,
//!   authorization, Turnstile siteverify, OTP delivery, sessions, revocation,
//!   and abuse controls.
//! - `atlas-auth-core` handles client-side protocol state, strict HTTPS origin
//!   validation, PKCE S256 generation, RFC 9449 ES256 DPoP proof creation,
//!   capability/response validation, and secret-redacting errors.
//! - OS-specific WebView2 / WKWebView / WebKitGTK hosting, secure credential
//!   storage (`keyring`), and hardware/OS device keys live behind platform
//!   adapters in `atlas-commerce-desktop`. Software or keychain keys MUST NEVER
//!   claim hardware attestation.
//! - Completely independent of `rust/atlas-finance-core`.

pub mod dpop;
pub mod error;
pub mod origin;
pub mod pkce;
pub mod schemas;
pub mod state;

pub use dpop::{
    authorization_header_for_token, build_dpop_proof, build_protected_request_headers,
    jwk_thumbprint, DeviceAttestationLevel, DeviceKeyAdapter, SoftwareP256Key,
};
pub use error::{AuthCoreError, Result};
pub use origin::ServerOrigin;
pub use pkce::{validate_pairing_code, PkcePair};
pub use schemas::{
    CapabilitiesV3Response, ChallengePollRequestV3, ChallengePollResponse, ChallengeVerifyStatus,
    CreateRequestV3Body, CreateRequestV3Response, DesktopV3ApiEnvelope, LogoutRequestV3,
    LogoutResponseV3, NonceResponse, OtpLoginRequestV3, PasswordLoginRequestV3,
    PasswordLoginResponseV3, RefreshRequestV3, SessionTokensV3,
};
pub use state::{ChallengePresentation, DesktopV3AuthSession, DesktopV3Phase};
