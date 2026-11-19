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

    /// `GET /api/auth/trusted-devices` (the worker's current device list).
    pub async fn devices(&self) -> Result<Vec<TrustedDevice>, ApiError> {
        self.get_data("/api/auth/trusted-devices").await
    }

    /// `DELETE /api/auth/trusted-devices/:fingerprint`.
    pub async fn revoke_device(&self, fingerprint: &str) -> Result<(), ApiError> {
        if fingerprint.is_empty() || fingerprint == "." || fingerprint == ".." {
            return Err(ApiError::transport("Empreinte invalide.".to_owned()));
        }
        let encoded = crate::commerce::urlencoding(fingerprint).replace('+', "%20");
        self.delete_success(&format!("/api/auth/trusted-devices/{encoded}"))
            .await
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

#[cfg(test)]
mod trusted_device_tests {
    use crate::ApiClient;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    #[test]
    fn worker_device_list_and_revoke_use_documented_routes() {
        for (expected, response, revoke) in [
            (
                "GET /api/auth/trusted-devices HTTP/1.1",
                r#"{"success":true,"data":[{"fingerprint":"abc","label":"Laptop","is_current":true}]}"#,
                false,
            ),
            (
                "DELETE /api/auth/trusted-devices/a%2Fb HTTP/1.1",
                r#"{"success":true}"#,
                true,
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                    let mut bytes = [0; 1024];
                    let count = stream.read(&mut bytes).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&bytes[..count]);
                }
                assert!(String::from_utf8_lossy(&request).starts_with(expected));
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
            });
            let client =
                ApiClient::new(&format!("http://{address}"), "fixture", "fixture").unwrap();
            let runner = atlas_core::jobs::JobRunner::new();
            let result = runner
                .spawn(async move {
                    if revoke {
                        client.revoke_device("a/b").await
                    } else {
                        let devices = client.devices().await?;
                        assert_eq!(devices[0].label, "Laptop");
                        assert!(devices[0].current);
                        Ok(())
                    }
                })
                .wait()
                .unwrap();
            assert!(result.is_ok());
            server.join().unwrap();
        }
    }
}
