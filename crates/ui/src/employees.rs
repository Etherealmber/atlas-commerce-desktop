//! Employees (HR) endpoints, verified against `worker/routes/employees.ts`.
//!
//! Requests carry the same native headers as [`atlas_api::ApiClient`] but are
//! issued here so the existing API crate stays untouched. Redirects are
//! disabled, responses are bounded, and errors never echo a response body.

use atlas_api::{ApiClient, ApiError};
use serde::{Deserialize, Serialize};

const MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Employee {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub emp_number: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub last_name: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub role_title: String,
    #[serde(default)]
    pub department: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub employment_status: String,
    #[serde(default)]
    pub joined_date: String,
    #[serde(default)]
    pub national_id: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub pay_amount: f64,
    #[serde(default)]
    pub pay_cycle_type: String,
    #[serde(default)]
    pub pay_cycle_days: i64,
    #[serde(default)]
    pub cycle_days: i64,
    #[serde(default)]
    pub last_paid_at: Option<String>,
    #[serde(default)]
    pub next_due: String,
    #[serde(default)]
    pub pay_overdue: bool,
    #[serde(default)]
    pub has_id_doc: bool,
    #[serde(default)]
    pub active: i64,
    #[serde(default)]
    pub is_resigned: bool,
    #[serde(default)]
    pub suggested_pay: f64,
    #[serde(default)]
    pub gross_accrued: f64,
    #[serde(default)]
    pub advances_in_period: f64,
    #[serde(default)]
    pub accrued_days: i64,
    #[serde(default)]
    pub period_from: Option<String>,
    #[serde(default)]
    pub can_advance: bool,
    #[serde(default)]
    pub max_advance: f64,
}

impl Employee {
    pub fn is_active(&self) -> bool {
        self.active != 0
    }

    pub fn label(&self) -> &str {
        if self.display_name.is_empty() {
            &self.name
        } else {
            &self.display_name
        }
    }

