//! Client (CRM) endpoints. The worker contract lives in
//! `worker/routes/clients.ts`: list with computed balances, create and
//! partial update; deletes are OTP-gated and handled in a later phase.

use crate::ApiClient;
use crate::error::ApiError;
use crate::models::{Client, ClientWrite};

impl ApiClient {
    /// `GET /api/clients` — every client with the computed `balance_due`.
    pub async fn clients(&self) -> Result<Vec<Client>, ApiError> {
        self.get_data("/api/clients").await
    }

    /// `POST /api/clients` — 201 with the stored row on success.
    pub async fn create_client(&self, body: &ClientWrite) -> Result<Client, ApiError> {
        self.post_data("/api/clients", body).await
    }

    /// `PUT /api/clients/:id` — the worker applies only present fields.
    pub async fn update_client(&self, id: i64, body: &ClientWrite) -> Result<Client, ApiError> {
        self.put_data(&format!("/api/clients/{id}"), body).await
    }
}
