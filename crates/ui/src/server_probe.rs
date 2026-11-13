//! Bounded, origin-preserving first-run health check for the selected worker.
//!
//! The public health route does not require a session or a device fingerprint.
//! It proves only that the configured origin answered with an Atlas-shaped
//! response, not that a user is authenticated or the service is trusted.

use std::time::{Duration, Instant};

use atlas_api::models::{Envelope, Health};
use atlas_api::probe::ProbeReport;
use atlas_core::ServerUrl;

const MAX_HEALTH_BYTES: usize = 64 * 1024;

fn failure(base: &str, elapsed: u128, message: &str) -> ProbeReport {
    ProbeReport {
        ok: false,
        base_url: base.to_owned(),
        latency_ms: elapsed,
        environment: None,
        database: false,
        storage: false,
        message: message.to_owned(),
    }
}

/// Check an installer-provided origin without initializing UI, storage,
/// credentials, or the single-instance handler. Only the success bit leaves
/// this process; the normal first-run probe still runs after installation.
#[must_use]
pub fn installer_check(raw: &str) -> bool {
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return false;
    };
    runtime.block_on(probe(raw, "Atlas-Commerce-Installer")).ok
}

/// Check only the selected origin, without credentials or redirect following.
pub(crate) async fn probe(raw: &str, user_agent: &str) -> ProbeReport {
    let Some(base) = ServerUrl::normalize(raw) else {
        return failure(
            "",
            0,
            "Adresse invalide. Indiquez une origine HTTPS sans chemin ni identifiants.",
        );
    };
    if ServerUrl::is_cleartext(&base) && !ServerUrl::is_loopback(&base) {
        return failure(&base, 0, "Connexion non sécurisée refusée. Utilisez HTTPS.");
    }
    let started = Instant::now();
    let client = match reqwest::Client::builder()
        .user_agent(user_agent)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(client) => client,
        Err(_) => return failure(&base, 0, "Client réseau indisponible."),
    };
    let mut response = match client
        .get(ServerUrl::join(&base, "/api/health"))
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let message = if error.is_timeout() {
                "Délai dépassé. Vérifiez que le serveur est disponible et réessayez."
            } else {
                "Serveur inaccessible. Vérifiez l’adresse, le certificat TLS et la connexion."
            };
            return failure(&base, started.elapsed().as_millis(), message);
        }
    };
    let status = response.status();
    if status != reqwest::StatusCode::OK {
        return failure(
            &base,
            started.elapsed().as_millis(),
            &format!(
                "Vérification refusée (HTTP {}). Vérifiez l’adresse et le serveur.",
                status.as_u16()
            ),
        );
    }
    let json = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"));
    if !json
        || response
            .content_length()
            .is_some_and(|size| size > MAX_HEALTH_BYTES as u64)
    {
        return failure(
            &base,
            started.elapsed().as_millis(),
            "Réponse de santé invalide ou trop volumineuse.",
        );
    }
    let mut bytes = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) if bytes.len().saturating_add(chunk.len()) <= MAX_HEALTH_BYTES => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(Some(_)) => {
                return failure(
                    &base,
                    started.elapsed().as_millis(),
                    "Réponse de santé trop volumineuse.",
                );
            }
            Ok(None) => break,
            Err(_) => {
                return failure(
                    &base,
                    started.elapsed().as_millis(),
                    "Lecture de la réponse interrompue.",
                );
            }
        }
    }
    let health = serde_json::from_slice::<Envelope<Health>>(&bytes)
        .ok()
        .filter(|envelope| envelope.success)
        .and_then(|envelope| envelope.data)
        .filter(|health| health.ok);
    let Some(health) = health else {
        return failure(
            &base,
            started.elapsed().as_millis(),
            "Réponse Atlas non reconnue. Vérifiez l’adresse et le serveur.",
        );
    };
    let elapsed = started.elapsed().as_millis();
    ProbeReport {
        ok: true,
        base_url: base,
        latency_ms: elapsed,
        // The environment is server-controlled and is not needed for setup.
        environment: None,
        database: health.d1,
        storage: health.r2,
        message: format!("Serveur Atlas Commerce vérifié · {elapsed} ms"),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    fn fixture(raw: Vec<u8>) -> (atlas_api::probe::ProbeReport, String) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut buffer = [0; 4096];
            let n = stream.read(&mut buffer).unwrap();
            let _ = stream.write_all(&raw);
            String::from_utf8_lossy(&buffer[..n]).into_owned()
        });
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let report = runtime.block_on(super::probe(&format!("http://{address}"), "Atlas-test"));
        (report, server.join().unwrap())
    }

    fn response(content_type: &str, body: &[u8]) -> Vec<u8> {
        let mut raw = format!("HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
        raw.extend_from_slice(body);
        raw
    }

    #[test]
    fn accepts_health_without_sending_session_or_fingerprint() {
        let (report, request) = fixture(response(
            "application/json; charset=utf-8",
            br#"{"success":true,"data":{"ok":true,"d1":true,"r2":false}}"#,
        ));
        assert!(report.ok && report.database && !report.storage);
        let request = request.to_ascii_lowercase();
        assert!(request.starts_with("get /api/health "));
        assert!(!request.contains("authorization:"));
        assert!(!request.contains("x-device-fingerprint:"));
        assert!(!request.contains("x-atlas-client:"));
    }

    #[test]
    fn rejects_redirect_html_bad_shape_and_large_responses() {
        let redirect = b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
        assert!(!fixture(redirect).0.ok);
        assert!(!fixture(response("text/html", b"<html>login</html>")).0.ok);
        assert!(
            !fixture(response(
                "application/json",
                br#"{"success":true,"data":{"ok":false}}"#
            ))
            .0
            .ok
        );
        assert!(
            !fixture(response(
                "application/json",
                &vec![b'x'; super::MAX_HEALTH_BYTES + 1]
            ))
            .0
            .ok
        );
    }

    #[test]
    fn rejects_oversized_stream_without_content_length() {
        let mut raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n10001\r\n".to_vec();
        raw.extend(std::iter::repeat_n(b'x', super::MAX_HEALTH_BYTES + 1));
        raw.extend_from_slice(b"\r\n0\r\n\r\n");
        assert!(!fixture(raw).0.ok);
    }

    #[test]
    fn rejects_remote_cleartext_before_network_access() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        assert!(
            !runtime
                .block_on(super::probe("http://shop.example.test", "Atlas-test"))
                .ok
        );
    }
}
