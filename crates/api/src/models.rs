//! Wire DTOs for the Atlas Commerce worker.
//!
//! Field names follow the worker's JSON exactly (snake_case); every field
//! gets a default so a server-side addition never breaks deserialization.

use serde::{Deserialize, Serialize};

// ── Generic envelope ─────────────────────────────────────────────────────

/// Most endpoints answer `{ success, data, meta? }`.
#[derive(Debug, Clone, Deserialize)]
pub struct Envelope<T> {
    pub success: bool,
    // `Option` fields are implicitly absent-equals-`None`; an explicit
    // `default` here would force a spurious `T: Default` bound.
    pub data: Option<T>,
    #[serde(default)]
    pub meta: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
}

/// Error answers without a `data` payload.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ApiStatus {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
}

// ── Health / canary ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Health {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub d1: bool,
    #[serde(default)]
    pub kv: bool,
    #[serde(default)]
    pub r2: bool,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub time: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Canary {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub total_ms: Option<u64>,
}

// ── Auth ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct LoginRequest<'a> {
    pub username: &'a str,
    pub password: &'a str,
    pub fingerprint: &'a str,
    /// The platform contract names the desktop client; the worker also
    /// receives an `X-Atlas-Client: native` header (see `client.rs`).
    pub client: &'a str,
}

#[derive(Debug, Serialize)]
pub struct OtpRequest<'a> {
    pub username: &'a str,
    pub password: &'a str,
    pub otp: &'a str,
    pub fingerprint: &'a str,
    pub client: &'a str,
}

#[derive(Debug, Serialize)]
pub struct VerifyLinkRequest<'a> {
    pub username: &'a str,
    pub otp: &'a str,
    pub sig: &'a str,
    pub exp: u64,
    pub fingerprint: &'a str,
    pub client: &'a str,
}

/// Top-level login / verify-otp answer (the token is not nested in `data`).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct LoginResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default, rename = "needs_otp")]
    pub needs_otp: bool,
    #[serde(default)]
    pub trusted_device: bool,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub reputation_score: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Me {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default, rename = "is_owner")]
    pub is_owner: bool,
    #[serde(default, rename = "display_name")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default, rename = "avatar_url")]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TrustedDevice {
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default)]
    pub full: String,
    #[serde(default, rename = "last_ip")]
    pub last_ip: Option<String>,
    #[serde(default, rename = "last_seen")]
    pub last_seen: Option<String>,
    #[serde(default, rename = "user_agent")]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub current: bool,
}

impl TrustedDevice {
    /// The identifier the revoke endpoint expects (full hash, not the
    /// truncated display value).
    #[must_use]
    pub fn revoke_id(&self) -> &str {
        if self.full.is_empty() {
            &self.fingerprint
        } else {
            &self.full
        }
    }
}

// ── QR pairing (desktop is the PC side) ──────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct QrSessionCreated {
    pub token: String,
    #[serde(rename = "expires_at")]
    pub expires_at: String,
    #[serde(rename = "expires_in")]
    pub expires_in: u64,
    /// `https://host/login/qr?t=…&mode=pc` — what the phone scans.
    #[serde(rename = "qr_url")]
    pub qr_url: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct QrSessionStatus {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default, rename = "display_name")]
    pub display_name: Option<String>,
    #[serde(default, rename = "avatar_url")]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default, rename = "expires_at")]
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct QrCompleteRequest<'a> {
    pub fingerprint: &'a str,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct QrComplete {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default, rename = "display_name")]
    pub display_name: Option<String>,
    #[serde(default, rename = "avatar_url")]
    pub avatar_url: Option<String>,
}

