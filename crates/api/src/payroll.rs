//! Read-only payroll contracts from the Worker's `routes/payroll.ts`.
//! Amounts and statutory calculations are supplied by the server, never
//! recalculated in the client.
use crate::{ApiClient, ApiError};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct PayrollRun {
    pub id: i64,
    pub period_month: String,
    pub status: String,
    pub run_kind: String,
    pub corrects_run_id: Option<i64>,
    pub total_gross: f64,
    pub total_cnss_emp: f64,
    pub total_amo_emp: f64,
    pub total_igr: f64,
    pub total_net: f64,
    pub total_employer_cost: f64,
    #[serde(default)]
    pub lines: Vec<PayrollLine>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PayrollLine {
    pub id: i64,
    pub employee_name: String,
    pub emp_number: Option<String>,
    pub days_worked: f64,
    pub overtime_hours: f64,
    pub base_salary: f64,
    pub attendance_adjustment: f64,
    pub bonuses: f64,
    pub overtime_amount: f64,
    pub gross_salary: f64,
    pub cnss_emp: f64,
    pub amo_emp: f64,
    pub igr: f64,
    pub advances_deducted: f64,
    pub net_salary: f64,
    pub employer_cost: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PayrollSettings {
    pub cnss_emp_rate: f64,
    pub cnss_ceiling: f64,
    pub amo_emp_rate: f64,
    pub disclaimer: String,
}

impl ApiClient {
    pub async fn payroll_runs(&self) -> Result<Vec<PayrollRun>, ApiError> {
        self.get_data("/api/payroll/runs").await
    }
    pub async fn payroll_run(&self, id: i64) -> Result<PayrollRun, ApiError> {
        if id <= 0 {
            return Err(ApiError::client("Lot de paie invalide."));
        }
        self.get_data(&format!("/api/payroll/runs/{id}")).await
    }
    pub async fn payroll_settings(&self) -> Result<PayrollSettings, ApiError> {
        self.get_data("/api/payroll/settings").await
    }
}

#[cfg(test)]
mod tests {
    use super::PayrollRun;
    #[test]
    fn list_and_detail_share_worker_run_shape() {
        let value = serde_json::json!({"id":1,"period_month":"2026-01","status":"draft","run_kind":"regular","corrects_run_id":null,"total_gross":1200.0,"total_cnss_emp":50.0,"total_amo_emp":25.0,"total_igr":0.0,"total_net":1125.0,"total_employer_cost":1400.0});
        let run: PayrollRun = serde_json::from_value(value).unwrap();
        assert_eq!(run.period_month, "2026-01");
        assert!(run.lines.is_empty());
    }
}
