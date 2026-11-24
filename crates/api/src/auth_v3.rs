//! Isolated transport for desktop v3 authentication.
//!
//! This is a building block, not an enabled login method. It deliberately does
//! not use the legacy client's bearer header, follow redirects, retry requests,
//! inspect WebView content, or store credentials. The caller must enforce the
//! server's monotonic deadline, polling interval and cancellation policy.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use atlas_auth_core::{
    CapabilitiesV3Response, ChallengePollResponse, CreateRequestV3Response, DesktopV3ApiEnvelope,
    DesktopV3AuthSession, DesktopV3Phase, DeviceKeyAdapter, LogoutRequestV3, LogoutResponseV3,
    NonceResponse, PasswordLoginResponseV3, RefreshRequestV3, ServerOrigin, SessionTokensV3,
    build_dpop_proof, build_protected_request_headers,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

const BASE: &str = "/api/auth/desktop/v3";
const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// A sanitized transport failure. Never contains a URL, server body or secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum V3TransportError {
    InvalidOrigin,
    Network,
    InvalidResponse,
    Protocol,
    /// Discovery is valid, but the server has not enabled the required method.
    Unavailable,
    /// Server asked the challenge poller to slow down; wait at least this
    /// many seconds, still respecting the challenge's deadline and poll cap.
    SlowDown(u64),
    /// HTTP status only; callers must not infer that a lost response is safe to retry.
    Rejected(u16),
}

fn validate_capabilities(capabilities: &CapabilitiesV3Response) -> Result<(), V3TransportError> {
    if capabilities.version != 3
        || capabilities
            .methods
            .device_proof
            .hardware_attestation_claimed
    {
        return Err(V3TransportError::InvalidResponse);
    }
    if !capabilities.enabled
        || !capabilities.methods.password_otp.available
        || !capabilities.methods.device_proof.available
    {
        return Err(V3TransportError::Unavailable);
    }
    Ok(())
}

/// A strict HTTPS client scoped to one origin. No v3 credentials enter `ApiClient`.
pub struct V3Transport {
    origin: ServerOrigin,
    http: reqwest::Client,
}

