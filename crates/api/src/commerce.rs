//! Commerce endpoints: products, stock movements, dashboard, sales and
//! alerts. Query parameters follow the worker and the Android client.

use crate::ApiClient;
use crate::error::ApiError;
use crate::models::*;

// The Worker reads this flag from JSON, not the query string.
#[derive(serde::Serialize)]
struct CreateProductRequest<'a> {
    #[serde(flatten)]
    product: &'a CreateProduct,
    force_duplicate: bool,
}

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
        let request = CreateProductRequest {
            product,
            force_duplicate,
        };
        self.post_data("/api/products", &request).await
    }

    /// `POST /api/products/:id/adjust-stock`.
    pub async fn adjust_stock(&self, id: i64, adjust: &StockAdjust) -> Result<Product, ApiError> {
        self.post_data(&format!("/api/products/{id}/adjust-stock"), adjust)
            .await
    }

    /// `GET /api/products/:id/movements`: 90-day window or paged full history.
    pub async fn product_movements(
        &self,
        id: i64,
        full: bool,
        offset: u32,
    ) -> Result<Vec<StockMovement>, ApiError> {
        self.get_data(&format!(
            "/api/products/{id}/movements?full={}&limit=50&offset={offset}",
            u8::from(full)
        ))
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
pub(crate) fn urlencoding(value: &str) -> String {
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
    fn duplicate_override_is_an_explicit_boolean_in_the_product_body() {
        let product = CreateProduct {
            name: "Produit exemple".to_owned(),
            sku: "SKU-EXEMPLE".to_owned(),
            ..Default::default()
        };
        for force in [false, true] {
            let value = serde_json::to_value(CreateProductRequest {
                product: &product,
                force_duplicate: force,
            })
            .unwrap();
            assert_eq!(value["name"], "Produit exemple");
            assert_eq!(value["sku"], "SKU-EXEMPLE");
            assert_eq!(value["force_duplicate"], force);
            assert!(value.get("product").is_none());
        }
    }

    #[test]
    fn product_override_is_sent_in_json_not_in_url() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::Duration;

        for force in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut raw = Vec::new();
                let header_end = loop {
                    let mut chunk = [0; 1024];
                    let count = stream.read(&mut chunk).unwrap();
                    assert!(count > 0);
                    raw.extend_from_slice(&chunk[..count]);
                    if let Some(i) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&raw[..header_end]);
                assert!(headers.starts_with("POST /api/products HTTP/1.1\r\n"));
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().ok())
                            .flatten()
                    })
                    .unwrap();
                while raw.len() - header_end < length {
                    let mut chunk = [0; 1024];
                    let count = stream.read(&mut chunk).unwrap();
                    assert!(count > 0);
                    raw.extend_from_slice(&chunk[..count]);
                }
                let body: serde_json::Value =
                    serde_json::from_slice(&raw[header_end..header_end + length]).unwrap();
                assert_eq!(body["force_duplicate"], force);
                assert_eq!(body["name"], "Produit exemple");
                let response = br#"{"success":true,"data":{"id":42}}"#;
                write!(stream, "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.len()).unwrap();
                stream.write_all(response).unwrap();
            });
            let runner = atlas_core::jobs::JobRunner::new();
            let client =
                ApiClient::new(&format!("http://{address}"), "fixture-device", "fixture").unwrap();
            let product = CreateProduct {
                name: "Produit exemple".to_owned(),
                ..Default::default()
            };
            assert!(
                runner
                    .spawn(async move { client.create_product(&product, force).await })
                    .wait()
                    .unwrap()
                    .is_ok()
            );
            server.join().unwrap();
        }
    }

    #[test]
    fn product_history_uses_the_worker_window_and_pagination_contract() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::Duration;

        for (full, offset) in [(false, 0), (true, 50)] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut raw = [0; 4096];
                let n = stream.read(&mut raw).unwrap();
                let request = String::from_utf8_lossy(&raw[..n]);
                assert!(request.starts_with(&format!(
                    "GET /api/products/9/movements?full={}&limit=50&offset={offset} HTTP/1.1",
                    u8::from(full)
                )));
                let response = br#"{"success":true,"data":[{"id":3,"type":"sale","quantity":1,"previous_quantity":51,"new_quantity":50}]}"#;
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.len()).unwrap();
                stream.write_all(response).unwrap();
            });
            let runner = atlas_core::jobs::JobRunner::new();
            let client =
                ApiClient::new(&format!("http://{address}"), "fixture-device", "fixture").unwrap();
            let rows = runner
                .spawn(async move { client.product_movements(9, full, offset).await })
                .wait()
                .unwrap()
                .unwrap();
            assert_eq!(rows[0].new_quantity, Some(50.0));
            server.join().unwrap();
        }
    }

    #[test]
    fn encodes_search_terms() {
        assert_eq!(urlencoding("café crème"), "caf%C3%A9+cr%C3%A8me");
        assert_eq!(urlencoding("SKU-12_3."), "SKU-12_3.");
    }
}
