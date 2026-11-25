//! Native desktop v2 phone approval. Proofs and request references stay in memory.
use crate::state::{AtlasApp, Screen};
use atlas_core::jobs::Job;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};
use url::Url;

#[derive(Default)]
pub(crate) struct NativeLogin {
    pub active: bool,
    pub qr_url: Option<String>,
    pub pairing_code: Option<String>,
    server: Option<String>,
    cancel: Option<Arc<AtomicBool>>,
    events: Option<mpsc::Receiver<CreatedEvent>>,
    job: Option<Job<Result<Session, &'static str>>>,
}
impl Drop for NativeLogin {
    fn drop(&mut self) {
        if let Some(flag) = &self.cancel {
            flag.store(true, Ordering::Relaxed);
        }
    }
}
#[cfg(feature = "visual-fixtures")]
pub(crate) fn fictional(server: String) -> NativeLogin {
    let base = root_origin(&server).expect("fixture origin");
    NativeLogin {
        active: true,
        server: Some(server),
        qr_url: phone_qr(&base, &"a".repeat(64)),
        pairing_code: Some("ABCDE-FGHIJ".to_owned()),
        cancel: None,
        events: None,
        job: None,
    }
}

struct CreatedEvent {
    qr_url: String,
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
    discovery_enabled: bool,
    methods: Methods,
}
#[derive(Deserialize)]
struct Methods {
    phone_approval: PhoneCapability,
}
#[derive(Deserialize)]
struct PhoneCapability {
    available: bool,
    version: Option<u32>,
    code_challenge_method: Option<String>,
    expires_in: Option<u64>,
    interval: Option<u64>,
    max_polls: Option<u64>,
    requires_phone_client: Option<bool>,
}
#[derive(Deserialize)]
struct Created {
    version: u32,
    request_id: String,
    origin: String,
    pairing_code: String,
    expires_in: u64,
    interval: u64,
}
#[derive(Deserialize)]
struct Poll {
    status: String,
    request_id: String,
    origin: String,
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
}

