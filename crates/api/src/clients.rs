//! Client (CRM) endpoints. The worker contract lives in
//! `worker/routes/clients.ts`: list with computed balances, create and
//! partial update and session-confirmed deletion with server-side blockers.

use crate::ApiClient;
use crate::error::ApiError;
use crate::models::{Client, ClientWrite};

#[derive(Debug, serde::Deserialize)]
pub struct ClientDeletion {
    pub success: bool,
    pub message: Option<String>,
    pub error: Option<String>,
    pub code: Option<String>,
}

impl ApiClient {
    /// `GET /api/clients` — every client with the computed `balance_due`.
    pub async fn clients(&self) -> Result<Vec<Client>, ApiError> {
        self.get_data("/api/clients").await
    }

    /// `POST /api/clients` — 201 with the stored row on success.
    pub async fn create_client(&self, body: &ClientWrite) -> Result<Client, ApiError> {
        self.post_data("/api/clients", body).await
    }

    /// `DELETE /api/clients/:id`. The web confirmation uses an empty
    /// password; the server checks session, role and transaction blockers.
    pub async fn delete_client(&self, id: i64) -> Result<ClientDeletion, ApiError> {
        let response = self
            .request(reqwest::Method::DELETE, &format!("/api/clients/{id}"))
            .json(&serde_json::json!({"confirm_password": ""}))
            .send()
            .await
            .map_err(|_| ApiError::transport("Échec de la connexion au serveur."))?;
        let status = response.status();
        if !status.is_success() {
            return Err(crate::error::response_into_error(response).await);
        }
        let result: ClientDeletion = response
            .json()
            .await
            .map_err(|_| ApiError::client("Réponse de suppression illisible."))?;
        if !result.success {
            return Err(ApiError {
                status: status.as_u16(),
                code: result.code,
                message: result
                    .error
                    .unwrap_or_else(|| "Suppression refusée par le serveur.".into()),
            });
        }
        Ok(result)
    }

    /// `PUT /api/clients/:id` — the worker applies only present fields.
    pub async fn update_client(&self, id: i64, body: &ClientWrite) -> Result<Client, ApiError> {
        self.put_data(&format!("/api/clients/{id}"), body).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    #[test]
    fn deletion_uses_session_confirmation_and_preserves_server_blockers() {
        for blocked in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut raw = Vec::new();
                let end = loop {
                    let mut bytes = [0; 1024];
                    let n = stream.read(&mut bytes).unwrap();
                    assert!(n > 0);
                    raw.extend_from_slice(&bytes[..n]);
                    if let Some(i) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&raw[..end]).to_lowercase();
                assert!(headers.starts_with("delete /api/clients/7 http/1.1"));
                assert!(headers.contains("authorization: bearer fixture-token"));
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse().ok())
                    })
                    .unwrap();
                while raw.len() - end < length {
                    let mut bytes = [0; 1024];
                    let n = stream.read(&mut bytes).unwrap();
                    assert!(n > 0);
                    raw.extend_from_slice(&bytes[..n]);
                }
                let body: serde_json::Value =
                    serde_json::from_slice(&raw[end..end + length]).unwrap();
                assert_eq!(body["confirm_password"], "");
                let (status, body) = if blocked {
                    (
                        "400 Bad Request",
                        r#"{"success":false,"code":"CLIENT_HAS_HISTORY","error":"Historique lié"}"#,
                    )
                } else {
                    ("200 OK", r#"{"success":true,"message":"Client supprimé"}"#)
                };
                write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            });
            let client =
                ApiClient::new(&format!("http://{address}"), "fixture", "fixture").unwrap();
            client.set_token(Some("fixture-token".into()));
            let result = atlas_core::jobs::JobRunner::new()
                .spawn(async move { client.delete_client(7).await })
                .wait()
                .unwrap();
            if blocked {
                assert_eq!(
                    result.unwrap_err().code.as_deref(),
                    Some("CLIENT_HAS_HISTORY")
                );
            } else {
                assert_eq!(result.unwrap().message.as_deref(), Some("Client supprimé"));
            }
            server.join().unwrap();
        }
    }
}
