//! Async client for the Atlas Commerce worker.
//!
//! One [`ApiClient`] per selected server. Every request carries the headers
//! the worker expects:
//!
//! - `X-Atlas-Client: native` — skips Cloudflare Turnstile for native apps,
//! - `X-Device-Fingerprint` — sessions are bound to `(IP, fingerprint)`,
//! - a short, non-browser `User-Agent`,
//! - `Authorization: Bearer <token>` once authenticated.
//!
//! The body keeps `client: "desktop"` per the desktop platform contract; the
//! native header is what the current worker's `isNativeAuthRequest` accepts
//! directly (the body value is forwarded too for forward compatibility).

pub mod agent;
pub mod auth;
pub mod clients;
pub mod commerce;
pub mod error;
pub mod models;
pub mod probe;

use std::sync::Mutex;

pub use error::ApiError;
pub use models::*;

const CLIENT_KIND: &str = "desktop";
const NATIVE_HEADER: &str = "native";

/// The authenticated, server-scoped HTTP client.
#[derive(Debug)]
pub struct ApiClient {
    http: reqwest::Client,
    base: String,
    fingerprint: String,
    token: Mutex<Option<String>>,
}

impl ApiClient {
    /// Builds a client for `base_url` (normalized, no trailing slash).
    ///
    /// # Errors
    /// Fails when the reqwest client cannot be built.
    pub fn new(base_url: &str, fingerprint: &str, user_agent: &str) -> Result<Self, ApiError> {
        let http = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|error| ApiError::transport(format!("Client HTTP impossible : {error}")))?;
        Ok(Self {
            http,
            base: base_url.trim_end_matches('/').to_owned(),
            fingerprint: fingerprint.to_owned(),
            token: Mutex::new(None),
        })
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base
    }

    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn set_token(&self, token: Option<String>) {
        if let Ok(mut guard) = self.token.lock() {
            *guard = token;
        }
    }

    #[must_use]
    pub fn token(&self) -> Option<String> {
        self.token.lock().ok().and_then(|guard| guard.clone())
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base, path.trim_start_matches('/'))
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut builder = self
            .http
            .request(method, self.url(path))
            .header(reqwest::header::ACCEPT, "application/json")
            .header("X-Atlas-Client", NATIVE_HEADER)
            .header("X-Device-Fingerprint", &self.fingerprint);
        if let Ok(guard) = self.token.lock()
            && let Some(token) = guard.as_deref()
        {
            builder = builder.bearer_auth(token);
        }
        builder
    }

    /// GET returning the envelope's `data`, or the verbatim worker error.
    pub(crate) async fn get_data<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<T, ApiError> {
        let response = self
            .request(reqwest::Method::GET, path)
            .send()
            .await
            .map_err(transport_error)?;
        self.unwrap_envelope(response).await
    }

    /// POST a JSON body, returning the envelope's `data`.
    pub(crate) async fn post_data<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        let response = self
            .request(reqwest::Method::POST, path)
            .json(body)
            .send()
            .await
            .map_err(transport_error)?;
        self.unwrap_envelope(response).await
    }

    /// PUT a JSON body, returning the envelope's `data`.
    pub(crate) async fn put_data<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        let response = self
            .request(reqwest::Method::PUT, path)
            .json(body)
            .send()
            .await
            .map_err(transport_error)?;
        self.unwrap_envelope(response).await
    }

    /// POST whose JSON answer is read whole (login endpoints keep the token
    /// at the top level rather than under `data`).
    pub(crate) async fn post_json<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        let response = self
            .request(reqwest::Method::POST, path)
            .json(body)
            .send()
            .await
            .map_err(transport_error)?;
        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|error| ApiError::transport(format!("Réponse illisible : {error}")))
        } else {
            Err(error::response_into_error(response).await)
        }
    }

    pub(crate) async fn unwrap_envelope<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, ApiError> {
        let status = response.status();
        if !status.is_success() {
            return Err(error::response_into_error(response).await);
        }
        let envelope: Envelope<T> = response
            .json()
            .await
            .map_err(|error| ApiError::transport(format!("Réponse illisible : {error}")))?;
        if !envelope.success {
            return Err(ApiError {
                status: status.as_u16(),
                code: envelope.code,
                message: envelope
                    .error
                    .unwrap_or_else(|| "Le serveur a refusé l'opération.".to_owned()),
            });
        }
        envelope
            .data
            .ok_or_else(|| ApiError::client("Réponse serveur incomplète (champ data absent)."))
    }
}

#[must_use]
pub fn client_kind() -> &'static str {
    CLIENT_KIND
}

fn transport_error(error: reqwest::Error) -> ApiError {
    log::warn!("network failure: {error}");
    if error.is_timeout() {
        ApiError::transport("Le serveur met trop de temps à répondre. Vérifiez votre connexion.")
    } else if error.is_connect() {
        ApiError::transport("Connexion impossible au serveur. Vérifiez l'adresse et votre réseau.")
    } else {
        ApiError::transport(format!("Erreur réseau : {error}"))
    }
}
