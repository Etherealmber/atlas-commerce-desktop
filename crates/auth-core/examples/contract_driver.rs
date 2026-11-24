//! JSON-line stdio contract driver used by `worker/__tests__/desktop-v3-rust-contract.test.ts`
//! to exercise the real compiled `atlas-auth-core` Rust crate against live
//! Hono Worker responses.
use atlas_auth_core::{
    build_dpop_proof, build_protected_request_headers, CapabilitiesV3Response,
    ChallengePollResponse, ChallengePresentation, CreateRequestV3Response, DesktopV3ApiEnvelope,
    DesktopV3AuthSession, DeviceKeyAdapter, LogoutRequestV3, LogoutResponseV3, NonceResponse,
    PasswordLoginResponseV3, RefreshRequestV3, ServerOrigin, SessionTokensV3, SoftwareP256Key,
};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Command {
    Init {
        origin: String,
        presentation: ChallengePresentation,
        device_label: String,
        capabilities_json: String,
    },
    BuildCreateRequest {
        nonce_json: String,
        endpoint_url: String,
        iat_secs: u64,
    },
    ApplyCreated {
        created_json: String,
    },
    BuildPollProof {
        nonce_json: String,
        endpoint_url: String,
        iat_secs: u64,
    },
    ApplyPoll {
        poll_json: String,
    },
    BuildPasswordLogin {
        nonce_json: String,
        endpoint_url: String,
        iat_secs: u64,
        username: String,
        password: String,
    },
    ApplyPasswordLogin {
        password_json: String,
    },
    BuildOtpLogin {
        nonce_json: String,
        endpoint_url: String,
        iat_secs: u64,
        otp: String,
    },
    ApplyAuthenticated {
        tokens_json: String,
    },
    BuildProtectedHeaders {
        method: String,
        target_url: String,
        nonce_json: String,
        iat_secs: u64,
    },
    BuildRefresh {
        nonce_json: String,
        endpoint_url: String,
        iat_secs: u64,
    },
    ApplyRefreshed {
        tokens_json: String,
    },
    BuildLogout {
        nonce_json: String,
        endpoint_url: String,
        iat_secs: u64,
    },
    ApplyLoggedOut {
        logout_json: String,
    },
}

#[derive(Serialize)]
struct DriverReply {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dpop: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authorization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    challenge_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
}

impl DriverReply {
    fn ok() -> Self {
        Self {
            ok: true,
            error: None,
            phase: None,
            thumbprint: None,
            dpop: None,
            authorization: None,
            body: None,
            challenge_url: None,
            request_id: None,
        }
    }

    fn err(msg: impl ToString) -> Self {
        Self {
            ok: false,
            error: Some(msg.to_string()),
            phase: None,
            thumbprint: None,
            dpop: None,
            authorization: None,
            body: None,
            challenge_url: None,
            request_id: None,
        }
    }
}

fn unwrap_data<T: for<'de> Deserialize<'de>>(raw: &str) -> Result<T, String> {
    let env: DesktopV3ApiEnvelope<T> = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    if !env.success {
        return Err(env.code.unwrap_or_else(|| "API_ERROR".to_string()));
    }
    env.data.ok_or_else(|| "MISSING_DATA".to_string())
}

fn main() {
    let key = SoftwareP256Key::generate().expect("generate key");
    let mut origin: Option<ServerOrigin> = None;
    let mut session: Option<DesktopV3AuthSession> = None;
    let mut tokens: Option<SessionTokensV3> = None;

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let raw = match line {
            Ok(l) => l,
            Err(e) => {
                let _ = writeln!(
                    stdout,
                    "{}",
                    serde_json::to_string(&DriverReply::err(e)).unwrap()
                );
                break;
            }
        };
        if raw.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Command>(&raw) {
            Err(e) => DriverReply::err(format!("INVALID_CMD: {e}")),
            Ok(cmd) => match handle(cmd, &key, &mut origin, &mut session, &mut tokens) {
                Ok(r) => r,
                Err(e) => DriverReply::err(e),
            },
        };
        writeln!(stdout, "{}", serde_json::to_string(&reply).unwrap()).unwrap();
        stdout.flush().unwrap();
    }
}

