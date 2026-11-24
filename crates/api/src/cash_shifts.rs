//! Cash shift contracts from Worker `routes/cashShifts.ts`.
//! Writes are never retried automatically because a lost response is ambiguous.
use crate::{ApiClient, ApiError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize)]
pub struct CashShift {
    pub id: i64,
    pub till_id: String,
    pub operator: String,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub opening_float: f64,
    pub counted_amount: Option<f64>,
    pub expected_amount: Option<f64>,
    pub variance: Option<f64>,
    pub status: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CashShiftDetail {
    #[serde(flatten)]
    pub shift: CashShift,
    pub movements: Vec<CashMovement>,
    pub summary: CashSummary,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CashMovement {
    pub direction: String,
    pub payment_method: String,
    pub amount: f64,
    pub reference: String,
    pub label: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashSummary {
    pub expected_cash: f64,
    pub total_expected_all: f64,
    pub methods: BTreeMap<String, MethodSummary>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MethodSummary {
    pub label: String,
    pub in_total: f64,
    pub out_total: f64,
    pub expected: f64,
    pub counted: Option<f64>,
    pub variance: Option<f64>,
}

#[derive(Serialize)]
struct OpenShift<'a> {
    till_id: &'a str,
    opening_float: f64,
    notes: &'a str,
}
#[derive(Serialize)]
struct Movement<'a> {
    direction: &'a str,
    payment_method: &'a str,
    amount: f64,
    label: &'a str,
    reference: &'a str,
}
#[derive(Serialize)]
struct CloseShift<'a> {
    counted_amount: f64,
    notes: &'a str,
}

impl ApiClient {
    pub async fn cash_shifts(&self) -> Result<Vec<CashShift>, ApiError> {
        self.get_data("/api/cash-shifts?limit=50").await
    }
    pub async fn cash_shift(&self, id: i64) -> Result<CashShiftDetail, ApiError> {
        if id <= 0 {
            return Err(ApiError::client("Session invalide."));
        }
        self.get_data(&format!("/api/cash-shifts/{id}")).await
    }
    pub async fn open_cash_shift(
        &self,
        till: &str,
        amount: f64,
        notes: &str,
    ) -> Result<CashShift, ApiError> {
        let till = till.trim();
        if till.is_empty()
            || till.chars().count() > 64
            || !valid_amount(amount, true)
            || notes.chars().count() > 500
        {
            return Err(ApiError::client("Caisse, montant ou notes invalides."));
        }
        self.post_data(
            "/api/cash-shifts/open",
            &OpenShift {
                till_id: till,
                opening_float: amount,
                notes,
            },
        )
        .await
    }
    pub async fn cash_shift_movement(
        &self,
        id: i64,
        direction: &str,
        method: &str,
        amount: f64,
        label: &str,
        reference: &str,
    ) -> Result<CashMovement, ApiError> {
        if id <= 0
            || !matches!(direction, "in" | "out")
            || !matches!(method, "cash" | "card" | "transfer" | "check" | "other")
            || !valid_amount(amount, false)
            || label.chars().count() > 200
            || reference.chars().count() > 120
        {
            return Err(ApiError::client("Mouvement invalide."));
        }
        self.post_data(
            &format!("/api/cash-shifts/{id}/movements"),
            &Movement {
                direction,
                payment_method: method,
                amount,
                label,
                reference,
            },
        )
        .await
    }
    pub async fn close_cash_shift(
        &self,
        id: i64,
        amount: f64,
        notes: &str,
    ) -> Result<CashShiftDetail, ApiError> {
        if id <= 0 || !valid_amount(amount, true) || notes.chars().count() > 1000 {
            return Err(ApiError::client("Clôture invalide."));
        }
        self.post_data(
            &format!("/api/cash-shifts/{id}/close"),
            &CloseShift {
                counted_amount: amount,
                notes,
            },
        )
        .await
    }
}

fn valid_amount(value: f64, allow_zero: bool) -> bool {
    let cents = value * 100.0;
    value.is_finite()
        && cents.is_finite()
        && (cents - cents.round()).abs() < 0.000_001
        && if allow_zero {
            value >= 0.0
        } else {
            value > 0.0
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_non_finite_and_negative_amounts() {
        assert!(!valid_amount(f64::NAN, true));
        assert!(!valid_amount(f64::INFINITY, true));
        assert!(!valid_amount(-1.0, true));
        assert!(!valid_amount(0.0, false));
        assert!(!valid_amount(0.001, false));
        assert!(valid_amount(0.0, true));
    }
    #[test]
    fn worker_detail_shape_deserializes() {
        let detail: CashShiftDetail = serde_json::from_value(serde_json::json!({
            "id": 4, "till_id":"CAISSE-1", "operator":"demo", "opened_at":"2026-10-22T00:00:00Z",
            "closed_at":null, "opening_float":100, "counted_amount":null, "expected_amount":null,
            "variance":null, "status":"open", "notes":"", "movements":[],
            "summary":{"expectedCash":100,"totalExpectedAll":100,"methods":{"cash":{"label":"Espèces","in_total":0,"out_total":0,"expected":100,"counted":null,"variance":null}}}
        })).unwrap();
        assert_eq!(detail.shift.id, 4);
        assert_eq!(detail.summary.expected_cash, 100.0);
    }
}
