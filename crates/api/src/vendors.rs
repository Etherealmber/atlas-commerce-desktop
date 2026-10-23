//! Vendor endpoints (purchasing side). Contract: the `/api/vendors` block
//! of `worker/index.ts` — list with bill balances, create, and update.
//! Deletes require a password confirmation header and arrive in a later
//! phase alongside the other destructive, OTP-gated endpoints.

use crate::ApiClient;
use crate::error::ApiError;
use crate::models::{Vendor, VendorId, VendorWrite};

impl ApiClient {
    /// `GET /api/vendors` — every vendor with the computed `balance_due`.
    pub async fn vendors(&self) -> Result<Vec<Vendor>, ApiError> {
        self.get_data("/api/vendors").await
    }

    /// `POST /api/vendors` — 201 with the stored row on success.
    pub async fn create_vendor(&self, body: &VendorWrite) -> Result<Vendor, ApiError> {
        self.post_data("/api/vendors", body).await
    }

    /// `PUT /api/vendors/:id` — the worker answers with `{ id }` only.
    pub async fn update_vendor(&self, id: i64, body: &VendorWrite) -> Result<VendorId, ApiError> {
        self.put_data(&format!("/api/vendors/{id}"), body).await
    }
}
