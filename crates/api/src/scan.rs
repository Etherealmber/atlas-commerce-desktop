//! Barcode-scan endpoints used by the in-app scan bar. Contract: the
//! `/api/scan` block of `worker/index.ts` — a hot-path lookup by barcode or
//! SKU, and the mobile "quick product" creation flow.

use crate::ApiClient;
use crate::error::ApiError;
use crate::models::{Product, QuickProductWrite, ScannedProduct};

impl ApiClient {
    /// `GET /api/scan/barcode/:code` — the active product (or variant)
    /// matching the barcode, falling back to a case-insensitive SKU match.
    pub async fn scan_barcode(&self, code: &str) -> Result<ScannedProduct, ApiError> {
        let encoded = encode_path(code.trim());
        self.get_data(&format!("/api/scan/barcode/{encoded}")).await
    }

    /// `POST /api/scan/quick-product` — creates a minimal product from a
    /// scanned barcode. A 409 `DUPLICATE_PRODUCT` is reported through the
    /// normal [`ApiError`] so the UI can retry with `force_duplicate`.
    pub async fn quick_product(&self, body: &QuickProductWrite) -> Result<Product, ApiError> {
        self.post_data("/api/scan/quick-product", body).await
    }
}

/// Percent-encodes a path segment (a space is `%20`, not `+`).
fn encode_path(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::encode_path;

    #[test]
    fn encodes_path_segments() {
        assert_eq!(encode_path("SKU-12_3."), "SKU-12_3.");
        assert_eq!(encode_path("a b/c"), "a%20b%2Fc");
    }
}