impl V3Transport {
    /// Construct an isolated client that never follows redirects.
    pub fn new(server: &str) -> Result<Self, V3TransportError> {
        let origin = ServerOrigin::parse(server).map_err(|_| V3TransportError::InvalidOrigin)?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| V3TransportError::Network)?;
        Ok(Self { origin, http })
    }

    /// Query v3 capability discovery without affecting the legacy login state.
    pub async fn capabilities(&self) -> Result<CapabilitiesV3Response, V3TransportError> {
        let url = self.endpoint("/capabilities")?;
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|_| V3TransportError::Network)?;
        let capabilities: CapabilitiesV3Response = decode(response).await?;
        validate_capabilities(&capabilities)?;
        Ok(capabilities)
    }

    async fn nonce(&self) -> Result<NonceResponse, V3TransportError> {
        let url = self.endpoint("/nonce")?;
        let response = self
            .http
            .post(url)
            .send()
            .await
            .map_err(|_| V3TransportError::Network)?;
        if !response.status().is_success() {
            return Err(V3TransportError::Rejected(response.status().as_u16()));
        }
        // The Worker supplies the same nonce in the response header and
        // envelope. Require both to agree before using it in any proof.
        let header_nonce = response
            .headers()
            .get("DPoP-Nonce")
            .and_then(|value| value.to_str().ok())
            .ok_or(V3TransportError::InvalidResponse)?
            .to_owned();
        let nonce: NonceResponse = decode(response).await?;
        if header_nonce != nonce.nonce
            || !(1..=120).contains(&nonce.expires_in)
            || nonce.nonce.len() != 43
            || !nonce
                .nonce
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(V3TransportError::InvalidResponse);
        }
        Ok(nonce)
    }

    fn endpoint(&self, path: &str) -> Result<String, V3TransportError> {
        // Only fixed endpoints supplied by this module are used here.
        self.origin
            .endpoint_url(&format!("{BASE}{path}"))
            .map_err(|_| V3TransportError::Protocol)
    }

    async fn post_proof<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
        key: &dyn DeviceKeyAdapter,
        transaction: Option<&str>,
    ) -> Result<T, V3TransportError> {
        let nonce = self.nonce().await?;
        let url = self.endpoint(path)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| V3TransportError::Protocol)?
            .as_secs();
        let proof = build_dpop_proof(key, "POST", &url, &nonce.nonce, now, None, transaction)
            .map_err(|_| V3TransportError::Protocol)?;
        let response = self
            .http
            .post(url)
            .header("DPoP", proof)
            .json(body)
            .send()
            .await
            .map_err(|_| V3TransportError::Network)?;
        decode(response).await
    }

    /// Create a challenge exactly once; a lost response must not be replayed.
    pub async fn create(
        &self,
        session: &mut DesktopV3AuthSession,
        key: &dyn DeviceKeyAdapter,
    ) -> Result<CreateRequestV3Response, V3TransportError> {
        let body = session
            .create_request_body()
            .map_err(|_| V3TransportError::Protocol)?;
        if body.origin != self.origin.as_str() {
            return Err(V3TransportError::Protocol);
        }
        let result: Result<CreateRequestV3Response, V3TransportError> =
            self.post_proof("/requests", &body, key, None).await;
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => {
                session.cancel();
                return Err(error);
            }
        };
        if !(1..=300).contains(&reply.expires_in)
            || !(5..=60).contains(&reply.interval)
            || session.apply_created(&reply, key).is_err()
        {
            session.cancel();
            return Err(V3TransportError::InvalidResponse);
        }
        Ok(reply)
    }

    /// Poll using the core-generated PKCE proof. The caller must wait at least
    /// five seconds and respect the transaction deadline and poll limit.
    pub async fn poll(
        &self,
        session: &mut DesktopV3AuthSession,
        key: &dyn DeviceKeyAdapter,
    ) -> Result<ChallengePollResponse, V3TransportError> {
        let body = session
            .build_poll_request()
            .map_err(|_| V3TransportError::Protocol)?;
        if body.origin != self.origin.as_str() {
            return Err(V3TransportError::Protocol);
        }
        let reply: ChallengePollResponse = self
            .post_proof("/challenge/poll", &body, key, Some(&body.request_id))
            .await?;
        if !(5..=60).contains(&reply.interval) {
            return Err(V3TransportError::InvalidResponse);
        }
        session
            .apply_poll(&reply)
            .map_err(|_| V3TransportError::InvalidResponse)?;
        Ok(reply)
    }

    /// Cancel the active server-side transaction once, and invalidate the
    /// local attempt even if the server response is lost or rejected.
    pub async fn cancel(
        &self,
        session: &mut DesktopV3AuthSession,
        key: &dyn DeviceKeyAdapter,
    ) -> Result<(), V3TransportError> {
        let path = match session.phase() {
            DesktopV3Phase::AwaitingChallenge | DesktopV3Phase::ChallengeVerified => {
                "/challenge/cancel"
            }
            DesktopV3Phase::AwaitingOtp => "/login/cancel",
            _ => return Err(V3TransportError::Protocol),
        };
        let body = session
            .build_cancel_request()
            .map_err(|_| V3TransportError::Protocol)?;
        if body.origin != self.origin.as_str() {
            session.cancel();
            return Err(V3TransportError::Protocol);
        }
        #[derive(serde::Deserialize)]
        struct CancelResponse {
            status: String,
        }
        let result: Result<CancelResponse, V3TransportError> = self
            .post_proof(path, &body, key, Some(&body.request_id))
            .await;
        session.cancel();
        match result {
            Ok(response) if response.status == "cancelled" => Ok(()),
            Ok(_) => Err(V3TransportError::InvalidResponse),
            Err(error) => Err(error),
        }
    }

    /// Rotate a refresh token once. This consumes the old token bundle even
    /// on failure: an uncertain response may already have rotated the server
    /// token, so replay could revoke the entire family. The caller must store
    /// the returned bundle atomically before issuing further requests.
    pub async fn refresh(
        &self,
        session: &mut DesktopV3AuthSession,
        key: &dyn DeviceKeyAdapter,
        tokens: SessionTokensV3,
    ) -> Result<SessionTokensV3, V3TransportError> {
        if session.phase() != DesktopV3Phase::Authenticated
            || tokens.origin != self.origin.as_str()
            || tokens.device_thumbprint != key.thumbprint()
        {
            session.cancel();
            return Err(V3TransportError::Protocol);
        }
        let body = RefreshRequestV3 {
            origin: self.origin.as_str().to_owned(),
            refresh_token: tokens.refresh_token.clone(),
        };
        let result: Result<SessionTokensV3, V3TransportError> =
            self.post_proof("/session/refresh", &body, key, None).await;
        match result {
            Ok(next)
                if next.session_id == tokens.session_id
                    && next.username == tokens.username
                    && next.role == tokens.role
                    && next.refresh_token != tokens.refresh_token
                    && session.apply_refreshed(&next, key).is_ok() =>
            {
                Ok(next)
            }
            Ok(_) => {
                session.cancel();
                Err(V3TransportError::InvalidResponse)
            }
            Err(error) => {
                session.cancel();
                Err(error)
            }
        }
    }

    /// Revoke the session once, consuming the local token bundle regardless of
    /// the result. An uncertain result must never be silently retried.
    /// The Worker contract for this endpoint uses refresh-token possession
    /// plus DPoP (its contract test does not send an Authorization header).
    pub async fn logout(
        &self,
        session: &mut DesktopV3AuthSession,
        key: &dyn DeviceKeyAdapter,
        tokens: SessionTokensV3,
    ) -> Result<(), V3TransportError> {
        if session.phase() != DesktopV3Phase::Authenticated
            || tokens.origin != self.origin.as_str()
            || tokens.device_thumbprint != key.thumbprint()
        {
            session.cancel();
            return Err(V3TransportError::Protocol);
        }
        let body = LogoutRequestV3 {
            origin: self.origin.as_str().to_owned(),
            refresh_token: tokens.refresh_token.clone(),
        };
        let result: Result<LogoutResponseV3, V3TransportError> =
            self.post_proof("/session/logout", &body, key, None).await;
        match result {
            Ok(reply) if session.mark_logged_out(&reply).is_ok() => Ok(()),
            Ok(_) => {
                session.cancel();
                Err(V3TransportError::InvalidResponse)
            }
            Err(error) => {
                session.cancel();
                Err(error)
            }
        }
    }

    /// Prepare an authenticated request on this origin. Each invocation
    /// fetches a fresh one-use nonce and signs a fresh proof with `ath`.
    /// Callers must handle revocation and must not log request headers.
    pub async fn protected_request(
        &self,
        method: reqwest::Method,
        path: &str,
        tokens: &SessionTokensV3,
        key: &dyn DeviceKeyAdapter,
    ) -> Result<reqwest::RequestBuilder, V3TransportError> {
        if tokens.origin != self.origin.as_str() || tokens.device_thumbprint != key.thumbprint() {
            return Err(V3TransportError::Protocol);
        }
        let url = self.protected_url(path)?;
        let nonce = self.nonce().await?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| V3TransportError::Protocol)?
            .as_secs();
        let (authorization, proof) =
            build_protected_request_headers(key, method.as_str(), &url, &nonce.nonce, now, tokens)
                .map_err(|_| V3TransportError::Protocol)?;
        Ok(self
            .http
            .request(method, url)
            .header(reqwest::header::AUTHORIZATION, authorization)
            .header("DPoP", proof)
            .header(reqwest::header::ACCEPT, "application/json"))
    }

    fn protected_url(&self, path: &str) -> Result<String, V3TransportError> {
        if !path.starts_with("/api/") || path.contains('\\') || path.contains('#') {
            return Err(V3TransportError::Protocol);
        }
        let url = reqwest::Url::parse(&format!("{}{path}", self.origin.as_str()))
            .map_err(|_| V3TransportError::Protocol)?;
        if url.origin().ascii_serialization() != self.origin.as_str()
            || !url.path().starts_with("/api/")
        {
            return Err(V3TransportError::Protocol);
        }
        Ok(url.to_string())
    }

    /// Submit credentials only after the core state machine verified a challenge.
    /// A lost response is uncertain and must not be automatically retried.
    pub async fn password(
        &self,
        session: &mut DesktopV3AuthSession,
        key: &dyn DeviceKeyAdapter,
        username: &str,
        password: &str,
    ) -> Result<(), V3TransportError> {
        let body = session
            .build_password_login(username, password)
            .map_err(|_| V3TransportError::Protocol)?;
        if body.origin != self.origin.as_str() {
            return Err(V3TransportError::Protocol);
        }
        let result: Result<PasswordLoginResponseV3, V3TransportError> = self
            .post_proof("/login/password", &body, key, Some(&body.request_id))
            .await;
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => {
                session.cancel();
                return Err(error);
            }
        };
        if reply.status != "otp_pending" {
            session.cancel();
            return Err(V3TransportError::InvalidResponse);
        }
        session
            .mark_otp_pending()
            .map_err(|_| V3TransportError::Protocol)
    }

    /// Exchange an OTP once. A lost response may have issued a session, so
    /// callers must not retry an uncertain response.
    pub async fn otp(
        &self,
        session: &mut DesktopV3AuthSession,
        key: &dyn DeviceKeyAdapter,
        otp: &str,
    ) -> Result<SessionTokensV3, V3TransportError> {
        let body = session
            .build_otp_login(otp)
            .map_err(|_| V3TransportError::Protocol)?;
        if body.origin != self.origin.as_str() {
            return Err(V3TransportError::Protocol);
        }
        let result: Result<SessionTokensV3, V3TransportError> = self
            .post_proof("/login/otp", &body, key, Some(&body.request_id))
            .await;
        let tokens = match result {
            Ok(tokens) => tokens,
            Err(error) => {
                // Conservative: restart even for a known rejection. In
                // particular, never replay after an uncertain response.
                session.cancel();
                return Err(error);
            }
        };
        if session.apply_authenticated(&tokens, key).is_err() {
            session.cancel();
            return Err(V3TransportError::InvalidResponse);
        }
        Ok(tokens)
    }
}

