//! Account and session operations from the Desktop v3 Worker contract.
//!
//! These operations are deliberately isolated from the legacy bearer client.
//! No caller is wired to them yet. Passwords and codes are borrowed, never
//! logged, and a failed or uncertain write must not be automatically retried.

use atlas_auth_core::{DeviceKeyAdapter, SessionTokensV3};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::auth_v3::{V3Transport, V3TransportError, decode};

const BASE: &str = "/api/auth/desktop/v3";

#[derive(Deserialize)]
pub struct MailboxStatus {
    pub enrolled: bool,
    pub active: bool,
    pub masked_address: Option<String>,
    pub verified_at: Option<String>,
    pub recovery_pending: bool,
    pub recovery_request_id: Option<String>,
    pub owner_supported: Option<bool>,
}

#[derive(Deserialize)]
pub struct EnrollmentStarted {
    pub request_id: String,
    pub expires_in: u64,
}

#[derive(Deserialize)]
pub struct EnrollmentConfirmed {
    pub enrolled: bool,
}

#[derive(Deserialize)]
pub struct RecoveryRequested {
    pub recovery_id: String,
    pub status: String,
    pub expires_in: u64,
}

#[derive(Deserialize)]
pub struct RecoveryApproved {
    pub status: String,
    pub username: String,
}

#[derive(Deserialize)]
pub struct DesktopSession {
    pub id: String,
    pub origin: String,
    pub username: String,
    pub role: String,
    pub device_thumbprint: String,
    pub device_label: String,
    pub last_used_at: String,
    pub expires_at: String,
}

#[derive(Deserialize)]
pub struct SessionRevoked {
    pub status: String,
    pub id: String,
}

// Borrowed serializable forms do not create separate credential String copies.
#[derive(Serialize)]
struct EnrollmentStart<'a> {
    password: &'a str,
    email: &'a str,
}
#[derive(Serialize)]
struct EnrollmentConfirm<'a> {
    password: &'a str,
    request_id: &'a str,
    new_code: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    old_code: Option<&'a str>,
}
#[derive(Serialize)]
struct RecoveryRequest<'a> {
    password: &'a str,
    reason: &'a str,
}
#[derive(Serialize)]
struct RecoveryApprove<'a> {
    password: &'a str,
    recovery_id: &'a str,
}

fn worker_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|c| c.is_ascii_digit() || matches!(c, b'a'..=b'f'))
}

impl V3Transport {
    async fn account_get<T: DeserializeOwned>(
        &self,
        path: &str,
        tokens: &SessionTokensV3,
        key: &dyn DeviceKeyAdapter,
    ) -> Result<T, V3TransportError> {
        let response = self
            .protected_request(reqwest::Method::GET, path, tokens, key)
            .await?
            .send()
            .await
            .map_err(|_| V3TransportError::Network)?;
        decode(response).await
    }

    async fn account_post<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
        tokens: &SessionTokensV3,
        key: &dyn DeviceKeyAdapter,
    ) -> Result<T, V3TransportError> {
        let response = self
            .protected_request(reqwest::Method::POST, path, tokens, key)
            .await?
            .json(body)
            .send()
            .await
            .map_err(|_| V3TransportError::Network)?;
        decode(response).await
    }

    pub async fn mailbox_status(
        &self,
        tokens: &SessionTokensV3,
        key: &dyn DeviceKeyAdapter,
    ) -> Result<MailboxStatus, V3TransportError> {
        self.account_get(&format!("{BASE}/mailbox/status"), tokens, key)
            .await
    }

    pub async fn mailbox_enroll_start(
        &self,
        tokens: &SessionTokensV3,
        key: &dyn DeviceKeyAdapter,
        password: &str,
        email: &str,
    ) -> Result<EnrollmentStarted, V3TransportError> {
        self.account_post(
            &format!("{BASE}/mailbox/enroll/start"),
            &EnrollmentStart { password, email },
            tokens,
            key,
        )
        .await
    }

    pub async fn mailbox_enroll_confirm(
        &self,
        tokens: &SessionTokensV3,
        key: &dyn DeviceKeyAdapter,
        password: &str,
        request_id: &str,
        new_code: &str,
        old_code: Option<&str>,
    ) -> Result<EnrollmentConfirmed, V3TransportError> {
        if !worker_id(request_id) {
            return Err(V3TransportError::Protocol);
        }
        self.account_post(
            &format!("{BASE}/mailbox/enroll/confirm"),
            &EnrollmentConfirm {
                password,
                request_id,
                new_code,
                old_code,
            },
            tokens,
            key,
        )
        .await
    }

    pub async fn mailbox_recovery_request(
        &self,
        tokens: &SessionTokensV3,
        key: &dyn DeviceKeyAdapter,
        password: &str,
        reason: &str,
    ) -> Result<RecoveryRequested, V3TransportError> {
        self.account_post(
            &format!("{BASE}/mailbox/recovery/request"),
            &RecoveryRequest { password, reason },
            tokens,
            key,
        )
        .await
    }

    pub async fn mailbox_recovery_approve(
        &self,
        tokens: &SessionTokensV3,
        key: &dyn DeviceKeyAdapter,
        password: &str,
        recovery_id: &str,
    ) -> Result<RecoveryApproved, V3TransportError> {
        if !worker_id(recovery_id) {
            return Err(V3TransportError::Protocol);
        }
        self.account_post(
            &format!("{BASE}/mailbox/recovery/approve"),
            &RecoveryApprove {
                password,
                recovery_id,
            },
            tokens,
            key,
        )
        .await
    }

    pub async fn sessions(
        &self,
        tokens: &SessionTokensV3,
        key: &dyn DeviceKeyAdapter,
    ) -> Result<Vec<DesktopSession>, V3TransportError> {
        self.account_get(&format!("{BASE}/sessions"), tokens, key)
            .await
    }

    pub async fn revoke_session(
        &self,
        tokens: &SessionTokensV3,
        key: &dyn DeviceKeyAdapter,
        session_id: &str,
    ) -> Result<SessionRevoked, V3TransportError> {
        if !worker_id(session_id) {
            return Err(V3TransportError::Protocol);
        }
        let path = format!("{BASE}/sessions/{session_id}");
        let response = self
            .protected_request(reqwest::Method::DELETE, &path, tokens, key)
            .await?
            .send()
            .await
            .map_err(|_| V3TransportError::Network)?;
        let reply: SessionRevoked = decode(response).await?;
        if reply.status != "revoked" || reply.id != session_id {
            return Err(V3TransportError::InvalidResponse);
        }
        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::{EnrollmentConfirm, worker_id};

    #[test]
    fn enrollment_omits_absent_old_code() {
        let request = EnrollmentConfirm {
            password: "fixture-password",
            request_id: "fixture-id",
            new_code: "123456",
            old_code: None,
        };
        let body = serde_json::to_value(request).unwrap();
        assert_eq!(body["new_code"], "123456");
        assert!(body.get("old_code").is_none());
    }

    #[test]
    fn worker_session_ids_are_lowercase_fixed_length_hex() {
        assert!(worker_id(&"a1".repeat(32)));
        for invalid in ["", "../sessions", "A", "zz", "../"] {
            assert!(!worker_id(invalid));
        }
        assert!(!worker_id(&"A".repeat(64)));
        assert!(!worker_id(&"g".repeat(64)));
    }
}
