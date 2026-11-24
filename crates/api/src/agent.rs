//! Atlas Agent IA — one persistent conversation per account.

use crate::ApiClient;
use crate::error::ApiError;
use crate::models::{AgentChat, AgentPolicy, AgentReply, AgentThread, AgentThreadSummary};

impl ApiClient {
    /// `GET /api/agent/policy` — quota and block state.
    pub async fn agent_policy(&self) -> Result<AgentPolicy, ApiError> {
        self.get_data("/api/agent/policy").await
    }

    /// Lists only conversations belonging to the authenticated account.
    pub async fn agent_threads(&self) -> Result<Vec<AgentThreadSummary>, ApiError> {
        self.get_data("/api/admin/agent/threads").await
    }

    /// Load one server thread. Reject path delimiters from untrusted list data.
    pub async fn agent_thread(&self, id: &str) -> Result<AgentThread, ApiError> {
        if !valid_thread_id(id) {
            return Err(ApiError::client("Identifiant de conversation invalide."));
        }
        self.get_data(&format!("/api/admin/agent/threads/{id}"))
            .await
    }

    /// `POST /api/assist/chat` — one turn of the persistent `live` thread.
    /// The worker keeps the recent history server-side keyed by thread id.
    pub async fn agent_chat(
        &self,
        message: &str,
        thread_id: &str,
        language: &str,
    ) -> Result<AgentReply, ApiError> {
        let request = AgentChat {
            message: message.to_owned(),
            user_message: message.to_owned(),
            task: "general".to_owned(),
            timezone: iana_timezone(),
            thread_id: thread_id.to_owned(),
            source: "desktop".to_owned(),
            response_language: language.to_owned(),
        };
        self.post_data("/api/assist/chat", &request).await
    }
}

fn valid_thread_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        && id != "."
        && id != ".."
}

/// Best-effort IANA timezone, defaulting to the platform's Moroccan default.
fn iana_timezone() -> String {
    std::env::var("TZ").unwrap_or_else(|_| "Africa/Casablanca".to_owned())
}

#[cfg(test)]
mod tests {
    use crate::models::AgentReply;

    #[test]
    fn thread_ids_cannot_change_the_request_path() {
        assert!(super::valid_thread_id("desktop-1234"));
        for id in ["", "..", "a/b", "%2f", "a?x=1", "a#b"] {
            assert!(!super::valid_thread_id(id));
        }
    }

    #[test]
    fn keeps_server_proposals_and_tool_status() {
        let reply: AgentReply = serde_json::from_str(r#"{"reply":"Voici une proposition", "proposals":[{"proposal_id":"abc"}], "tool_trace":[{"name":"propose_inventory_adjustment","ok":true}]}"#).unwrap();
        assert_eq!(reply.proposals.len(), 1);
        assert!(reply.tool_trace[0].ok);
        assert_eq!(reply.tool_trace[0].name, "propose_inventory_adjustment");
    }
}