/// Only the fixed, allowlisted error code is inspected. A 429 from a proxy,
/// malformed response or a different budget must not be treated as SLOW_DOWN.
async fn classify_rate_limit(mut response: reqwest::Response) -> V3TransportError {
    let retry = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| (5..=600).contains(seconds));
    let json = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"));
    if retry.is_none() || !json || response.content_length().is_some_and(|n| n > 4096) {
        return V3TransportError::Rejected(429);
    }
    let mut bytes = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) if bytes.len().saturating_add(chunk.len()) <= 4096 => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(None) => break,
            _ => return V3TransportError::Rejected(429),
        }
    }
    classify_rate_limit_body(&bytes, retry.unwrap_or(5))
}

fn classify_rate_limit_body(bytes: &[u8], retry: u64) -> V3TransportError {
    #[derive(serde::Deserialize)]
    struct ErrorCode {
        code: Option<String>,
    }
    match serde_json::from_slice::<ErrorCode>(bytes) {
        Ok(error) if error.code.as_deref() == Some("SLOW_DOWN") => {
            V3TransportError::SlowDown(retry)
        }
        _ => V3TransportError::Rejected(429),
    }
}

pub(crate) async fn decode<T: DeserializeOwned>(
    mut response: reqwest::Response,
) -> Result<T, V3TransportError> {
    if !response.status().is_success() {
        let status = response.status().as_u16();
        if status == 429 {
            return Err(classify_rate_limit(response).await);
        }
        return Err(V3TransportError::Rejected(status));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if !content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("application/json")
        || response
            .content_length()
            .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
    {
        return Err(V3TransportError::InvalidResponse);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| V3TransportError::Network)?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(V3TransportError::InvalidResponse);
        }
        bytes.extend_from_slice(&chunk);
    }
    let envelope: DesktopV3ApiEnvelope<T> =
        serde_json::from_slice(&bytes).map_err(|_| V3TransportError::InvalidResponse)?;
    if !envelope.success {
        return Err(V3TransportError::InvalidResponse);
    }
    envelope.data.ok_or(V3TransportError::InvalidResponse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_down_does_not_confuse_other_rate_limits() {
        assert_eq!(
            classify_rate_limit_body(br#"{"success":false,"code":"SLOW_DOWN"}"#, 5),
            V3TransportError::SlowDown(5)
        );
        assert_eq!(
            classify_rate_limit_body(br#"{"code":"RATE_LIMITED"}"#, 600),
            V3TransportError::Rejected(429)
        );
        assert_eq!(
            classify_rate_limit_body(b"not json", 5),
            V3TransportError::Rejected(429)
        );
    }

    #[test]
    fn discovery_requires_the_enabled_proof_bound_password_method() {
        let value = serde_json::json!({
            "version": 3, "enabled": true,
            "methods": {
                "password_otp": {"available": true},
                "mailbox_enrollment": {"available": true},
                "recovery": {"available": true},
                "device_proof": {"available": true, "hardware_attestation_claimed": false}
            }
        });
        let parse = |v| serde_json::from_value::<CapabilitiesV3Response>(v).unwrap();
        assert_eq!(validate_capabilities(&parse(value.clone())), Ok(()));
        for pointer in [
            "/enabled",
            "/methods/password_otp/available",
            "/methods/device_proof/available",
        ] {
            let mut disabled = value.clone();
            *disabled.pointer_mut(pointer).unwrap() = false.into();
            assert_eq!(
                validate_capabilities(&parse(disabled)),
                Err(V3TransportError::Unavailable)
            );
        }
        let mut invalid = value.clone();
        invalid["methods"]["device_proof"]["hardware_attestation_claimed"] = true.into();
        assert_eq!(
            validate_capabilities(&parse(invalid)),
            Err(V3TransportError::InvalidResponse)
        );
        let mut invalid = value;
        invalid["version"] = 2.into();
        assert_eq!(
            validate_capabilities(&parse(invalid)),
            Err(V3TransportError::InvalidResponse)
        );
    }

    #[test]
    fn protected_targets_are_confined_to_the_api_on_the_selected_origin() {
        let transport = V3Transport::new("https://example.invalid:8443").unwrap();
        assert_eq!(
            transport.protected_url("/api/products?limit=2").unwrap(),
            "https://example.invalid:8443/api/products?limit=2"
        );
        for invalid in [
            "https://evil.invalid/api/x",
            "//evil.invalid/api/x",
            "/api/../login",
            "/api/\\evil",
            "/api/products#fragment",
        ] {
            assert!(transport.protected_url(invalid).is_err());
        }
    }

    #[test]
    fn transport_requires_bare_https_origin_and_preserves_port() {
        assert!(V3Transport::new("http://example.invalid").is_err());
        assert!(V3Transport::new("https://example.invalid/login").is_err());
        assert!(V3Transport::new("https://user@example.invalid").is_err());
        let transport = V3Transport::new("https://example.invalid:8443").unwrap();
        assert_eq!(
            transport.endpoint("/nonce").unwrap(),
            "https://example.invalid:8443/api/auth/desktop/v3/nonce"
        );
    }
}