    pub fn cycle_label(&self) -> String {
        if self.pay_cycle_type == "days" {
            format!("Tous les {} j", self.cycle_days.max(1))
        } else {
            "Mensuel".to_owned()
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Payment {
    pub id: i64,
    #[serde(default)]
    pub amount: f64,
    #[serde(default)]
    pub paid_at: String,
    #[serde(default)]
    pub period_label: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct EmployeeDetail {
    #[serde(flatten)]
    pub employee: Employee,
    #[serde(default)]
    pub payments: Vec<Payment>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct EmployeesList {
    #[serde(default)]
    pub rows: Vec<Employee>,
    #[serde(default)]
    pub categories: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Action {
    #[serde(default)]
    pub message: String,
}

#[derive(Serialize)]
struct PayBody<'a> {
    amount: f64,
    paid_at: &'a str,
    note: &'a str,
    payment_method: &'a str,
}

#[derive(Serialize)]
struct ResignBody<'a> {
    reason: &'a str,
    settle_pay: bool,
}

#[derive(Serialize)]
pub(crate) struct NewEmployee {
    first_name: String,
    last_name: String,
    name: String,
    phone: String,
    email: String,
    role_title: String,
    department: String,
    category: String,
    national_id: String,
    city: String,
    pay_amount: f64,
    pay_cycle_type: String,
    pay_cycle_days: i64,
    notes: String,
}

#[derive(Deserialize)]
struct Envelope<T> {
    success: bool,
    data: Option<T>,
    #[serde(default)]
    categories: Option<Vec<String>>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

fn client() -> Result<reqwest::Client, ApiError> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|_| ApiError::transport("Client HTTP indisponible."))
}

async fn request<T: serde::de::DeserializeOwned>(
    api: &ApiClient,
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
) -> Result<Envelope<T>, ApiError> {
    let token = api
        .token()
        .ok_or_else(|| ApiError::client("Session indisponible."))?;
    let url = format!("{}{path}", api.base_url());
    let mut builder = client()?
        .request(method, url)
        .bearer_auth(token)
        .header("X-Atlas-Client", "native")
        .header("X-Device-Fingerprint", api.fingerprint());
    if let Some(body) = body {
        builder = builder.json(&body);
    }
    let mut response = builder
        .send()
        .await
        .map_err(|_| ApiError::transport("Le serveur est injoignable."))?;
    let status = response.status().as_u16();
    if response
        .content_length()
        .is_some_and(|size| size > MAX_BYTES as u64)
    {
        return Err(ApiError::client(
            "La réponse du serveur est trop volumineuse.",
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ApiError::transport("Lecture de la réponse interrompue."))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_BYTES {
            return Err(ApiError::client(
                "La réponse du serveur est trop volumineuse.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    let envelope: Envelope<T> = serde_json::from_slice(&bytes)
        .map_err(|_| ApiError::transport("Réponse du serveur illisible."))?;
    if !envelope.success {
        return Err(ApiError {
            status,
            code: None,
            message: envelope
                .error
                .unwrap_or_else(|| "Le serveur a refusé l’opération.".to_owned()),
        });
    }
    Ok(envelope)
}

async fn action(
    api: &ApiClient,
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
) -> Result<Action, ApiError> {
    let envelope = request::<serde_json::Value>(api, method, path, body).await?;
    Ok(Action {
        message: envelope.message.unwrap_or_default(),
    })
}

pub(crate) async fn fetch(
    api: &ApiClient,
    status: &str,
    query: &str,
    category: &str,
) -> Result<EmployeesList, ApiError> {
    // The percent-encoding serializer holds a non-Sync encoder; build the
    // path in its own scope so it is dropped before the first await.
    let path = {
        let mut pairs = url::form_urlencoded::Serializer::new(String::new());
        pairs.append_pair("status", status);
        if !query.trim().is_empty() {
            pairs.append_pair("q", query.trim());
        }
        if !category.trim().is_empty() {
            pairs.append_pair("category", category.trim());
        }
        format!("/api/employees?{}", pairs.finish())
    };
    let envelope = request::<Vec<Employee>>(api, reqwest::Method::GET, &path, None).await?;
    if envelope.data.is_none() {
        return Err(ApiError::transport("Réponse des employés incomplète."));
    }
    Ok(EmployeesList {
        rows: envelope.data.unwrap_or_default(),
        categories: envelope.categories.unwrap_or_default(),
    })
}

pub(crate) async fn detail(api: &ApiClient, id: i64) -> Result<EmployeeDetail, ApiError> {
    if id <= 0 {
        return Err(ApiError::client("Identifiant employé invalide."));
    }
    let envelope = request::<EmployeeDetail>(
        api,
        reqwest::Method::GET,
        &format!("/api/employees/{id}"),
        None,
    )
    .await?;
    let detail = envelope
        .data
        .ok_or_else(|| ApiError::transport("Détail employé incomplet."))?;
    if detail.employee.id != id {
        return Err(ApiError::client(
            "L’employé reçu ne correspond pas à la demande.",
        ));
    }
    Ok(detail)
}

pub(crate) async fn pay(
    api: &ApiClient,
    id: i64,
    amount: f64,
    paid_at: &str,
    note: &str,
) -> Result<Action, ApiError> {
    if id <= 0 || !amount.is_finite() || amount <= 0.0 {
        return Err(ApiError::client("Montant de paiement invalide."));
    }
    if !valid_date(paid_at) {
        return Err(ApiError::client("Date de paiement invalide."));
    }
    action(
        api,
        reqwest::Method::POST,
        &format!("/api/employees/{id}/pay"),
        Some(
            serde_json::to_value(PayBody {
                amount,
                paid_at,
                note,
                payment_method: "cash",
            })
            .map_err(|_| ApiError::client("Requête de paiement invalide."))?,
        ),
    )
    .await
}

pub(crate) async fn advance(
    api: &ApiClient,
    id: i64,
    amount: f64,
    note: &str,
) -> Result<Action, ApiError> {
    if id <= 0 || !amount.is_finite() || amount <= 0.0 {
        return Err(ApiError::client("Montant d’acompte invalide."));
    }
    action(
        api,
        reqwest::Method::POST,
        &format!("/api/employees/{id}/advance"),
        Some(serde_json::json!({
            "amount": amount,
            "note": note,
            "payment_method": "cash",
        })),
    )
    .await
}

pub(crate) async fn resign(api: &ApiClient, id: i64, reason: &str) -> Result<Action, ApiError> {
    if id <= 0 {
        return Err(ApiError::client("Identifiant employé invalide."));
    }
    let reason = reason.trim();
    if reason.is_empty() || reason.chars().count() > 200 {
        return Err(ApiError::client("Motif de démission invalide."));
    }
    action(
        api,
        reqwest::Method::POST,
        &format!("/api/employees/{id}/resign"),
        Some(
            serde_json::to_value(ResignBody {
                reason,
                settle_pay: false,
            })
            .map_err(|_| ApiError::client("Requête de démission invalide."))?,
        ),
    )
    .await
}

pub(crate) async fn reactivate(api: &ApiClient, id: i64) -> Result<Action, ApiError> {
    if id <= 0 {
        return Err(ApiError::client("Identifiant employé invalide."));
    }
    action(
        api,
        reqwest::Method::POST,
        &format!("/api/employees/{id}/reactivate"),
        Some(serde_json::json!({})),
    )
    .await
}

/// The worker derives `name` from the two parts and applies its own defaults.
pub(crate) async fn create(api: &ApiClient, form: &NewEmployee) -> Result<Action, ApiError> {
    if form.first_name.trim().is_empty() && form.last_name.trim().is_empty() {
        return Err(ApiError::client("Le nom est obligatoire."));
    }
    if !form.pay_amount.is_finite() || form.pay_amount < 0.0 {
        return Err(ApiError::client("Salaire invalide."));
    }
    action(
        api,
        reqwest::Method::POST,
        "/api/employees",
        Some(serde_json::to_value(form).map_err(|_| ApiError::client("Fiche invalide."))?),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn new_employee(
    first_name: &str,
    last_name: &str,
    phone: &str,
    email: &str,
    role_title: &str,
    department: &str,
    category: &str,
    national_id: &str,
    city: &str,
    pay_amount: f64,
    pay_cycle_type: &str,
    pay_cycle_days: i64,
    notes: &str,
) -> NewEmployee {
    NewEmployee {
        first_name: first_name.to_owned(),
        last_name: last_name.to_owned(),
        name: format!("{first_name} {last_name}").trim().to_owned(),
        phone: phone.to_owned(),
        email: email.to_owned(),
        role_title: role_title.to_owned(),
        department: department.to_owned(),
        category: category.to_owned(),
        national_id: national_id.to_owned(),
        city: city.to_owned(),
        pay_amount,
        pay_cycle_type: pay_cycle_type.to_owned(),
        pay_cycle_days: if pay_cycle_type == "days" {
            pay_cycle_days.clamp(1, 365)
        } else {
            30
        },
        notes: notes.to_owned(),
    }
}

/// Strict `YYYY-MM-DD`, also rejecting impossible calendar days.
fn valid_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return false;
    }
    let year = value[0..4].parse::<i32>().unwrap_or(0);
    let month = value[5..7].parse::<u32>().unwrap_or(0);
    let day = value[8..10].parse::<u32>().unwrap_or(0);
    let max = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => return false,
    };
    (1900..=2200).contains(&year) && (1..=max).contains(&day)
}

/// Print/report paths the worker's 30-minute ticket accepts. Kept as an
/// explicit allowlist so a server-provided string can never become a
/// generic authenticated URL.
pub(crate) fn printable_path(path: &str) -> bool {
    const FIXED: [&str; 5] = [
        "/api/employees/form/print",
        "/api/reports/employee-situation",
        "/api/reports/client-situation",
        "/api/reports/receivables",
        "/api/reports/payables",
    ];
    if FIXED.contains(&path) {
        return true;
    }
    let Some(rest) = path.strip_prefix("/api/") else {
        return false;
    };
    let mut parts = rest.split('/');
    let (Some(kind), Some(id)) = (parts.next(), parts.next()) else {
        return false;
    };
    let tail = parts.collect::<Vec<_>>().join("/");
    if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    matches!(
        (kind, tail.as_str()),
        ("employees", "form/print") | ("invoices" | "bills" | "receipts", "print" | "pdf")
    )
}

#[cfg(test)]
mod tests {
    use super::{Employee, printable_path, valid_date};

    #[test]
    fn dates_are_strict_calendar_days() {
        for value in ["2026-10-22", "2024-02-29", "2000-02-29"] {
            assert!(valid_date(value), "{value}");
        }
        for value in [
            "2026-02-29",
            "2026-13-01",
            "2026-00-10",
            "2026-04-31",
            "2026-1-01",
            "2026-10-22T00:00",
            "",
        ] {
            assert!(!valid_date(value), "{value}");
        }
    }

    #[test]
    fn print_allowlist_matches_the_worker_scope() {
        for path in [
            "/api/employees/form/print",
            "/api/employees/42/form/print",
            "/api/invoices/7/print",
            "/api/invoices/7/pdf",
            "/api/bills/3/print",
            "/api/receipts/9/pdf",
            "/api/reports/employee-situation",
            "/api/reports/client-situation",
            "/api/reports/receivables",
            "/api/reports/payables",
        ] {
            assert!(printable_path(path), "{path}");
        }
        for path in [
            "/api/employees/42",
            "/api/employees/42/pay",
            "/api/employees/42/form/print/extra",
            "/api/invoices/7/print/../../pay",
            "/api/reports/../employees/42/pay",
            "https://example.test/api/invoices/7/print",
            "/api/settings/invoice-preview",
            "/api/invoices//print",
        ] {
            assert!(!printable_path(path), "{path}");
        }
    }

    #[test]
    fn employee_defaults_do_not_look_active() {
        let employee: Employee = serde_json::from_str("{}").unwrap();
        assert!(!employee.is_active());
        assert_eq!(employee.label(), "");
        let employee: Employee = serde_json::from_str(
            r#"{"id":1,"display_name":"Amina","active":1,"pay_cycle_type":"days","cycle_days":7}"#,
        )
        .unwrap();
        assert!(employee.is_active());
        assert_eq!(employee.label(), "Amina");
        assert_eq!(employee.cycle_label(), "Tous les 7 j");
    }
}