fn handle(
    cmd: Command,
    key: &SoftwareP256Key,
    origin_slot: &mut Option<ServerOrigin>,
    session_slot: &mut Option<DesktopV3AuthSession>,
    tokens_slot: &mut Option<SessionTokensV3>,
) -> Result<DriverReply, String> {
    match cmd {
        Command::Init {
            origin,
            presentation,
            device_label,
            capabilities_json,
        } => {
            let parsed_origin = ServerOrigin::parse(&origin).map_err(|e| e.to_string())?;
            let caps: CapabilitiesV3Response = unwrap_data(&capabilities_json)?;
            let sess = DesktopV3AuthSession::new(
                parsed_origin.clone(),
                presentation,
                &device_label,
                &caps,
            )
            .map_err(|e| e.to_string())?;
            let phase = sess.phase().as_str().to_string();
            *origin_slot = Some(parsed_origin);
            *session_slot = Some(sess);
            *tokens_slot = None;
            let mut r = DriverReply::ok();
            r.phase = Some(phase);
            r.thumbprint = Some(key.thumbprint());
            Ok(r)
        }
        Command::BuildCreateRequest {
            nonce_json,
            endpoint_url,
            iat_secs,
        } => {
            let sess = session_slot.as_ref().ok_or("NO_SESSION")?;
            let nonce = unwrap_data::<NonceResponse>(&nonce_json)?.nonce;
            let body = sess.create_request_body().map_err(|e| e.to_string())?;
            let dpop = build_dpop_proof(key, "POST", &endpoint_url, &nonce, iat_secs, None, None)
                .map_err(|e| e.to_string())?;
            let mut r = DriverReply::ok();
            r.dpop = Some(dpop);
            r.body = Some(serde_json::to_value(body).map_err(|e| e.to_string())?);
            Ok(r)
        }
        Command::ApplyCreated { created_json } => {
            let sess = session_slot.as_mut().ok_or("NO_SESSION")?;
            let created: CreateRequestV3Response = unwrap_data(&created_json)?;
            sess.apply_created(&created, key)
                .map_err(|e| e.to_string())?;
            let mut r = DriverReply::ok();
            r.phase = Some(sess.phase().as_str().to_string());
            r.request_id = sess.request_id().map(|s| s.to_string());
            r.challenge_url = sess.challenge_url().map(|s| s.to_string());
            Ok(r)
        }
        Command::BuildPollProof {
            nonce_json,
            endpoint_url,
            iat_secs,
        } => {
            let sess = session_slot.as_ref().ok_or("NO_SESSION")?;
            let _ = origin_slot.as_ref().ok_or("NO_ORIGIN")?;
            let nonce = unwrap_data::<NonceResponse>(&nonce_json)?.nonce;
            let body = sess.build_poll_request().map_err(|e| e.to_string())?;
            let dpop = build_dpop_proof(
                key,
                "POST",
                &endpoint_url,
                &nonce,
                iat_secs,
                None,
                Some(&body.request_id),
            )
            .map_err(|e| e.to_string())?;
            let mut r = DriverReply::ok();
            r.dpop = Some(dpop);
            r.body = Some(serde_json::to_value(&body).map_err(|e| e.to_string())?);
            Ok(r)
        }
        Command::ApplyPoll { poll_json } => {
            let sess = session_slot.as_mut().ok_or("NO_SESSION")?;
            let poll: ChallengePollResponse = unwrap_data(&poll_json)?;
            sess.apply_poll(&poll).map_err(|e| e.to_string())?;
            let mut r = DriverReply::ok();
            r.phase = Some(sess.phase().as_str().to_string());
            Ok(r)
        }
        Command::BuildPasswordLogin {
            nonce_json,
            endpoint_url,
            iat_secs,
            username,
            password,
        } => {
            let sess = session_slot.as_ref().ok_or("NO_SESSION")?;
            let nonce = unwrap_data::<NonceResponse>(&nonce_json)?.nonce;
            let body = sess
                .build_password_login(&username, &password)
                .map_err(|e| e.to_string())?;
            let dpop = build_dpop_proof(
                key,
                "POST",
                &endpoint_url,
                &nonce,
                iat_secs,
                None,
                sess.request_id(),
            )
            .map_err(|e| e.to_string())?;
            let mut r = DriverReply::ok();
            r.dpop = Some(dpop);
            r.body = Some(serde_json::to_value(&body).map_err(|e| e.to_string())?);
            Ok(r)
        }
        Command::ApplyPasswordLogin { password_json } => {
            let sess = session_slot.as_mut().ok_or("NO_SESSION")?;
            let resp: PasswordLoginResponseV3 = unwrap_data(&password_json)?;
            if resp.status != "otp_pending" {
                return Err("UNEXPECTED_PW_STATUS".to_string());
            }
            sess.mark_otp_pending().map_err(|e| e.to_string())?;
            let mut r = DriverReply::ok();
            r.phase = Some(sess.phase().as_str().to_string());
            Ok(r)
        }
        Command::BuildOtpLogin {
            nonce_json,
            endpoint_url,
            iat_secs,
            otp,
        } => {
            let sess = session_slot.as_ref().ok_or("NO_SESSION")?;
            let nonce = unwrap_data::<NonceResponse>(&nonce_json)?.nonce;
            let body = sess.build_otp_login(&otp).map_err(|e| e.to_string())?;
            let dpop = build_dpop_proof(
                key,
                "POST",
                &endpoint_url,
                &nonce,
                iat_secs,
                None,
                sess.request_id(),
            )
            .map_err(|e| e.to_string())?;
            let mut r = DriverReply::ok();
            r.dpop = Some(dpop);
            r.body = Some(serde_json::to_value(&body).map_err(|e| e.to_string())?);
            Ok(r)
        }
        Command::ApplyAuthenticated { tokens_json } => {
            let sess = session_slot.as_mut().ok_or("NO_SESSION")?;
            let toks: SessionTokensV3 = unwrap_data(&tokens_json)?;
            sess.apply_authenticated(&toks, key)
                .map_err(|e| e.to_string())?;
            *tokens_slot = Some(toks);
            let mut r = DriverReply::ok();
            r.phase = Some(sess.phase().as_str().to_string());
            Ok(r)
        }
        Command::BuildProtectedHeaders {
            method,
            target_url,
            nonce_json,
            iat_secs,
        } => {
            let toks = tokens_slot.as_ref().ok_or("NO_TOKENS")?;
            let nonce = unwrap_data::<NonceResponse>(&nonce_json)?.nonce;
            let (auth, dpop) =
                build_protected_request_headers(key, &method, &target_url, &nonce, iat_secs, toks)
                    .map_err(|e| e.to_string())?;
            let mut r = DriverReply::ok();
            r.authorization = Some(auth);
            r.dpop = Some(dpop);
            Ok(r)
        }
        Command::BuildRefresh {
            nonce_json,
            endpoint_url,
            iat_secs,
        } => {
            let origin = origin_slot.as_ref().ok_or("NO_ORIGIN")?;
            let toks = tokens_slot.as_ref().ok_or("NO_TOKENS")?;
            let nonce = unwrap_data::<NonceResponse>(&nonce_json)?.nonce;
            let body = RefreshRequestV3 {
                origin: origin.as_str().to_string(),
                refresh_token: toks.refresh_token.clone(),
            };
            let dpop = build_dpop_proof(key, "POST", &endpoint_url, &nonce, iat_secs, None, None)
                .map_err(|e| e.to_string())?;
            let mut r = DriverReply::ok();
            r.dpop = Some(dpop);
            r.body = Some(serde_json::to_value(&body).map_err(|e| e.to_string())?);
            Ok(r)
        }
        Command::ApplyRefreshed { tokens_json } => {
            let sess = session_slot.as_mut().ok_or("NO_SESSION")?;
            let toks: SessionTokensV3 = unwrap_data(&tokens_json)?;
            sess.apply_refreshed(&toks, key)
                .map_err(|e| e.to_string())?;
            *tokens_slot = Some(toks);
            let mut r = DriverReply::ok();
            r.phase = Some(sess.phase().as_str().to_string());
            Ok(r)
        }
        Command::BuildLogout {
            nonce_json,
            endpoint_url,
            iat_secs,
        } => {
            let origin = origin_slot.as_ref().ok_or("NO_ORIGIN")?;
            let toks = tokens_slot.as_ref().ok_or("NO_TOKENS")?;
            let nonce = unwrap_data::<NonceResponse>(&nonce_json)?.nonce;
            let body = LogoutRequestV3 {
                origin: origin.as_str().to_string(),
                refresh_token: toks.refresh_token.clone(),
            };
            let dpop = build_dpop_proof(key, "POST", &endpoint_url, &nonce, iat_secs, None, None)
                .map_err(|e| e.to_string())?;
            let mut r = DriverReply::ok();
            r.dpop = Some(dpop);
            r.body = Some(serde_json::to_value(&body).map_err(|e| e.to_string())?);
            Ok(r)
        }
        Command::ApplyLoggedOut { logout_json } => {
            let sess = session_slot.as_mut().ok_or("NO_SESSION")?;
            let resp: LogoutResponseV3 = unwrap_data(&logout_json)?;
            sess.mark_logged_out(&resp).map_err(|e| e.to_string())?;
            *tokens_slot = None;
            let mut r = DriverReply::ok();
            r.phase = Some(sess.phase().as_str().to_string());
            Ok(r)
        }
    }
}
