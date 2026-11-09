//! Read-only, client-filtered invoice list for the Clients view.
//!
//! The worker's `GET /api/invoices?client=<id>` filters on client_id. The
//! list is requested with the selected server's bearer token in the header;
//! redirects are disabled and response size is bounded.

use atlas_api::{ApiClient, ApiError, models::Invoice};
use serde::Deserialize;

#[derive(Deserialize)]
struct Envelope<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

pub(crate) async fn fetch(api: &ApiClient, client_id: i64) -> Result<Vec<Invoice>, ApiError> {
    if client_id <= 0 {
        return Err(ApiError::client("Identifiant client invalide."));
    }
    fetch_path(api, &format!("/api/invoices?client={client_id}")).await
}

/// A supplier invoice as returned by the existing GET /api/bills endpoint.
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct Bill {
    pub id: i64,
    #[serde(default)]
    pub number: Option<String>,
    #[serde(default)]
    pub vendor_name: Option<String>,
    #[serde(default)]
    pub issue_date: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub total: f64,
    #[serde(default)]
    pub amount_paid: Option<f64>,
}

pub(crate) async fn fetch_bills(api: &ApiClient) -> Result<Vec<Bill>, ApiError> {
    fetch_path(api, "/api/bills").await
}

/// Read-only reconciliation returned by GET /api/ledger/reconcile.
/// Discrepancy arrays are retained only to count them, not to infer repairs.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct LedgerReconciliation {
    pub payments_count: u64,
    pub ledger_income_count: u64,
    pub receipts_count: u64,
    pub unmatched_payments: Vec<serde_json::Value>,
    pub orphan_ledger: Vec<serde_json::Value>,
    pub paid_without_receipt: Vec<serde_json::Value>,
    pub summary: LedgerSummary,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct LedgerSummary {
    pub ok: bool,
    pub message: String,
}

pub(crate) async fn fetch_ledger_reconciliation(
    api: &ApiClient,
) -> Result<LedgerReconciliation, ApiError> {
    fetch_path(api, "/api/ledger/reconcile").await
}

async fn fetch_path<T: serde::de::DeserializeOwned>(
    api: &ApiClient,
    path: &str,
) -> Result<T, ApiError> {
    let token = api
        .token()
        .ok_or_else(|| ApiError::client("Session indisponible."))?;
    let url = format!("{}{path}", api.base_url());
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|_| ApiError::transport("Client HTTP indisponible."))?;
    let mut response = http
        .get(url)
        .bearer_auth(token)
        .header("X-Atlas-Client", "native")
        .header("X-Device-Fingerprint", api.fingerprint())
        .send()
        .await
        .map_err(|_| ApiError::transport("Impossible de charger les factures."))?;
    let status = response.status().as_u16();
    const MAX_BYTES: usize = 4 * 1024 * 1024;
    if response
        .content_length()
        .is_some_and(|n| n > MAX_BYTES as u64)
    {
        return Err(ApiError::client("Réponse des factures trop volumineuse."));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ApiError::transport("Lecture des factures interrompue."))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_BYTES {
            return Err(ApiError::client("Réponse des factures trop volumineuse."));
        }
        bytes.extend_from_slice(&chunk);
    }
    if !(200..300).contains(&status) {
        // A proxy or gateway can return HTML; preserve the HTTP status rather
        // than turning a 401 into a misleading JSON parse failure.
        let message = serde_json::from_slice::<Envelope<serde_json::Value>>(&bytes)
            .ok()
            .and_then(|body| body.error)
            .unwrap_or_else(|| format!("Requête refusée ({status})."));
        return Err(ApiError {
            status,
            code: None,
            message,
        });
    }
    let envelope: Envelope<T> = serde_json::from_slice(&bytes)
        .map_err(|_| ApiError::client("Réponse du serveur illisible."))?;
    if !envelope.success {
        return Err(ApiError {
            status,
            code: None,
            message: envelope
                .error
                .unwrap_or_else(|| format!("Requête refusée ({status}).")),
        });
    }
    envelope
        .data
        .ok_or_else(|| ApiError::client("Liste des factures absente."))
}

#[cfg(test)]
mod tests {
    use super::{Bill, Envelope, LedgerReconciliation};
    use atlas_api::models::Invoice;

    #[test]
    fn decodes_the_worker_list_envelope() {
        let data: Envelope<Vec<Invoice>> = serde_json::from_str(
            r#"{"success":true,"data":[{"id":42,"number":"F-42","total":120,"amount_paid":20}]}"#,
        )
        .unwrap();
        let invoices = data.data.unwrap();
        assert_eq!(invoices[0].id, 42);
        assert_eq!(invoices[0].remaining(), 100.0);
    }
    #[test]
    fn decodes_supplier_bill_without_inventing_payment_status() {
        let data: Envelope<Vec<Bill>> = serde_json::from_str(r#"{"success":true,"data":[{"id":7,"number":"FA-7","vendor_name":"Fournisseur","total":120,"amount_paid":30,"status":"partial"}]}"#).unwrap();
        let bill = &data.data.unwrap()[0];
        assert_eq!(bill.id, 7);
        assert_eq!(bill.total - bill.amount_paid.unwrap(), 90.0);
        assert_eq!(bill.status.as_deref(), Some("partial"));
    }
    #[test]
    fn reconciliation_does_not_hide_paid_invoices_without_receipts() {
        let reply: Envelope<LedgerReconciliation> = serde_json::from_str(r#"{"success":true,"data":{"payments_count":1,"ledger_income_count":1,"receipts_count":0,"unmatched_payments":[],"orphan_ledger":[],"paid_without_receipt":[{"id":12}],"summary":{"ok":true,"message":"Des écarts de rapprochement ont été détectés."}}}"#).unwrap();
        let data = reply.data.unwrap();
        assert!(data.summary.ok);
        assert_eq!(data.paid_without_receipt.len(), 1);
        assert!(!data.summary.message.is_empty());
        assert!(
            serde_json::from_str::<Envelope<LedgerReconciliation>>(r#"{"success":true,"data":{}}"#)
                .is_err()
        );
    }
}