// ── Dashboard ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UnpaidSummary {
    #[serde(default)]
    pub count: i64,
    #[serde(default)]
    pub total: f64,
    #[serde(default)]
    pub open: f64,
    #[serde(default)]
    pub overdue: f64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MonthlyPoint {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub sales: f64,
    #[serde(default)]
    pub expenses: f64,
    #[serde(default)]
    pub profit: f64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Dashboard {
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(rename = "currentBalance", default)]
    pub current_balance: f64,
    #[serde(rename = "startingBalance", default)]
    pub starting_balance: f64,
    #[serde(rename = "stockValue", default)]
    pub stock_value: f64,
    #[serde(rename = "productCount", default)]
    pub product_count: i64,
    #[serde(rename = "lowStockCount", default)]
    pub low_stock_count: i64,
    #[serde(rename = "salesReceived", default)]
    pub sales_received: f64,
    #[serde(rename = "expensesTotal", default)]
    pub expenses_total: f64,
    #[serde(rename = "refundsTotal", default)]
    pub refunds_total: f64,
    #[serde(rename = "purchasesPaid", default)]
    pub purchases_paid: f64,
    #[serde(rename = "unpaidSales", default)]
    pub unpaid_sales: UnpaidSummary,
    #[serde(rename = "unpaidPurchases", default)]
    pub unpaid_purchases: UnpaidSummary,
    #[serde(rename = "recentInvoices", default)]
    pub recent_invoices: Vec<Invoice>,
    #[serde(rename = "monthlySeries", default)]
    pub monthly_series: Vec<MonthlyPoint>,
}

impl Dashboard {
    #[must_use]
    pub fn currency(&self) -> &str {
        self.currency.as_deref().unwrap_or("MAD")
    }
}

// ── Products ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Product {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sku: Option<String>,
    #[serde(default)]
    pub barcode: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub price: f64,
    #[serde(rename = "stock_quantity", default)]
    pub stock: f64,
    #[serde(rename = "low_stock_threshold", default)]
    pub low_stock_threshold: Option<f64>,
    #[serde(rename = "track_inventory", default = "default_one")]
    pub track_inventory: i64,
    #[serde(rename = "is_active", default = "default_one")]
    pub is_active: i64,
    #[serde(rename = "image_url", default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

fn default_one() -> i64 {
    1
}

impl Product {
    #[must_use]
    pub fn threshold(&self) -> f64 {
        self.low_stock_threshold.unwrap_or(0.0)
    }

    #[must_use]
    pub fn tracks_inventory(&self) -> bool {
        self.track_inventory != 0
    }

    #[must_use]
    pub fn is_low_stock(&self) -> bool {
        self.tracks_inventory() && self.stock <= self.threshold()
    }

    #[must_use]
    pub fn is_out_of_stock(&self) -> bool {
        self.tracks_inventory() && self.stock <= 0.0
    }

