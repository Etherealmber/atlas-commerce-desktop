//! Authentication: password + OTP, one-tap e-mail link, profile, trusted
//! devices and QR phone pairing (desktop acting as the PC).

use crate::error::ApiError;
use crate::models::*;
use crate::{ApiClient, client_kind};

impl ApiClient {
    /// `POST /api/auth/login` — returns either a session token directly
    /// (trusted device) or `needs_otp` with an e-mailed code.
    pub async fn login(&self, username: &str, password: &str) -> Result<LoginResponse, ApiError> {
        let request = LoginRequest {
            username,
            password,
            fingerprint: &self.fingerprint,
            client: client_kind(),
        };
        self.post_json("/api/auth/login", &request).await
    }

    /// `POST /api/auth/verify-otp`.
    pub async fn verify_otp(
        &self,
        username: &str,
        password: &str,
        otp: &str,
    ) -> Result<LoginResponse, ApiError> {
        let request = OtpRequest {
            username,
            password,
            otp,
            fingerprint: &self.fingerprint,
            client: client_kind(),
        };
        self.post_json("/api/auth/verify-otp", &request).await
    }

    /// `POST /api/auth/verify-link` — the one-tap e-mail link, no typing.
    pub async fn verify_link(
        &self,
        username: &str,
        otp: &str,
        sig: &str,
        exp: u64,
    ) -> Result<LoginResponse, ApiError> {
        let request = VerifyLinkRequest {
            username,
            otp,
            sig,
            exp,
            fingerprint: &self.fingerprint,
            client: client_kind(),
        };
        self.post_json("/api/auth/verify-link", &request).await
    }

    /// `GET /api/auth/me`.
    pub async fn me(&self) -> Result<Me, ApiError> {
        self.get_data("/api/auth/me").await
    }

    /// `PATCH /api/auth/me` — editable profile fields only. The worker
    /// rejects role, username and password changes on this endpoint.
    pub async fn update_me(&self, patch: &MeUpdate) -> Result<Me, ApiError> {
        self.patch_data("/api/auth/me", patch).await
    }

    /// `GET /api/auth/devices`.
    pub async fn devices(&self) -> Result<Vec<TrustedDevice>, ApiError> {
        self.get_data("/api/auth/devices").await
    }

    /// `POST /api/auth/devices/revoke`.
    pub async fn revoke_device(&self, fingerprint: &str) -> Result<(), ApiError> {
        let body = serde_json::json!({ "fingerprint": fingerprint });
        let status: ApiStatus = self.post_data("/api/auth/devices/revoke", &body).await?;
        if status.success {
            Ok(())
        } else {
            Err(ApiError {
                status: 400,
                code: status.code,
                message: status
                    .error
                    .unwrap_or_else(|| "Révocation impossible.".to_owned()),
            })
        }
    }

    /// Creates the PC-side QR session the phone scans
    /// (`POST /api/auth/qr/pc-session`, public, 60 seconds).
    pub async fn qr_create_pc_session(&self) -> Result<QrSessionCreated, ApiError> {
        let body = serde_json::json!({});
        self.post_data("/api/auth/qr/pc-session", &body).await
    }

    /// Polls the QR session status (`GET /api/auth/qr/session/:token`).
    pub async fn qr_status(&self, token: &str) -> Result<QrSessionStatus, ApiError> {
        let path = format!("/api/auth/qr/session/{token}");
        self.get_data(&path).await
    }

    /// Once the phone approved, exchanges the one-time QR session for this
    /// device's token (`POST …/complete`). The `pc-login` flow allows the
    /// PC fingerprint to differ from the phone's.
    pub async fn qr_complete(&self, token: &str) -> Result<QrComplete, ApiError> {
        let path = format!("/api/auth/qr/session/{token}/complete");
        let body = QrCompleteRequest {
            fingerprint: &self.fingerprint,
        };
        self.post_data(&path, &body).await
    }
}
