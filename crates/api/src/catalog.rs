//! Product catalog endpoints: categories and measurement units.
//! Contract: the `/api/categories` and `/api/units` blocks of the worker.
//! Deletes require a password-confirmation body and are intentionally not
//! exposed here; create and update return the stored row.

use crate::ApiClient;
use crate::error::ApiError;
use crate::models::{Category, CategoryWrite, Unit, UnitWrite};

impl ApiClient {
    /// `GET /api/categories` — table rows plus product-only virtual labels.
    pub async fn categories(&self) -> Result<Vec<Category>, ApiError> {
        self.get_data("/api/categories").await
    }

    /// `POST /api/categories` — 409 `DUPLICATE_CATEGORY` on an existing name.
    pub async fn create_category(
        &self,
        body: &CategoryWrite,
    ) -> Result<Category, ApiError> {
        self.post_data("/api/categories", body).await
    }

    /// `PUT /api/categories/:id` — renaming also re-labels the products.
    pub async fn update_category(
        &self,
        id: i64,
        body: &CategoryWrite,
    ) -> Result<Category, ApiError> {
        self.put_data(&format!("/api/categories/{id}"), body).await
    }

    /// `GET /api/units` — table rows plus product-only virtual labels.
    pub async fn units(&self) -> Result<Vec<Unit>, ApiError> {
        self.get_data("/api/units").await
    }

    /// `POST /api/units` — 409 `DUPLICATE_UNIT` on an existing name.
    pub async fn create_unit(&self, body: &UnitWrite) -> Result<Unit, ApiError> {
        self.post_data("/api/units", body).await
    }

    /// `PUT /api/units/:id` — renaming also re-labels the products.
    pub async fn update_unit(
        &self,
        id: i64,
        body: &UnitWrite,
    ) -> Result<Unit, ApiError> {
        self.put_data(&format!("/api/units/{id}"), body).await
    }
}
