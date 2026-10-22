//! Commerce endpoints: products, stock movements, dashboard, sales and
//! alerts. Query parameters follow the worker and the Android client.

use crate::ApiClient;
use crate::error::ApiError;
use crate::models::*;

impl ApiClient {
    /// `GET /api/products` — the mobile/light field set, capped like Android.
    pub async fn products(&self, query: &str, limit: u32) -> Result<Vec<Product>, ApiError> {
        let path = format!(
            "/api/products?q={}&limit={limit}&fields=mobile",
            urlencoding(query)
        );
        self.get_data(&path).await
    }

    /// `GET /api/products/:id`.
    pub async fn product(&self, id: i64) -> Result<Product, ApiError> {
        self.get_data(&format!("/api/products/{id}")).await
    }

    /// `POST /api/products`. A 409 `DUPLICATE_PRODUCT` carries the French
    /// worker message verbatim; callers may retry with `force=true`.
    pub async fn create_product(
        &self,
        product: &CreateProduct,
        force_duplicate: bool,
    ) -> Result<Product, ApiError> {
        let path = if force_duplicate {
            "/api/products?force_duplicate=true"
        } else {
            "/api/products"
        };
        self.post_data(path, product).await
    }

    /// `POST /api/products/:id/adjust-stock`.
    pub async fn adjust_stock(&self, id: i64, adjust: &StockAdjust) -> Result<Product, ApiError> {
        self.post_data(&format!("/api/products/{id}/adjust-stock"), adjust)
            .await
    }

    /// `GET /api/dashboard`.
    pub async fn dashboard(&self) -> Result<Dashboard, ApiError> {
        self.get_data("/api/dashboard").await
    }

    /// `GET /api/invoices?kind=…` — sales and tickets.
    pub async fn invoices(&self, kind: &str) -> Result<Vec<Invoice>, ApiError> {
        self.get_data(&format!("/api/invoices?kind={kind}")).await
    }

    /// `POST /api/invoices` with an idempotency key, for quick sales.
    pub async fn create_quick_sale(
        &self,
        sale: &QuickSale,
        idempotency_key: &str,
    ) -> Result<CreatedInvoice, ApiError> {
        let response = self
            .request(reqwest::Method::POST, "/api/invoices")
            .header("Idempotency-Key", idempotency_key)
            .json(sale)
            .send()
            .await
            .map_err(|error| crate::error::ApiError::transport(error.to_string()))?;
        self.unwrap_envelope(response).await
    }

    /// `GET /api/alerts` — stock and invoice alerts for notifications.
    pub async fn alerts(&self) -> Result<Vec<Alert>, ApiError> {
        self.get_data("/api/alerts").await
    }

    /// `GET /api/nav-badges` — the counters the left rail shows as badges.
    pub async fn nav_badges(&self) -> Result<NavBadges, ApiError> {
        self.get_data("/api/nav-badges").await
    }
}

/// Minimal query encoding (the worker parses `q` itself; keep reserved
/// characters safe without pulling another dependency).
fn urlencoding(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_search_terms() {
        assert_eq!(urlencoding("café crème"), "caf%C3%A9+cr%C3%A8me");
        assert_eq!(urlencoding("SKU-12_3."), "SKU-12_3.");
    }
}
