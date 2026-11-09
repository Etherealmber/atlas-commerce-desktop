//! Browser authorization v1. Proofs live only in the running process.
//! Requests never follow redirects, and server-provided URLs are validated.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

use atlas_core::jobs::Job;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

use crate::state::{AtlasApp, Screen};

#[derive(Default)]
pub(crate) struct BrowserLogin {
    job: Option<Job<Result<Session, &'static str>>>,
    events: Option<mpsc::Receiver<Event>>,
    cancel: Option<Arc<AtomicBool>>,
    server: Option<String>,
    pub(crate) pairing_code: Option<String>,
    pub(crate) active: bool,
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    pub(crate) embedded: bool,
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    pub(crate) pending_url: Option<Url>,
}

struct Event {
    url: Url,
    pairing_code: String,
}
#[derive(Deserialize)]
struct Envelope<T> {
    success: bool,
    data: Option<T>,
}
#[derive(Deserialize)]
struct Capabilities {
    version: u32,
    flow: String,
    code_challenge_method: String,
    callbacks: bool,
}
#[derive(Deserialize)]
struct Created {
    version: u32,
    request_id: String,
    origin: String,
    state: String,
    authorization_url: String,
    pairing_code: String,
    expires_in: u64,
    expires_at: String,
    interval: u64,
}
#[derive(Deserialize)]
struct Poll {
    status: String,
    request_id: String,
    origin: String,
    state: String,
    interval: u64,
}
#[derive(Deserialize)]
struct Session {
    token: String,
    token_type: String,
    username: String,
    role: String,
    request_id: String,
    origin: String,
    state: String,
}

fn secret() -> Result<String, &'static str> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|_| "Impossible de générer une preuve sécurisée.")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn origin(base: &str) -> Option<Url> {
    let url = Url::parse(base).ok()?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return None;
    }
    Some(url)
}
fn valid_authorization_url(base: &Url, raw: &str, id: &str, state: &str) -> Option<Url> {
    let url = Url::parse(raw).ok()?;
    if url.origin() != base.origin()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/login/desktop"
        || url.query().is_some()
    {
        return None;
    }
    let parts: Vec<_> = url::form_urlencoded::parse(url.fragment()?.as_bytes()).collect();
    if parts.len() != 2
        || parts
            .iter()
            .filter(|(k, v)| k == "request_id" && v == id)
            .count()
            != 1
        || parts
            .iter()
            .filter(|(k, v)| k == "state" && v == state)
            .count()
            != 1
    {
        return None;
    }
    Some(url)
}
fn same_context(
    id: &str,
    server: &str,
    state: &str,
    got_id: &str,
    got_origin: &str,
    got_state: &str,
) -> bool {
    id == got_id && server == got_origin && state == got_state
}
/// Identifies protocol stages without logging URLs, request bodies or proofs.
#[derive(Clone, Copy)]
enum Stage {
    Capabilities,
    Create,
    Poll,
    Exchange,
}

impl Stage {
    fn label(self) -> &'static str {
        match self {
            Self::Capabilities => "capabilities",
            Self::Create => "requests",
            Self::Poll => "poll",
            Self::Exchange => "exchange",
        }
    }
}

fn decode_envelope<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, &'static str> {
    let body: Envelope<T> = serde_json::from_slice(bytes)
        .map_err(|_| "Réponse de connexion invalide (format inattendu).")?;
    if !body.success {
        return Err("Le serveur a refusé la demande de connexion.");
    }
    body.data.ok_or("Réponse de connexion incomplète.")
}

