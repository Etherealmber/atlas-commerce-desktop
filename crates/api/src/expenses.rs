//! Expense endpoints. Contract: the `/api/expenses` block of
//! `worker/index.ts` — the list is ordered newest first, and a create
//! returns the stored row with a French confirmation message.
//!
//! Links, imports and the password-gated `DELETE` arrive in a later phase
//! with the other destructive endpoints.

use crate::ApiClient;
use crate::error::ApiError;
use crate::models::{Expense, ExpenseWrite};

impl ApiClient {
    /// `GET /api/expenses` — every expense/income row, newest date first.
    pub async fn expenses(&self) -> Result<Vec<Expense>, ApiError> {
        self.get_data("/api/expenses").await
    }

    /// `POST /api/expenses` — 201 with the stored row on success.
    pub async fn create_expense(&self, body: &ExpenseWrite) -> Result<Expense, ApiError> {
        self.post_data("/api/expenses", body).await
    }
}
