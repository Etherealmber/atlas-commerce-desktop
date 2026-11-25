//! Worker mobile-action inbox (`worker/routes/mobileActions.ts`).
//! The list may be cached server-side; an individual read goes to D1.
use crate::{ApiClient, ApiError};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct MobileAction {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub barcode: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<String>,
    #[serde(default)]
    pub completed_by: Option<String>,
    #[serde(default)]
    pub product_id: Option<i64>,
    #[serde(default)]
    pub meta_json: Option<String>,
}

impl ApiClient {
    /// Compact mode omits product image data from the list response.
    pub async fn mobile_actions(&self) -> Result<Vec<MobileAction>, ApiError> {
        self.get_data("/api/mobile-actions?status=all&limit=100&compact=1")
            .await
    }

    pub async fn mobile_action(&self, id: i64) -> Result<MobileAction, ApiError> {
        if id <= 0 {
            return Err(ApiError::client("Identifiant invalide."));
        }
        self.get_data(&format!("/api/mobile-actions/{id}")).await
    }

    /// Mark an action handled without applying stock or creating a document.
    /// Callers must explain this to the user and must not retry on failure.
    pub async fn complete_mobile_action(&self, id: i64) -> Result<MobileAction, ApiError> {
        if id <= 0 {
            return Err(ApiError::client("Identifiant invalide."));
        }
        self.post_data(
            &format!("/api/mobile-actions/{id}/complete"),
            &serde_json::json!({"apply_stock": false}),
        )
        .await
    }

    pub async fn reject_mobile_action(
        &self,
        id: i64,
        reason: &str,
    ) -> Result<MobileAction, ApiError> {
        let reason = reason.trim();
        if id <= 0 || reason.is_empty() || reason.chars().count() > 400 {
            return Err(ApiError::client("Identifiant ou motif invalide."));
        }
        self.post_data(
            &format!("/api/mobile-actions/{id}/reject"),
            &serde_json::json!({"reason": reason}),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::MobileAction;
    #[test]
    fn parses_compact_worker_row_with_nullable_optional_fields() {
        let item: MobileAction = serde_json::from_value(serde_json::json!({"id": 5, "type": "stock_check", "title": "Contrôle", "status": "pending", "detail": null, "product_id": null, "meta_json": "{}", "has_image": true})).unwrap();
        assert_eq!(item.kind, "stock_check");
        assert!(item.product_id.is_none());
    }
}
