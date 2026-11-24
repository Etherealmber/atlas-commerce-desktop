use crate::state::ChallengePresentation;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Clone, Debug, Deserialize)]
pub struct DesktopV3ApiEnvelope<T> {
    pub success: bool,
    pub data: Option<T>,
    pub code: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CapabilitiesV3Response {
    pub version: u8,
    pub enabled: bool,
    pub reason: Option<String>,
    pub methods: CapabilitiesV3Methods,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CapabilitiesV3Methods {
    pub password_otp: PasswordOtpMethodV3,
    pub mailbox_enrollment: SimpleCapabilityV3,
    pub recovery: SimpleCapabilityV3,
    pub device_proof: DeviceProofCapabilityV3,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PasswordOtpMethodV3 {
    pub available: bool,
    #[serde(default)]
    pub challenge_modes: Vec<ChallengePresentation>,
    #[serde(default)]
    pub automatic_fallback: bool,
    #[serde(default)]
    pub otp_required: bool,
    #[serde(default)]
    pub dpop_required: bool,
    #[serde(default)]
    pub verified_mailbox_required: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SimpleCapabilityV3 {
    pub available: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DeviceProofCapabilityV3 {
    pub available: bool,
    #[serde(default)]
    pub hardware_attestation_claimed: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct NonceResponse {
    pub nonce: String,
    pub expires_in: u64,
}

#[derive(Clone, Serialize)]
pub struct CreateRequestV3Body<'a> {
    pub origin: &'a str,
    pub state: &'a str,
    pub code_challenge: &'a str,
    pub code_challenge_method: &'static str,
    pub device_label: &'a str,
    pub presentation: ChallengePresentation,
}

impl std::fmt::Debug for CreateRequestV3Body<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateRequestV3Body")
            .field("origin", &self.origin)
            .field("state", &"[REDACTED]")
            .field("code_challenge", &self.code_challenge)
            .field("code_challenge_method", &self.code_challenge_method)
            .field("device_label", &self.device_label)
            .field("presentation", &self.presentation)
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateRequestV3Response {
    pub version: u8,
    pub request_id: String,
    pub origin: String,
    pub presentation: ChallengePresentation,
    pub challenge_url: String,
    pub pairing_code: String,
    pub device_thumbprint: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChallengePollResponse {
    pub status: ChallengeVerifyStatus,
    pub request_id: String,
    pub origin: String,
    pub interval: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeVerifyStatus {
    Pending,
    Verifying,
    Verified,
    Denied,
    Failed,
    Cancelled,
    Consumed,
}

#[derive(Clone, Serialize)]
pub struct ChallengePollRequestV3 {
    pub origin: String,
    pub request_id: String,
    pub state: String,
    pub code_verifier: String,
}

impl Zeroize for ChallengePollRequestV3 {
    fn zeroize(&mut self) {
        self.state.zeroize();
        self.code_verifier.zeroize();
    }
}

impl Drop for ChallengePollRequestV3 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for ChallengePollRequestV3 {}

#[derive(Clone, Serialize)]
pub struct PasswordLoginRequestV3 {
    pub origin: String,
    pub request_id: String,
    pub state: String,
    pub code_verifier: String,
    pub username: String,
    pub password: String,
}

impl Zeroize for PasswordLoginRequestV3 {
    fn zeroize(&mut self) {
        self.state.zeroize();
        self.code_verifier.zeroize();
        self.password.zeroize();
    }
}

impl Drop for PasswordLoginRequestV3 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for PasswordLoginRequestV3 {}

#[derive(Clone, Serialize)]
pub struct OtpLoginRequestV3 {
    pub origin: String,
    pub request_id: String,
    pub state: String,
    pub code_verifier: String,
    pub otp: String,
}

impl Zeroize for OtpLoginRequestV3 {
    fn zeroize(&mut self) {
        self.state.zeroize();
        self.code_verifier.zeroize();
        self.otp.zeroize();
    }
}

impl Drop for OtpLoginRequestV3 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for OtpLoginRequestV3 {}

#[derive(Clone, Deserialize)]
pub struct SessionTokensV3 {
    pub token_type: String,
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: String,
    pub session_expires_at: String,
    pub origin: String,
    pub username: String,
    pub role: String,
    pub session_id: String,
    pub device_thumbprint: String,
    pub attestation: Option<String>,
}

impl Zeroize for SessionTokensV3 {
    fn zeroize(&mut self) {
        self.access_token.zeroize();
        self.refresh_token.zeroize();
    }
}

impl Drop for SessionTokensV3 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for SessionTokensV3 {}

impl std::fmt::Debug for SessionTokensV3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionTokensV3")
            .field("token_type", &self.token_type)
            .field("username", &self.username)
            .field("role", &self.role)
            .field("session_id", &self.session_id)
            .field("device_thumbprint", &self.device_thumbprint)
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct PasswordLoginResponseV3 {
    pub status: String,
}

#[derive(Clone, Serialize)]
pub struct RefreshRequestV3 {
    pub origin: String,
    pub refresh_token: String,
}

impl Zeroize for RefreshRequestV3 {
    fn zeroize(&mut self) {
        self.refresh_token.zeroize();
    }
}

impl Drop for RefreshRequestV3 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for RefreshRequestV3 {}

impl std::fmt::Debug for RefreshRequestV3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RefreshRequestV3")
            .field("origin", &self.origin)
            .field("refresh_token", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Serialize)]
pub struct LogoutRequestV3 {
    pub origin: String,
    pub refresh_token: String,
}

impl Zeroize for LogoutRequestV3 {
    fn zeroize(&mut self) {
        self.refresh_token.zeroize();
    }
}

impl Drop for LogoutRequestV3 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for LogoutRequestV3 {}

impl std::fmt::Debug for LogoutRequestV3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogoutRequestV3")
            .field("origin", &self.origin)
            .field("refresh_token", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct LogoutResponseV3 {
    pub status: String,
}
