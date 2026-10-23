//! Owner/admin integrations: public API keys and the AI assist provider.
//!
//! Contract (worker `index.ts`):
//! - `GET    /api/integrations/keys`        — list key metadata,
//! - `POST   /api/integrations/keys`        — create, full secret returned once,
//! - `DELETE /api/integrations/keys/:id`    — revoke,
//! - `GET    /api/settings/assist-provider` — Groq configuration state.
//!
//! The GROQ API key itself is a Worker-side secret (`wrangler secret put`);
//! no endpoint accepts it from a client, so the desktop only displays state.

use crate::ApiClient;
use crate::error::ApiError;
use crate::models::*;

impl ApiClient {
    /// `GET /api/integrations/keys` (owner/admin).
    pub async fn list_api_keys(&self) -> Result<Vec<ApiKey>, ApiError> {
        self.get_data("/api/integrations/keys").await
    }

    /// `POST /api/integrations/keys`. `name` is required and `scopes` is an
    /// explicit choice from the caller — no client-side default is sent.
    pub async fn create_api_key(
        &self,
        name: &str,
        scopes: &str,
    ) -> Result<CreatedApiKey, ApiError> {
        let body = serde_json::json!({ "name": name, "scopes": scopes });
        self.post_data("/api/integrations/keys", &body).await
    }

    /// `DELETE /api/integrations/keys/:id` — revokes the key immediately.
    pub async fn delete_api_key(&self, id: i64) -> Result<(), ApiError> {
        let path = format!("/api/integrations/keys/{id}");
        self.delete_success(&path).await
    }

    /// `GET /api/settings/assist-provider` — whether the server holds a
    /// working Groq secret.
    pub async fn assist_provider(&self) -> Result<AssistProvider, ApiError> {
        self.get_data("/api/settings/assist-provider").await
    }
}
