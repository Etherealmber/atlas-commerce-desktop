//! Atlas Agent IA — one persistent conversation per account.

use crate::error::ApiError;
use crate::models::{AgentChat, AgentPolicy, AgentReply};
use crate::ApiClient;

impl ApiClient {
    /// `GET /api/agent/policy` — quota and block state.
    pub async fn agent_policy(&self) -> Result<AgentPolicy, ApiError> {
        self.get_data("/api/agent/policy").await
    }

    /// `POST /api/assist/chat` — one turn of the persistent `live` thread.
    /// The worker keeps the recent history server-side keyed by thread id.
    pub async fn agent_chat(
        &self,
        message: &str,
        thread_id: &str,
    ) -> Result<AgentReply, ApiError> {
        let request = AgentChat {
            message: message.to_owned(),
            user_message: message.to_owned(),
            task: "general".to_owned(),
            timezone: iana_timezone(),
            thread_id: thread_id.to_owned(),
            source: "desktop".to_owned(),
        };
        self.post_data("/api/assist/chat", &request).await
    }
}

/// Best-effort IANA timezone, defaulting to the platform's Moroccan default.
fn iana_timezone() -> String {
    std::env::var("TZ").unwrap_or_else(|_| "Africa/Casablanca".to_owned())
}