    #[must_use]
    pub fn subtitle(&self) -> String {
        [
            self.sku.clone(),
            self.barcode.clone(),
            self.category.clone(),
        ]
        .into_iter()
        .flatten()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CreateProduct {
    pub name: String,
    pub sku: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub barcode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub unit: String,
    pub price: f64,
    pub stock_quantity: f64,
    pub low_stock_threshold: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// `in` (entrée), `out` (sortie) or `adjustment` (signed quantity).
#[derive(Debug, Clone, Serialize)]
pub struct StockAdjust {
    pub quantity: f64,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// ── Invoices / quick sales ───────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Invoice {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub number: Option<String>,
    #[serde(rename = "client_name", default)]
    pub client_name: Option<String>,
    #[serde(rename = "issue_date", default)]
    pub issue_date: Option<String>,
    #[serde(rename = "due_date", default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub total: f64,
    #[serde(rename = "amount_paid", default)]
    pub amount_paid: Option<f64>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(rename = "doc_kind", default)]
    pub doc_kind: Option<String>,
}

impl Invoice {
    #[must_use]
    pub fn remaining(&self) -> f64 {
        (self.total - self.amount_paid.unwrap_or(0.0)).max(0.0)
    }

    #[must_use]
    pub fn is_quick_sale(&self) -> bool {
        matches!(
            self.doc_kind.as_deref(),
            Some("quick_sale") | Some("ticket")
        ) || self
            .number
            .as_deref()
            .is_some_and(|number| number.starts_with("VR"))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct QuickSaleItem {
    pub product_id: i64,
    pub product_name: String,
    pub quantity: f64,
    pub unit_price: f64,
    #[serde(rename = "tax_rate")]
    pub tax_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuickSale {
    pub client_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_phone: Option<String>,
    pub issue_date: String,
    pub status: String,
    pub quick_sale: bool,
    pub doc_kind: String,
    pub tax_exempt: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub items: Vec<QuickSaleItem>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CreatedInvoice {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub number: Option<String>,
    #[serde(default)]
    pub total: f64,
    #[serde(default)]
    pub status: Option<String>,
}

// ── Alerts / notifications ───────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Alert {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub href: Option<String>,
    #[serde(default)]
    pub count: Option<i64>,
}

// ── Atlas Agent IA ───────────────────────────────────────────────────────

/// `GET /api/nav-badges` — the shell's badge counters. The worker always
/// answers 200 and zero-fills anything its database version does not have.
#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
pub struct NavBadges {
    #[serde(rename = "invoices_unpaid", default)]
    pub invoices_unpaid: i64,
    #[serde(rename = "bills_unpaid", default)]
    pub bills_unpaid: i64,
    #[serde(rename = "bills_receive", default)]
    pub bills_receive: i64,
    #[serde(rename = "products_low", default)]
    pub products_low: i64,
    #[serde(rename = "actions_pending", default)]
    pub actions_pending: i64,
    #[serde(rename = "quotes_open", default)]
    pub quotes_open: i64,
    #[serde(rename = "vr_open", default)]
    pub vr_open: i64,
    #[serde(rename = "employees_pay_due", default)]
    pub employees_pay_due: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentChat {
    pub message: String,
    pub user_message: String,
    pub task: String,
    pub timezone: String,
    pub thread_id: String,
    pub source: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AgentReply {
    #[serde(default)]
    pub reply: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AgentPolicy {
    #[serde(default)]
    pub blocked: bool,
    #[serde(rename = "isAdmin", default)]
    pub is_admin: bool,
    #[serde(rename = "dailyUsed", default)]
    pub daily_used: u64,
    #[serde(rename = "weeklyUsed", default)]
    pub weekly_used: u64,
    #[serde(rename = "dailyLimit", default = "default_daily_limit")]
    pub daily_limit: u64,
    #[serde(rename = "weeklyLimit", default = "default_weekly_limit")]
    pub weekly_limit: u64,
}

const fn default_daily_limit() -> u64 {
    25_000
}

const fn default_weekly_limit() -> u64 {
    100_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_envelope_parses_otp_and_token_answers() {
        let otp: LoginResponse =
            serde_json::from_str(r#"{"success":true,"needs_otp":true,"emailed":true}"#).unwrap();
        assert!(otp.needs_otp && otp.token.is_none());

        let ok: LoginResponse = serde_json::from_str(
            r#"{"success":true,"needs_otp":false,"token":"jwt","username":"admin","role":"owner"}"#,
        )
        .unwrap();
        assert_eq!(ok.token.as_deref(), Some("jwt"));
        assert_eq!(ok.role.as_deref(), Some("owner"));
    }

    #[test]
    fn unknown_fields_are_forgiven() {
        let product: Product = serde_json::from_str(
            r#"{"id":3,"name":"Café","price":12.5,"stock_quantity":4,"future_field":true}"#,
        )
        .unwrap();
        assert_eq!(product.id, 3);
        assert_eq!(product.name, "Café");
        assert!(product.tracks_inventory());
        assert!(!product.is_out_of_stock());
    }

    #[test]
    fn nav_badges_zero_fill_missing_counters() {
        let badges: NavBadges = serde_json::from_str(r#"{"products_low":3,"vr_open":1}"#).unwrap();
        assert_eq!(badges.products_low, 3);
        assert_eq!(badges.vr_open, 1);
        assert_eq!(badges.invoices_unpaid, 0);
        assert_eq!(NavBadges::default().employees_pay_due, 0);
    }

    #[test]
    fn alerts_and_dashboard_accept_partial_payloads() {
        let alert: Alert = serde_json::from_str(r#"{"kind":"rupture","label":"x"}"#).unwrap();
        assert_eq!(alert.severity, None);
        let dashboard: Dashboard = serde_json::from_str(r#"{"productCount":12}"#).unwrap();
        assert_eq!(dashboard.product_count, 12);
        assert_eq!(dashboard.currency(), "MAD");
    }
}