async fn response<T: serde::de::DeserializeOwned>(
    mut resp: reqwest::Response,
    stage: Stage,
) -> Result<T, &'static str> {
    let status = resp.status();
    // Do not log the header itself: it can contain arbitrary server input.
    let content_kind = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        });
    let content_kind = match content_kind.as_deref() {
        Some("application/json") => "json",
        Some("text/html") => "html",
        Some(_) => "other",
        None => "missing",
    };
    if !status.is_success() {
        log::warn!(
            "Authentification desktop: étape={}, statut={}, contenu={}, catégorie=http",
            stage.label(),
            status.as_u16(),
            content_kind
        );
        return Err("Le serveur a refusé la demande de connexion. Recommencez.");
    }
    const MAX_BODY: usize = 8192;
    if resp
        .content_length()
        .is_some_and(|size| size > MAX_BODY as u64)
    {
        log::warn!(
            "Authentification desktop: étape={}, statut={}, contenu={}, catégorie=taille",
            stage.label(),
            status.as_u16(),
            content_kind
        );
        return Err("Réponse de connexion trop volumineuse.");
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|_| "Réponse de connexion illisible.")?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_BODY {
            log::warn!(
                "Authentification desktop: étape={}, statut={}, contenu={}, catégorie=taille",
                stage.label(),
                status.as_u16(),
                content_kind
            );
            return Err("Réponse de connexion trop volumineuse.");
        }
        bytes.extend_from_slice(&chunk);
    }
    decode_envelope(&bytes).inspect_err(|_| {
        log::warn!(
            "Authentification desktop: étape={}, statut={}, contenu={}, catégorie=enveloppe",
            stage.label(),
            status.as_u16(),
            content_kind
        );
    })
}

async fn run(
    base: String,
    fingerprint: String,
    cancelled: Arc<AtomicBool>,
    events: mpsc::Sender<Event>,
) -> Result<Session, &'static str> {
    let base = origin(&base).ok_or("La connexion navigateur exige un serveur HTTPS valide.")?;
    let server = base.origin().ascii_serialization();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| "Impossible de créer le client sécurisé.")?;
    let endpoint = |path: &str| format!("{server}/api/auth/desktop/v1/{path}");
    let cap = client
        .get(endpoint("capabilities"))
        .send()
        .await
        .map_err(|_| "Impossible de joindre le serveur en HTTPS.")?;
    if matches!(cap.status().as_u16(), 401 | 404) {
        return Err(
            "Ce serveur ne prend pas en charge la connexion navigateur v1. Utilisez le mot de passe ou le QR.",
        );
    }
    let caps: Capabilities = response(cap, Stage::Capabilities).await?;
    if caps.version != 1
        || caps.flow != "system_browser_pkce_polling"
        || caps.code_challenge_method != "S256"
        || caps.callbacks
    {
        return Err("Ce serveur ne prend pas en charge le protocole attendu.");
    }
    let state = secret()?;
    let verifier = secret()?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let req = serde_json::json!({"origin":server,"state":state,"code_challenge":challenge,"code_challenge_method":"S256","device_label":"Atlas Desktop"});
    let created: Created = response(
        client
            .post(endpoint("requests"))
            .header("X-Device-Fingerprint", &fingerprint)
            .json(&req)
            .send()
            .await
            .map_err(|_| "Impossible de créer la demande.")?,
        Stage::Create,
    )
    .await?;
    if created.version != 1
        || created.origin != server
        || created.state != state
        || created.request_id.len() != 64
        || !created.request_id.bytes().all(|b| b.is_ascii_hexdigit())
        || created.expires_in == 0
        || created.expires_in > 300
        || created.interval < 5
        || created.interval > 300
        || created.pairing_code.len() != 9
        || !created.pairing_code.bytes().enumerate().all(|(i, b)| {
            if i == 4 {
                b == b'-'
            } else {
                b.is_ascii_uppercase() || b.is_ascii_digit()
            }
        })
    {
        log::warn!("Authentification desktop: étape=requests, catégorie=contexte");
        return Err("Réponse de connexion invalide (création de la demande).");
    }
    let url = valid_authorization_url(
        &base,
        &created.authorization_url,
        &created.request_id,
        &state,
    )
    .ok_or("Adresse d’autorisation invalide.")?;
    let proof = serde_json::json!({"request_id":created.request_id,"origin":server,"state":state,"code_verifier":verifier});
    let expires = chrono::DateTime::parse_from_rfc3339(&created.expires_at)
        .map_err(|_| "Expiration de connexion invalide.")?;
    let remaining = expires
        .signed_duration_since(chrono::Utc::now())
        .num_seconds();
    if remaining <= 0 {
        return Err("La demande a expiré. Recommencez.");
    }
    let deadline = Instant::now() + Duration::from_secs((remaining as u64).min(created.expires_in));
    if events
        .send(Event {
            url,
            pairing_code: created.pairing_code,
        })
        .is_err()
    {
        cancelled.store(true, Ordering::Relaxed);
    }
    let mut wait = Duration::from_secs(created.interval);
    for _ in 0..60 {
        let wake = (Instant::now() + wait).min(deadline);
        while Instant::now() < wake && !cancelled.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        if cancelled.load(Ordering::Relaxed) {
            let _ = client
                .post(endpoint("cancel"))
                .header("X-Device-Fingerprint", &fingerprint)
                .json(&proof)
                .send()
                .await;
            return Err("Connexion annulée.");
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("La demande a expiré. Recommencez.");
        }
        let reply = client
            .post(endpoint("poll"))
            .header("X-Device-Fingerprint", &fingerprint)
            .timeout(remaining.min(Duration::from_secs(15)))
            .json(&proof)
            .send()
            .await
            .map_err(|_| "Connexion interrompue ; recommencez.")?;
        if reply.status().as_u16() == 429 {
            let seconds = reply
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5)
                .max(5);
            wait = Duration::from_secs(seconds.min(300));
            continue;
        }
        let polled: Poll = response(reply, Stage::Poll).await?;
        if !same_context(
            &created.request_id,
            &server,
            &state,
            &polled.request_id,
            &polled.origin,
            &polled.state,
        ) || polled.interval < 5
        {
            log::warn!("Authentification desktop: étape=poll, catégorie=contexte");
            return Err("Réponse de connexion invalide (suivi de la demande).");
        }
        wait = Duration::from_secs(polled.interval.min(300));
        match polled.status.as_str() {
            "pending" => {}
            "approved" => {
                if cancelled.load(Ordering::Relaxed) {
                    return Err("Connexion annulée.");
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err("La demande a expiré. Recommencez.");
                }
                let session: Session = response(
                    client
                        .post(endpoint("exchange"))
                        .header("X-Device-Fingerprint", &fingerprint)
                        .timeout(remaining.min(Duration::from_secs(15)))
                        .json(&proof)
                        .send()
                        .await
                        .map_err(|_| "Échange interrompu ; recommencez.")?,
                    Stage::Exchange,
                )
                .await?;
                if !same_context(
                    &created.request_id,
                    &server,
                    &state,
                    &session.request_id,
                    &session.origin,
                    &session.state,
                ) || session.token_type != "Bearer"
                    || session.token.is_empty()
                    || session.username.is_empty()
                    || session.role.is_empty()
                {
                    return Err("Réponse de session invalide.");
                }
                return Ok(session);
            }
            "denied" => return Err("Connexion refusée dans le navigateur."),
            "cancelled" => return Err("Connexion annulée."),
            _ => {
                log::warn!("Authentification desktop: étape=poll, catégorie=statut_inconnu");
                return Err("Réponse de connexion invalide (statut inattendu).");
            }
        }
    }
    Err("La demande a expiré. Recommencez.")
}

