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
    pub(crate) pairing_code: Option<String>,
    pub(crate) active: bool,
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
async fn response<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
) -> Result<T, &'static str> {
    if !resp.status().is_success() {
        return Err("Le serveur a refusé la demande de connexion. Recommencez.");
    }
    let body: Envelope<T> = resp
        .json()
        .await
        .map_err(|_| "Réponse de connexion invalide.")?;
    if !body.success {
        return Err("Le serveur a refusé la demande de connexion.");
    }
    body.data.ok_or("Réponse de connexion incomplète.")
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
    let caps: Capabilities = response(cap).await?;
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
        return Err("Réponse de connexion invalide.");
    }
    let url = valid_authorization_url(
        &base,
        &created.authorization_url,
        &created.request_id,
        &state,
    )
    .ok_or("Adresse d’autorisation invalide.")?;
    let proof = serde_json::json!({"request_id":created.request_id,"origin":server,"state":state,"code_verifier":verifier});
    let deadline = Instant::now() + Duration::from_secs(created.expires_in);
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
        let polled: Poll = response(reply).await?;
        if !same_context(
            &created.request_id,
            &server,
            &state,
            &polled.request_id,
            &polled.origin,
            &polled.state,
        ) || polled.interval < 5
        {
            return Err("Réponse de connexion invalide.");
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
            _ => return Err("Réponse de connexion invalide."),
        }
    }
    Err("La demande a expiré. Recommencez.")
}

pub(crate) fn start(app: &mut AtlasApp) {
    if app.browser_login.active || app.login.busy {
        return;
    }
    let Some(base) = app.server_store.selected_url().map(str::to_owned) else {
        return;
    };
    let fingerprint = app.identity.fingerprint().to_owned();
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
pub(crate) fn cancel(app: &mut AtlasApp) {
    if let Some(flag) = &app.browser_login.cancel {
        flag.store(true, Ordering::Relaxed);
    }
}
pub(crate) fn poll(app: &mut AtlasApp) {
    if !app.browser_login.active {
        return;
    }
    if !matches!(app.screen, Screen::Login) {
        cancel(app);
    }
    if let Some(event) = app
        .browser_login
        .events
        .as_ref()
        .and_then(|rx| rx.try_recv().ok())
    {
        if matches!(app.screen, Screen::Login)
            && !app
                .browser_login
                .cancel
                .as_ref()
                .is_some_and(|v| v.load(Ordering::Relaxed))
        {
            app.browser_login.pairing_code = Some(event.pairing_code);
            if open::that(event.url.as_str()).is_err() {
                cancel(app);
                app.login.error = Some("Impossible d’ouvrir le navigateur.".to_owned());
            }
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
                app.notice("Session gardée en mémoire seulement ; reconnectez-vous après fermeture complète.".to_owned());
            }
            Err(error) => app.login.error = Some(error.to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{origin, valid_authorization_url};
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
