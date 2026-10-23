//! API errors. The worker's French `error` text is always preserved
//! verbatim — the login reputation gate and the OTP flow answer users in
//! French, and the interface must not paraphrase them.

use thiserror::Error;

/// A failed API call.
#[derive(Debug, Clone, Error)]
#[error("{message}")]
pub struct ApiError {
    /// The HTTP status code, or zero for a transport failure.
    pub status: u16,
    /// The worker's machine code (`BAD_OTP`, `RATE_LIMITED`…), when present.
    pub code: Option<String>,
    /// The French message shown to the user, verbatim from the worker.
    pub message: String,
}

impl ApiError {
    #[must_use]
    pub fn transport(message: impl Into<String>) -> Self {
        Self {
            status: 0,
            code: Some("NETWORK".to_owned()),
            message: message.into(),
        }
    }

    #[must_use]
    pub fn client(message: impl Into<String>) -> Self {
        Self {
            status: 0,
            code: Some("CLIENT".to_owned()),
            message: message.into(),
        }
    }

    /// Wrong-password / wrong-OTP style answers must not destroy the stored
    /// session the way a dead token does (same rule as the Android app).
    #[must_use]
    pub fn is_user_mistake(&self) -> bool {
        matches!(
            self.code.as_deref(),
            Some(
                "BAD_PASSWORD"
                    | "PASSWORD_REQUIRED"
                    | "BAD_OTP"
                    | "OTP_REQUIRED"
                    | "OTP_EXPIRED"
                    | "OTP_NOT_REQUESTED"
                    | "RATE_LIMITED"
                    | "TURNSTILE_FAILED"
                    | "LINK_MALFORMED"
                    | "LINK_EXPIRED"
                    | "LINK_INVALID"
            )
        )
    }

    /// A 401 on an authenticated request whose code is not a user mistake
    /// means the session token died (IP/device binding changed).
    #[must_use]
    pub fn is_session_expired(&self, was_authenticated: bool) -> bool {
        was_authenticated && self.status == 401 && !self.is_user_mistake()
    }
}

/// Turns an HTTP response into an [`ApiError`], keeping the French payload.
pub(crate) async fn response_into_error(response: reqwest::Response) -> ApiError {
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    match serde_json::from_str::<crate::models::ApiStatus>(&body) {
        Ok(parsed) => ApiError {
            status,
            code: parsed.code,
            message: parsed
                .error
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| format!("Requête refusée ({status}).")),
        },
        Err(_) => ApiError {
            status,
            code: None,
            message: format!("Le serveur a répondu par une erreur {status}."),
        },
    }
}
