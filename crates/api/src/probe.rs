//! The live server health probe used by the server-setup screen.
//!
//! Mirrors the Android `ServerProbe`: ping the public `/api/health` endpoint
//! and only accept an answer shaped like an Atlas worker, so a typo or a
//! different service is detected before the server is saved.

use atlas_core::servers::ServerUrl;

use crate::error::ApiError;
use crate::models::{Canary, Envelope, Health};

/// The probe outcome, with French guidance for every failure mode.
#[derive(Debug, Clone)]
pub struct ProbeReport {
    pub ok: bool,
    pub base_url: String,
    pub latency_ms: u128,
    pub environment: Option<String>,
    pub database: bool,
    pub storage: bool,
    pub message: String,
}

/// Probes a raw, user-typed address.
pub async fn probe(raw_url: &str, fingerprint: &str, user_agent: &str) -> ProbeReport {
    let Some(base) = ServerUrl::normalize(raw_url) else {
        return ProbeReport {
            ok: false,
            base_url: raw_url.to_owned(),
            latency_ms: 0,
            environment: None,
            database: false,
            storage: false,
            message:
                "Adresse invalide — vérifiez ce que vous avez saisi (ex. https://votre-domaine.com)."
                    .to_owned(),
        };
    };
    probe_normalized(&base, fingerprint, user_agent).await
}

/// Probes an already normalized base URL.
pub async fn probe_normalized(
    base: &str,
    fingerprint: &str,
    user_agent: &str,
) -> ProbeReport {
    let started = std::time::Instant::now();
    let client = match reqwest::Client::builder()
        .user_agent(user_agent)
        .timeout(std::time::Duration::from_secs(8))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return ProbeReport {
                ok: false,
                base_url: base.to_owned(),
                latency_ms: 0,
                environment: None,
                database: false,
                storage: false,
                message: format!("Client HTTP impossible : {error}"),
            }
        }
    };
    let url = ServerUrl::join(base, "api/health");
    let result = client
        .get(&url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header("X-Atlas-Client", "native")
        .header("X-Device-Fingerprint", fingerprint)
        .send()
        .await;
    let elapsed = started.elapsed().as_millis();
    match result {
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if !status.is_success() {
                return ProbeReport {
                    ok: false,
                    base_url: base.to_owned(),
                    latency_ms: elapsed,
                    environment: None,
                    database: false,
                    storage: false,
                    message: match status.as_u16() {
                        404 => "Aucune API Atlas trouvée à cette adresse (404). Vérifiez l'URL exacte et le déploiement du worker.".to_owned(),
                        429 => "Serveur joignable mais temporairement limité (429). Patientez puis réessayez.".to_owned(),
                        code if (500..600).contains(&code) =>
                            "Site temporairement indisponible (erreur 5xx). Vérifiez votre déploiement.".to_owned(),
                        _ => format!("Connexion refusée ({code}). Vérifiez l'adresse saisie."),
                    },
                };
            }
            match serde_json::from_str::<Envelope<Health>>(&body) {
                Ok(envelope) if envelope.success && envelope.data.as_ref().is_some_and(|health| health.ok) => {
                    let health = envelope.data.expect("checked");
                    ProbeReport {
                        ok: true,
                        base_url: base.to_owned(),
                        latency_ms: elapsed,
                        environment: health.environment,
                        database: health.d1,
                        storage: health.r2,
                        message: format!("Serveur Atlas Commerce vérifié · {elapsed} ms"),
                    }
                }
                _ => ProbeReport {
                    ok: false,
                    base_url: base.to_owned(),
                    latency_ms: elapsed,
                    environment: None,
                    database: false,
                    storage: false,
                    message: "Ce site ne répond pas comme un espace Atlas Commerce. Vérifiez l'adresse et l'installation d'Atlas Commerce.".to_owned(),
                },
            }
        }
        Err(error) => ProbeReport {
            ok: false,
            base_url: base.to_owned(),
            latency_ms: elapsed,
            environment: None,
            database: false,
            storage: false,
            message: if error.is_timeout() {
                "Délai d'attente dépassé — le serveur est peut-être hors ligne.".to_owned()
            } else if error.is_connect() {
                "Site introuvable ou injoignable — vérifiez l'adresse et votre connexion.".to_owned()
            } else {
                "Site ou serveur indisponible. Vérifiez l'adresse saisie.".to_owned()
            },
        },
    }
}

/// Lightweight `/api/canary` check (deploy smoke), used by the settings
/// screen's diagnostics panel.
pub async fn canary(client: &ApiClient) -> Result<Canary, ApiError> {
    client.get_data("/api/canary").await
}
