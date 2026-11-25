use atlas_auth_core::{
    authorization_header_for_token, build_dpop_proof, build_protected_request_headers,
    CapabilitiesV3Response, ChallengePollResponse, ChallengePresentation, ChallengeVerifyStatus,
    CreateRequestV3Response, DesktopV3ApiEnvelope, DesktopV3AuthSession, DesktopV3Phase,
    DeviceKeyAdapter, LogoutRequestV3, LogoutResponseV3, NonceResponse, PasswordLoginResponseV3,
    RefreshRequestV3, ServerOrigin, SessionTokensV3, SoftwareP256Key,
};

#[test]
fn parses_worker_capabilities_and_enforces_full_session_lifecycle() {
    let caps_json = r#"{
        "success": true,
        "data": {
            "version": 3,
            "enabled": true,
            "methods": {
                "password_otp": {
                    "available": true,
                    "challenge_modes": ["turnstile_webview", "system_browser"],
                    "automatic_fallback": false,
                    "otp_required": true,
                    "dpop_required": true,
                    "verified_mailbox_required": true
                },
                "mailbox_enrollment": { "available": true },
                "recovery": { "available": true },
                "device_proof": { "available": true, "hardware_attestation_claimed": false }
            }
        }
    }"#;
    let caps_env: DesktopV3ApiEnvelope<CapabilitiesV3Response> =
        serde_json::from_str(caps_json).expect("capabilities envelope");
    assert!(caps_env.success);
    let caps = caps_env.data.expect("capabilities data");

    let origin = ServerOrigin::parse("https://stock.afmarbre.com").expect("valid origin");
    let key = SoftwareP256Key::generate().expect("key");
    let mut session = DesktopV3AuthSession::new(
        origin.clone(),
        ChallengePresentation::TurnstileWebview,
        "Workstation",
        &caps,
    )
    .expect("session init");

    let nonce_env: DesktopV3ApiEnvelope<NonceResponse> =
        serde_json::from_str(r#"{"success":true,"data":{"nonce":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","expires_in":120}}"#)
            .expect("nonce envelope");
    let nonce = nonce_env.data.expect("nonce data").nonce;
    let create_body = session.create_request_body().expect("create body");
    let create_proof = build_dpop_proof(
        &key,
        "POST",
        &origin
            .endpoint_url("/api/auth/desktop/v3/requests")
            .unwrap(),
        &nonce,
        1_760_000_000,
        None,
        None,
    )
    .expect("dpop");
    assert_eq!(create_proof.split('.').count(), 3);
    assert!(format!("{create_body:?}").contains("[REDACTED]"));

    let req_id = "b".repeat(64);
    let created_json = format!(
        r#"{{
            "success": true,
            "data": {{
                "version": 3,
                "request_id": "{req_id}",
                "origin": "https://stock.afmarbre.com",
                "presentation": "turnstile_webview",
                "challenge_url": "https://stock.afmarbre.com/login/desktop-v3-challenge#request_id={req_id}",
                "pairing_code": "ABCDE-12345",
                "device_thumbprint": "{}",
                "expires_in": 300,
                "interval": 5
            }}
        }}"#,
        key.thumbprint()
    );
    let created_env: DesktopV3ApiEnvelope<CreateRequestV3Response> =
        serde_json::from_str(&created_json).expect("created envelope");
    session
        .apply_created(&created_env.data.unwrap(), &key)
        .expect("apply_created");

    let poll_env: DesktopV3ApiEnvelope<ChallengePollResponse> = serde_json::from_str(&format!(
        r#"{{"success":true,"data":{{"status":"verified","request_id":"{req_id}","origin":"https://stock.afmarbre.com","interval":5}}}}"#
    ))
    .expect("poll envelope");
    let poll_status = session.apply_poll(&poll_env.data.unwrap()).expect("poll");
    assert_eq!(poll_status, ChallengeVerifyStatus::Verified);

    let synthetic_input = SoftwareP256Key::generate().unwrap().thumbprint();
    let _pw_req = session
        .build_password_login("alice", &synthetic_input)
        .expect("pw body");
    let pw_env: DesktopV3ApiEnvelope<PasswordLoginResponseV3> =
        serde_json::from_str(r#"{"success":true,"data":{"status":"otp_pending"}}"#)
            .expect("pw resp");
    assert_eq!(pw_env.data.unwrap().status, "otp_pending");
    session.mark_otp_pending().expect("otp pending");
    let _otp_req = session.build_otp_login("123456").expect("otp body");

    let tokens_json = format!(
        r#"{{
            "success": true,
            "data": {{
                "token_type": "DPoP",
                "access_token": "nv3.alice.secretaire.{}",
                "refresh_token": "{}",
                "access_expires_at": "2026-10-20T00:15:00.000Z",
                "session_expires_at": "2026-10-20T12:00:00.000Z",
                "origin": "https://stock.afmarbre.com",
                "username": "alice",
                "role": "secretaire",
                "session_id": "{}",
                "device_thumbprint": "{}",
                "attestation": "client_key_unattested"
            }}
        }}"#,
        "c".repeat(64),
        "d".repeat(64),
        "e".repeat(64),
        key.thumbprint()
    );
    let tokens_env: DesktopV3ApiEnvelope<SessionTokensV3> =
        serde_json::from_str(&tokens_json).expect("tokens");
    let tokens = tokens_env.data.unwrap();
    session
        .apply_authenticated(&tokens, &key)
        .expect("authenticated");
    assert_eq!(session.phase(), DesktopV3Phase::Authenticated);

    let (auth_hdr, dpop_hdr) = build_protected_request_headers(
        &key,
        "GET",
        "https://stock.afmarbre.com/api/auth/me",
        &nonce,
        1_760_000_010,
        &tokens,
    )
    .expect("protected headers");
    assert!(auth_hdr.starts_with("DPoP nv3."));
    assert_eq!(dpop_hdr.split('.').count(), 3);
    assert_eq!(
        authorization_header_for_token(&tokens).unwrap(),
        format!("DPoP {}", tokens.access_token)
    );

    let refreshed_json = format!(
        r#"{{
            "success": true,
            "data": {{
                "token_type": "DPoP",
                "access_token": "nv3.alice.secretaire.{}",
                "refresh_token": "{}",
                "access_expires_at": "2026-10-20T00:30:00.000Z",
                "session_expires_at": "2026-10-20T12:00:00.000Z",
                "origin": "https://stock.afmarbre.com",
                "username": "alice",
                "role": "secretaire",
                "session_id": "{}",
                "device_thumbprint": "{}"
            }}
        }}"#,
        "f".repeat(64),
        "1".repeat(64),
        "e".repeat(64),
        key.thumbprint()
    );
    let refreshed: DesktopV3ApiEnvelope<SessionTokensV3> =
        serde_json::from_str(&refreshed_json).expect("refreshed");
    let refreshed_tokens = refreshed.data.unwrap();
    session
        .apply_refreshed(&refreshed_tokens, &key)
        .expect("apply_refreshed");

    let refresh_req = RefreshRequestV3 {
        origin: origin.as_str().to_string(),
        refresh_token: tokens.refresh_token.clone(),
    };
    let logout_req = LogoutRequestV3 {
        origin: origin.as_str().to_string(),
        refresh_token: refreshed_tokens.refresh_token.clone(),
    };
    assert!(format!("{refresh_req:?}").contains("[REDACTED]"));
    assert!(format!("{logout_req:?}").contains("[REDACTED]"));

    let logout_resp: DesktopV3ApiEnvelope<LogoutResponseV3> =
        serde_json::from_str(r#"{"success":true,"data":{"status":"revoked"}}"#)
            .expect("logout resp");
    session
        .mark_logged_out(&logout_resp.data.unwrap())
        .expect("logged out");
    assert_eq!(session.phase(), DesktopV3Phase::LoggedOut);
}

#[test]
fn rejects_bearer_token_type_for_v3_sessions() {
    let key = SoftwareP256Key::generate().unwrap();
    let bad_tokens = SessionTokensV3 {
        token_type: "Bearer".to_string(),
        access_token: format!("nv3.alice.user.{}", "a".repeat(64)),
        refresh_token: "b".repeat(64),
        access_expires_at: "2026-10-20T00:15:00.000Z".to_string(),
        session_expires_at: "2026-10-20T12:00:00.000Z".to_string(),
        origin: "https://stock.afmarbre.com".to_string(),
        username: "alice".to_string(),
        role: "user".to_string(),
        session_id: "c".repeat(64),
        device_thumbprint: key.thumbprint(),
        attestation: None,
        remember_me: None,
        session_kind: None,
        refresh_expires_at: None,
        session_ttl_seconds: None,
        method: None,
    };
    assert!(authorization_header_for_token(&bad_tokens).is_err());
}