pub(crate) fn start(app: &mut AtlasApp) {
    start_with_mode(app, false);
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(crate) fn start_embedded(app: &mut AtlasApp) {
    start_with_mode(app, true);
}

fn start_with_mode(app: &mut AtlasApp, embedded: bool) {
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    let _ = embedded;
    if app.browser_login.active || app.login.busy {
        return;
    }
    let Some(base) = app.server_store.selected_url().map(str::to_owned) else {
        return;
    };
    let fingerprint = app.identity.fingerprint().to_owned();
    app.browser_login.server = Some(base.clone());
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    {
        app.browser_login.embedded = embedded;
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    app.qr = Default::default();
    app.login.error = None;
    app.login.busy = true;
    app.browser_login.active = true;
    app.browser_login.cancel = Some(cancelled.clone());
    app.browser_login.events = Some(rx);
    app.browser_login.job = Some(app.runner.spawn(run(base, fingerprint, cancelled, tx)));
}
/// Keep the existing v1 request and polling, but finish it in the system
/// browser when the embedded engine cannot start. The URL was validated by
/// the host before this call; it never comes from a navigation event.
/// Policy failures must not be opened in the browser: only host failures
/// fall back. Both messages are produced before any hosted page is loaded.
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(crate) fn fallback_allowed(message: &str) -> bool {
    !matches!(
        message,
        "Adresse d’autorisation invalide." | "Serveur HTTPS invalide."
    )
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(crate) fn fallback_to_browser(app: &mut AtlasApp, url: Url, message: &str) -> bool {
    app.browser_login.embedded = false;
    app.browser_login.pending_url = None;
    if open::that(url.as_str()).is_err() {
        cancel(app);
        app.login.error = Some("Impossible d’ouvrir le navigateur.".to_owned());
        false
    } else {
        app.login.error = Some(message.to_owned());
        true
    }
}

pub(crate) fn cancel(app: &mut AtlasApp) {
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    {
        app.browser_login.pending_url = None;
    }
    if let Some(flag) = &app.browser_login.cancel {
        flag.store(true, Ordering::Relaxed);
    }
}
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(crate) fn embedded_active(app: &AtlasApp) -> bool {
    app.browser_login.active
        && app.browser_login.embedded
        && matches!(app.screen, Screen::Login)
        && app.browser_login.server.as_deref() == app.server_store.selected_url()
        && !app
            .browser_login
            .cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
}

pub(crate) fn poll(app: &mut AtlasApp) {
    if !app.browser_login.active {
        return;
    }
    if !matches!(app.screen, Screen::Login)
        || app.browser_login.server.as_deref() != app.server_store.selected_url()
    {
        cancel(app);
    }
    if let Some(event) = app
        .browser_login
        .events
        .as_ref()
        .and_then(|rx| rx.try_recv().ok())
        && matches!(app.screen, Screen::Login)
        && !app
            .browser_login
            .cancel
            .as_ref()
            .is_some_and(|v| v.load(Ordering::Relaxed))
    {
        app.browser_login.pairing_code = Some(event.pairing_code);
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
        if app.browser_login.embedded {
            app.browser_login.pending_url = Some(event.url);
        } else if open::that(event.url.as_str()).is_err() {
            cancel(app);
            app.login.error = Some("Impossible d’ouvrir le navigateur.".to_owned());
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        if open::that(event.url.as_str()).is_err() {
            cancel(app);
            app.login.error = Some("Impossible d’ouvrir le navigateur.".to_owned());
        }
    }
    if let Some(result) = app.browser_login.job.as_ref().and_then(Job::poll) {
        let was_cancelled = app
            .browser_login
            .cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed));
        app.browser_login = BrowserLogin::default();
        app.login.busy = false;
        if !matches!(app.screen, Screen::Login) || was_cancelled {
            return;
        }
        match result {
            Ok(session) => {
                app.enter_browser_session(session.token, session.username, session.role);
            }
            Err(error) => app.login.error = Some(error.to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Capabilities, Created, Envelope, Poll, Session, decode_envelope, origin, same_context,
        secret, valid_authorization_url,
    };
    // A local fixture exercises the actual HTTP response handling without
    // contacting a real server or weakening the HTTPS requirement in run().
    fn fixture_response(raw: Vec<u8>) -> Result<Poll, &'static str> {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            // A rejection may stop reading before the whole body is sent.
            let _ = stream.write_all(&raw);
        });
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(async {
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap();
            let resp = client
                .get(format!("http://{address}/"))
                .send()
                .await
                .unwrap();
            super::response(resp, super::Stage::Poll).await
        });
        server.join().unwrap();
        result
    }

    #[test]
    fn http_response_rejects_redirects_proxy_pages_and_oversized_bodies() {
        let redirect = b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
        assert_eq!(
            fixture_response(redirect).err(),
            Some("Le serveur a refusé la demande de connexion. Recommencez.")
        );
        let page = b"<html>private proxy page</html>";
        let mut html = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", page.len()).into_bytes();
        html.extend_from_slice(page);
        assert_eq!(
            fixture_response(html).err(),
            Some("Réponse de connexion invalide (format inattendu).")
        );
        let mut large = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n2001\r\n".to_vec();
        large.extend(std::iter::repeat_n(b'x', 8193));
        large.extend_from_slice(b"\r\n0\r\n\r\n");
        assert_eq!(
            fixture_response(large).err(),
            Some("Réponse de connexion trop volumineuse.")
        );
    }

    #[test]
    fn http_response_accepts_only_a_valid_poll_envelope() {
        let body = br#"{"success":true,"data":{"status":"pending","request_id":"id","origin":"https://example.test","state":"state","interval":5}}"#;
        let mut raw = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
        raw.extend_from_slice(body);
        let poll = fixture_response(raw).ok().unwrap();
        assert_eq!(poll.status, "pending");
        assert_eq!(poll.interval, 5);
    }

    #[test]
    fn v1_envelopes_do_not_confuse_poll_or_otp_with_desktop_session() {
        // These are distinct worker contracts; no token is accepted outside
        // the exchange envelope or from the legacy web-login root.
        let poll = r#"{"success":true,"data":{"status":"pending","request_id":"id","origin":"https://example.test","state":"state","interval":5}}"#;
        let parsed: Envelope<Poll> = serde_json::from_str(poll).unwrap();
        assert_eq!(parsed.data.unwrap().status, "pending");
        assert!(serde_json::from_str::<Envelope<Session>>(poll).is_err());
        let otp = r#"{"success":true,"needs_otp":true}"#;
        assert!(
            serde_json::from_str::<Envelope<Session>>(otp)
                .unwrap()
                .data
                .is_none()
        );
        for body in [
            "",
            "<html>proxy</html>",
            "{",
            r#"{"success":true,"data":{"token":"x"}}"#,
        ] {
            assert!(serde_json::from_str::<Envelope<Session>>(body).is_err());
        }
        let valid = r#"{"success":true,"data":{"token":"opaque","token_type":"Bearer","username":"test","role":"owner","request_id":"id","origin":"https://example.test","state":"state"}}"#;
        let session: Envelope<Session> = serde_json::from_str(valid).unwrap();
        assert_eq!(session.data.unwrap().token, "opaque");
        assert!(
            serde_json::from_str::<Envelope<Capabilities>>(
                r#"{"success":true,"data":{"version":"1"}}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<Envelope<Created>>(r#"{"success":true,"data":{}}"#).is_err()
        );
    }

    #[test]
    fn envelope_errors_are_fixed_and_do_not_expose_response_content() {
        for body in [b"<html>private proxy response</html>".as_slice(), b"", b"{"] {
            assert_eq!(
                decode_envelope::<Poll>(body).err(),
                Some("Réponse de connexion invalide (format inattendu).")
            );
        }
        assert_eq!(
            decode_envelope::<Poll>(br#"{"success":false,"error":"private"}"#).err(),
            Some("Le serveur a refusé la demande de connexion.")
        );
        assert_eq!(
            decode_envelope::<Poll>(br#"{"success":true}"#).err(),
            Some("Réponse de connexion incomplète.")
        );
    }

    #[test]
    fn proofs_are_random_url_safe_and_context_is_bound() {
        let a = secret().unwrap();
        let b = secret().unwrap();
        assert_eq!(a.len(), 43);
        assert_ne!(a, b);
        assert!(
            a.bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        );
        assert!(same_context(
            "id",
            "https://server",
            &a,
            "id",
            "https://server",
            &a
        ));
        assert!(!same_context(
            "id",
            "https://server",
            &a,
            "id",
            "https://other",
            &a
        ));
        assert!(!same_context(
            "id",
            "https://server",
            &a,
            "id",
            "https://server",
            &b
        ));
    }

    #[test]
    fn authorization_url_is_bound_to_selected_server_and_proof() {
        let base = origin("https://self.example:8443/").unwrap();
        assert!(
            valid_authorization_url(
                &base,
                "https://self.example:8443/login/desktop#request_id=abc&state=xyz",
                "abc",
                "xyz"
            )
            .is_some()
        );
        for raw in [
            "http://self.example:8443/login/desktop#request_id=abc&state=xyz",
            "https://evil.example/login/desktop#request_id=abc&state=xyz",
            "https://self.example:8443/login/desktop?x=1#request_id=abc&state=xyz",
            "https://self.example:8443/login/desktop#request_id=abc&state=bad",
            "https://self.example:8443/login/desktop#request_id=abc&state=xyz&next=bad",
        ] {
            assert!(valid_authorization_url(&base, raw, "abc", "xyz").is_none());
        }
        assert!(origin("http://self.example/").is_none());
        assert!(origin("https://self.example/path").is_none());
    }
}
