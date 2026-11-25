//! Attendance endpoints verified against Worker `routes/attendance.ts`.
//! A missing `date` intentionally uses the Worker's Morocco-local date.

use crate::{ApiClient, ApiError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct AttendanceDay {
    pub date: String,
    pub rows: Vec<AttendanceRow>,
    pub summary: AttendanceSummary,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AttendanceRow {
    pub employee_id: i64,
    pub name: String,
    pub role_title: String,
    pub check_in: Option<String>,
    pub check_out: Option<String>,
    pub status: String,
    pub note: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AttendanceSummary {
    pub present: u64,
    pub absent: u64,
    pub late: u64,
    pub excused: u64,
    pub total: u64,
}

#[derive(Deserialize)]
pub struct AttendanceMutation {}

#[derive(Serialize)]
struct MarkTime<'a> {
    employee_id: i64,
    date: &'a str,
    source: &'static str,
}

impl ApiClient {
    /// Fetch today's attendance, using the server's local date.
    pub async fn attendance_today(&self) -> Result<AttendanceDay, ApiError> {
        self.get_data("/api/attendance").await
    }

    /// Enable attendance through the company settings endpoint. The server
    /// applies its own role checks; the response is not retained.
    pub async fn enable_attendance(&self) -> Result<AttendanceMutation, ApiError> {
        self.put_data(
            "/api/settings",
            &serde_json::json!({"attendance_enabled": true}),
        )
        .await
    }

    /// Record a check-in or check-out using the Worker's local time. Never
    /// automatically retry: the write may have succeeded despite a lost reply.
    pub async fn attendance_mark(
        &self,
        employee_id: i64,
        date: &str,
        check_out: bool,
    ) -> Result<AttendanceMutation, ApiError> {
        if employee_id <= 0 || !valid_date(date) {
            return Err(ApiError::client("Date ou employé invalide."));
        }
        let path = if check_out {
            "/api/attendance/check-out"
        } else {
            "/api/attendance/check-in"
        };
        self.post_data(
            path,
            &MarkTime {
                employee_id,
                date,
                source: "manual",
            },
        )
        .await
    }
}

fn valid_date(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_day_shape_accepts_nullable_times_and_extra_fields() {
        let day: AttendanceDay = serde_json::from_value(serde_json::json!({
            "date": "2026-10-22",
            "summary": {"present": 0, "absent": 1, "late": 0, "excused": 0, "total": 1},
            "rows": [{"employee_id": 7, "name": "Exemple", "role_title": "", "check_in": null,
                "check_out": null, "status": "absent", "note": "", "suggested_pay": 100}],
            "period_note": "cycle"
        }))
        .unwrap();
        assert_eq!(day.rows[0].employee_id, 7);
        assert!(day.rows[0].check_in.is_none());
        assert_eq!(day.summary.absent, 1);
    }

    #[test]
    fn attendance_date_cannot_add_query_or_body_syntax() {
        assert!(valid_date("2026-10-22"));
        assert!(!valid_date("2026-10-22&x=1"));
        assert!(!valid_date("2026/10/22"));
    }
}
