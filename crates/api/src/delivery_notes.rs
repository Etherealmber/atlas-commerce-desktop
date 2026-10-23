//! Delivery note (`bon de livraison`) endpoints. Contract: the
//! `/api/delivery-notes` block of `worker/index.ts`. The worker generates
//! the `BL-YYYY-NNNN` number itself and copies invoice items server-side.

use crate::ApiClient;
use crate::error::ApiError;
use crate::models::{DeliveryNote, DeliveryNoteWrite};

impl ApiClient {
    /// `GET /api/delivery-notes` — the 100 most recent notes.
    pub async fn delivery_notes(&self) -> Result<Vec<DeliveryNote>, ApiError> {
        self.get_data("/api/delivery-notes").await
    }

    /// `POST /api/delivery-notes` — 201 with the created note. Requires the
    /// business-mutator role; the worker answers 403 otherwise.
    pub async fn create_delivery_note(
        &self,
        body: &DeliveryNoteWrite,
    ) -> Result<DeliveryNote, ApiError> {
        self.post_data("/api/delivery-notes", body).await
    }
}