fn root_origin(raw: &str) -> Option<Url> {
    let url = Url::parse(raw).ok()?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    Some(url)
}
fn random_secret() -> Result<String, &'static str> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| "Impossible de générer une preuve sécurisée.")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_code(code: &str) -> bool {
    code.len() == 11
        && code.bytes().enumerate().all(|(i, c)| {
            if i == 5 {
                c == b'-'
            } else {
                c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_' || c == b'-'
            }
        })
}
fn phone_qr(base: &Url, id: &str) -> Option<String> {
    if !valid_id(id) {
        return None;
    }
    let mut url = base.clone();
    url.set_path("/login/desktop-phone");
    url.set_fragment(Some(&format!("request_id={id}")));
    Some(url.to_string())
}
fn validate_capability(cap: &Capabilities) -> bool {
    let phone = &cap.methods.phone_approval;
    cap.version == 2
        && cap.discovery_enabled
        && phone.available
        && phone.version == Some(2)
        && phone.code_challenge_method.as_deref() == Some("S256")
        && phone.expires_in.is_some_and(|n| (1..=300).contains(&n))
        && phone.interval.is_some_and(|n| (5..=300).contains(&n))
        && phone.max_polls.is_some_and(|n| (1..=60).contains(&n))
        && phone.requires_phone_client == Some(true)
}
async fn decode<T: serde::de::DeserializeOwned>(
    mut response: reqwest::Response,
) -> Result<T, &'static str> {
    if !response.status().is_success() {
        return Err(
            "Le serveur a refusé la connexion v2. Recommencez ou choisissez une autre méthode.",
        );
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("application/json")
    {
        return Err("Réponse v2 non reconnue.");
    }
    const MAX: usize = 8192;
    if response.content_length().is_some_and(|n| n > MAX as u64) {
        return Err("Réponse v2 trop volumineuse.");
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Lecture de la réponse v2 interrompue.")?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX {
            return Err("Réponse v2 trop volumineuse.");
        }
        bytes.extend_from_slice(&chunk);
    }
    let envelope: Envelope<T> =
        serde_json::from_slice(&bytes).map_err(|_| "Réponse v2 invalide.")?;
    if !envelope.success {
        return Err("Le serveur a refusé la connexion v2.");
    }
    envelope.data.ok_or("Réponse v2 incomplète.")
}
async fn cancel_request(
    client: &reqwest::Client,
    endpoint: &str,
    fingerprint: &str,
    proof: &serde_json::Value,
    deadline: Instant,
) {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return;
    }
    let _ = client
        .post(format!("{endpoint}/cancel"))
        .header("X-Device-Fingerprint", fingerprint)
        .timeout(remaining.min(Duration::from_secs(15)))
        .json(proof)
        .send()
        .await;
}
async fn run(
    base: String,
    fingerprint: String,
    cancelled: Arc<AtomicBool>,
    events: mpsc::Sender<CreatedEvent>,
) -> Result<Session, &'static str> {
    let base = root_origin(&base).ok_or("La connexion v2 exige un serveur HTTPS valide.")?;
    let origin = base.origin().ascii_serialization();
    let endpoint = format!("{origin}/api/auth/desktop/v2");
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| "Client sécurisé indisponible.")?;
    let caps: Capabilities = decode(
        client
            .get(format!("{endpoint}/capabilities"))
            .send()
            .await
            .map_err(|_| "Impossible de joindre le serveur en HTTPS.")?,
    )
    .await?;
    if !validate_capability(&caps) {
        return Err(
            "Le serveur n’annonce pas l’approbation téléphone v2. Vérifiez ses capacités et sa configuration ; vous pouvez choisir une autre méthode.",
        );
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err("Connexion annulée.");
    }
    let state = random_secret()?;
    let verifier = random_secret()?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let request = serde_json::json!({"origin":origin,"state":state,"code_challenge":challenge,"code_challenge_method":"S256","device_label":"Atlas Desktop"});
    let started = Instant::now();
    let created: Created = decode(
        client
            .post(format!("{endpoint}/requests"))
            .header("X-Device-Fingerprint", &fingerprint)
            .json(&request)
            .send()
            .await
            .map_err(|_| "Impossible de créer la demande v2.")?,
    )
    .await?;
    if created.version != 2
        || created.origin != origin
        || !valid_id(&created.request_id)
        || !valid_code(&created.pairing_code)
        || !(1..=300).contains(&created.expires_in)
        || !(5..=300).contains(&created.interval)
    {
        return Err("Réponse de création v2 invalide.");
    }
    let qr_url = phone_qr(&base, &created.request_id).ok_or("Adresse QR v2 invalide.")?;
    let deadline = started + Duration::from_secs(created.expires_in);
    let proof = serde_json::json!({"origin":origin,"request_id":created.request_id,"state":state,"code_verifier":verifier});
    if cancelled.load(Ordering::Relaxed)
        || events
            .send(CreatedEvent {
                qr_url,
                pairing_code: created.pairing_code,
            })
            .is_err()
    {
        cancel_request(&client, &endpoint, &fingerprint, &proof, deadline).await;
        return Err("Connexion annulée.");
    }
    let mut wait = Duration::from_secs(created.interval);
    for _ in 0..caps.methods.phone_approval.max_polls.unwrap_or(0) {
        let wake = (Instant::now() + wait).min(deadline);
        while Instant::now() < wake && !cancelled.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        if cancelled.load(Ordering::Relaxed) {
            cancel_request(&client, &endpoint, &fingerprint, &proof, deadline).await;
            return Err("Connexion annulée.");
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("La demande a expiré. Recommencez.");
        }
        let reply = client
            .post(format!("{endpoint}/poll"))
            .header("X-Device-Fingerprint", &fingerprint)
            .timeout(remaining.min(Duration::from_secs(15)))
            .json(&proof)
            .send()
            .await
            .map_err(|_| "Suivi v2 interrompu ; recommencez.")?;
        if cancelled.load(Ordering::Relaxed) {
            cancel_request(&client, &endpoint, &fingerprint, &proof, deadline).await;
            return Err("Connexion annulée.");
        }
        if reply.status().as_u16() == 429 {
            let seconds = reply
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5)
                .clamp(5, 300);
            wait = Duration::from_secs(seconds);
            continue;
        }
        let poll: Poll = decode(reply).await?;
        if poll.request_id != created.request_id
            || poll.origin != origin
            || !(5..=300).contains(&poll.interval)
        {
            return Err("Réponse de suivi v2 invalide.");
        }
        wait = Duration::from_secs(poll.interval);
        match poll.status.as_str() {
            "pending" => {}
            "approved" => {
                if cancelled.load(Ordering::Relaxed) {
                    cancel_request(&client, &endpoint, &fingerprint, &proof, deadline).await;
                    return Err("Connexion annulée.");
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err("La demande a expiré. Recommencez.");
                }
                // Exactly one exchange: a lost response is never retried.
                let session: Session = decode(
                    client
                        .post(format!("{endpoint}/exchange"))
                        .header("X-Device-Fingerprint", &fingerprint)
                        .timeout(remaining.min(Duration::from_secs(15)))
                        .json(&proof)
                        .send()
                        .await
                        .map_err(|_| "Échange v2 interrompu ; recommencez.")?,
                )
                .await?;
                if session.request_id != created.request_id
                    || session.origin != origin
                    || session.token_type != "Bearer"
                    || session.token.is_empty()
                    || session.username.is_empty()
                    || session.role.is_empty()
                {
                    return Err("Réponse de session v2 invalide.");
                }
                return Ok(session);
            }
            _ => return Err("La demande v2 a été refusée ou annulée."),
        }
    }
    Err("La demande a expiré. Recommencez.")
}

