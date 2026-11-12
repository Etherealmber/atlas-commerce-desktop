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
    fetch_path(api, &format!("/api/invoices?client={client_id}"), None).await
}

/// Mirrors `isStandardInvoice` in the web Invoices page. The worker's
/// client filter alone also returns quotes and credit notes; those are not
/// sales invoices and must not be presented as such. The situation report is
/// deliberately different: the server includes VR tickets and WR credits.
pub(crate) fn is_standard_invoice(invoice: &Invoice) -> bool {
    let kind = invoice
        .doc_kind
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let status = invoice.status.as_deref().unwrap_or("").to_ascii_lowercase();
    let number = invoice.number.as_deref().unwrap_or("");
    !matches!(
        kind.as_str(),
        "quick_sale" | "ticket" | "quote" | "devis" | "remboursement"
    ) && !matches!(status.as_str(), "quote" | "devis" | "remboursement")
        && !["VR", "WR", "DV", "DE"]
            .iter()
            .any(|prefix| number.starts_with(prefix))
}

/// Native, read-only representation of the invoice detail endpoint.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct InvoiceDetail {
    #[serde(flatten)]
    pub invoice: Invoice,
    pub items: Vec<InvoiceLine>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct InvoiceLine {
    pub product_name: Option<String>,
    pub quantity: Option<f64>,
    pub unit_price: Option<f64>,
    pub tax_rate: Option<f64>,
    pub line_total: Option<f64>,
}

pub(crate) async fn fetch_detail(api: &ApiClient, id: i64) -> Result<InvoiceDetail, ApiError> {
    if id <= 0 {
        return Err(ApiError::client("Identifiant de facture invalide."));
    }
    let detail: InvoiceDetail = fetch_path(api, &format!("/api/invoices/{id}"), None).await?;
    if detail.invoice.id != id {
        return Err(ApiError::client(
            "La facture reçue ne correspond pas à la demande.",
        ));
    }
    Ok(detail)
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
    fetch_path(api, "/api/bills", None).await
}

/// Receipt from GET /api/receipts; a receipt is not itself a new payment.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Receipt {
    pub id: i64,
    pub number: String,
    pub client_name: String,
    pub amount: f64,
    pub issue_date: Option<String>,
    pub payment_method: Option<String>,
    pub invoice_id: Option<i64>,
    #[serde(default)]
    pub invoice_number: Option<String>,
}

pub(crate) async fn fetch_receipts(api: &ApiClient) -> Result<Vec<Receipt>, ApiError> {
    fetch_path(api, "/api/receipts", None).await
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
    fetch_path(api, "/api/ledger/reconcile", None).await
}

/// The worker proposes matches only; it does not apply or confirm them.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct BankReconciliation {
    pub parsed: u64,
    pub matched: u64,
    pub unmatched_count: u64,
    pub matches: Vec<BankMatch>,
    pub unmatched: Vec<BankLine>,
}
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct BankLine {
    pub date: String,
    pub label: String,
    pub amount: f64,
    pub line: u64,
}
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct BankMatch {
    pub bank: BankLine,
    #[serde(rename = "match")]
    pub candidate: BankCandidate,
    pub score: i64,
}
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct BankCandidate {
    pub kind: String,
    pub id: i64,
    pub label: String,
}

pub(crate) async fn reconcile_bank_csv(
    api: &ApiClient,
    csv: String,
) -> Result<BankReconciliation, ApiError> {
    if csv.trim().is_empty() || csv.len() > 1024 * 1024 {
        return Err(ApiError::client(
            "Le relevé doit contenir entre 1 octet et 1 Mo.",
        ));
    }
    fetch_path(
        api,
        "/api/bank/reconcile-csv",
        Some(serde_json::json!({"csv":csv, "date_window_days":7})),
    )
    .await
}

async fn fetch_path<T: serde::de::DeserializeOwned>(
    api: &ApiClient,
    path: &str,
    json: Option<serde_json::Value>,
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
    let request = if let Some(body) = json {
        http.post(url).json(&body)
    } else {
        http.get(url)
    };
    let mut response = request
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
    use super::{BankReconciliation, Bill, Envelope, InvoiceDetail, LedgerReconciliation, Receipt};
    use atlas_api::models::Invoice;

    #[test]
    fn client_invoices_follow_web_document_classification() {
        let invoice = Invoice {
            number: Some("FA-1".into()),
            ..Default::default()
        };
        assert!(super::is_standard_invoice(&invoice));
        for number in ["VR-1", "WR-1", "DV-1", "DE-1"] {
            let other = Invoice {
                number: Some(number.into()),
                ..invoice.clone()
            };
            assert!(!super::is_standard_invoice(&other));
        }
        for kind in ["quick_sale", "ticket", "quote", "devis", "remboursement"] {
            let other = Invoice {
                doc_kind: Some(kind.into()),
                ..invoice.clone()
            };
            assert!(!super::is_standard_invoice(&other));
        }
        for status in ["quote", "devis", "remboursement"] {
            let other = Invoice {
                status: Some(status.into()),
                ..invoice.clone()
            };
            assert!(!super::is_standard_invoice(&other));
        }
        let cancelled = Invoice {
            status: Some("cancelled".into()),
            ..invoice
        };
        assert!(super::is_standard_invoice(&cancelled));
    }

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
    fn decodes_invoice_detail_and_lines() {
        let reply: Envelope<InvoiceDetail> = serde_json::from_str(r#"{"success":true,"data":{"id":42,"number":"FA-42","total":120,"items":[{"product_name":"Produit","quantity":2,"unit_price":50,"tax_rate":20,"line_total":120}]}}"#).unwrap();
        let detail = reply.data.unwrap();
        assert_eq!(detail.invoice.id, 42);
        assert_eq!(detail.items[0].product_name.as_deref(), Some("Produit"));
        assert_eq!(detail.items[0].line_total, Some(120.0));
        assert!(
            serde_json::from_str::<Envelope<InvoiceDetail>>(r#"{"success":true,"data":{"id":42}}"#)
                .is_err()
        );
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
    fn decodes_receipts_with_optional_invoice_link() {
        let data: Envelope<Vec<Receipt>> = serde_json::from_str(r#"{"success":true,"data":[{"id":1,"number":"REC-1","client_name":"Client","amount":100,"issue_date":"2026-01-01","payment_method":"cash","invoice_id":null}]}"#).unwrap();
        let receipts = data.data.unwrap();
        assert_eq!(receipts[0].number, "REC-1");
        assert!(receipts[0].invoice_id.is_none());
        assert!(receipts[0].invoice_number.is_none());
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
    #[test]
    fn parses_bank_proposals_without_implying_confirmation() {
        let reply: Envelope<BankReconciliation> = serde_json::from_str(r#"{"success":true,"data":{"parsed":2,"matched":1,"unmatched_count":1,"matches":[{"bank":{"date":"2026-01-02","label":"Paiement","amount":120,"line":1},"match":{"kind":"invoice_payment","id":9,"label":"FA-1"},"score":75}],"unmatched":[{"date":"2026-01-03","label":"Autre","amount":10,"line":2}]}}"#).unwrap();
        let data = reply.data.unwrap();
        assert_eq!(data.matched, 1);
        assert_eq!(data.matches[0].candidate.id, 9);
        assert_eq!(data.unmatched_count, 1);
    }
}
