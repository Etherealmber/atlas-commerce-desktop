//! Printable HTML reports opened in the system browser.
//!
//! The worker never accepts the session JWT inside a URL. Instead a
//! short-lived (30-minute) HMAC report ticket is minted through an
//! authenticated request and appended to the printable report URL,
//! mirroring the web app's `lib/reports.ts` flow.

use serde::Deserialize;

use crate::{ApiClient, ApiError};

/// Short-lived ticket granting access to a printable report URL.
#[derive(Debug, Clone, Deserialize)]
pub struct ReportTicket {
    /// Opaque HMAC token carried by the `rt` query parameter.
    pub rt: String,
    /// Validity window in seconds.
    pub expires_in: u64,
}

impl ApiClient {
    /// Mint a short-lived report link ticket.
    ///
    /// `GET /api/reports/link-ticket` — authenticated via the regular
    /// `Authorization: Bearer` header; the returned [`ReportTicket`] is
    /// safe to place inside a browser URL.
    pub async fn report_link_ticket(&self) -> Result<ReportTicket, ApiError> {
        self.get_data("/api/reports/link-ticket").await
    }
}