pub(crate) fn start(app: &mut AtlasApp) {
    if app.native_login.active || app.login.busy || app.requires_password {
        return;
    }
    let Some(base) = app.server_store.selected_url().map(str::to_owned) else {
        app.login.error = Some("Choisissez un serveur.".into());
        return;
    };
    let flag = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let job = app.runner.spawn(run(
        base.clone(),
        app.identity.fingerprint().to_owned(),
        flag.clone(),
        tx,
    ));
    app.qr = Default::default();
    app.login.error = None;
    app.login.busy = true;
    app.native_login = NativeLogin {
        active: true,
        server: Some(base),
        cancel: Some(flag),
        events: Some(rx),
        job: Some(job),
        qr_url: None,
        pairing_code: None,
    };
}
pub(crate) fn cancel(app: &mut AtlasApp) {
    if app.native_login.active {
        app.native_login = NativeLogin::default();
        app.login.busy = false;
    }
}
pub(crate) fn poll(app: &mut AtlasApp) {
    if !app.native_login.active {
        return;
    }
    if !matches!(app.screen, Screen::Login)
        || app.native_login.server.as_deref() != app.server_store.selected_url()
    {
        cancel(app);
        return;
    }
    if let Some(event) = app
        .native_login
        .events
        .as_ref()
        .and_then(|rx| rx.try_recv().ok())
    {
        app.native_login.qr_url = Some(event.qr_url);
        app.native_login.pairing_code = Some(event.pairing_code);
    }
    if let Some(result) = app.native_login.job.as_ref().and_then(Job::poll) {
        app.native_login = NativeLogin::default();
        app.login.busy = false;
        match result {
            Ok(session) => app.enter_browser_session(session.token, session.username, session.role),
            Err(error) => app.login.error = Some(error.to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn v2_qr_contains_only_request_reference_on_exact_https_origin() {
        let base = root_origin("https://example.invalid:8443").unwrap();
        let id = "a".repeat(64);
        let qr = Url::parse(&phone_qr(&base, &id).unwrap()).unwrap();
        assert_eq!(qr.origin(), base.origin());
        assert_eq!(qr.path(), "/login/desktop-phone");
        assert_eq!(qr.fragment(), Some(format!("request_id={id}").as_str()));
        assert!(qr.query().is_none());
        assert!(phone_qr(&base, "../bad").is_none());
        assert!(root_origin("http://example.invalid").is_none());
        assert!(root_origin("https://example.invalid/path").is_none());
    }
    #[test]
    fn capabilities_fail_closed_for_unavailable_or_wrong_protocol() {
        let enabled = r#"{"version":2,"discovery_enabled":true,"methods":{"phone_approval":{"available":true,"version":2,"code_challenge_method":"S256","expires_in":300,"interval":5,"max_polls":60,"requires_phone_client":true}}}"#;
        let caps: Capabilities = serde_json::from_str(enabled).unwrap();
        assert!(validate_capability(&caps));
        let disabled: Capabilities = serde_json::from_str(r#"{"version":2,"discovery_enabled":true,"methods":{"phone_approval":{"available":false,"reason":"METHOD_DISABLED"}}}"#).unwrap();
        assert!(!validate_capability(&disabled));
        let invalid: Capabilities =
            serde_json::from_str(&enabled.replace("S256", "plain")).unwrap();
        assert!(!validate_capability(&invalid));
    }

    #[test]
    fn pairing_code_has_documented_shape() {
        assert!(valid_code("ABCDE-FGHIJ"));
        assert!(valid_code("AB_DE-FG-IJ"));
        assert!(!valid_code("ABCD-EFGH"));
        assert!(!valid_code("abcde-fghij"));
    }
}
