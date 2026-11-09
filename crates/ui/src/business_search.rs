//! User-initiated business search over existing read endpoints.
//! Products use the worker's query and limit; clients and documents are
//! fetched as lists and filtered locally. Failed categories are reported.
use crate::state::Route;
use atlas_api::{ApiClient, ApiError};

#[derive(Clone, Debug)]
pub(crate) struct SearchHit {
    pub route: Route,
    pub id: i64,
    pub label: String,
    pub subtitle: String,
    pub receipt: bool,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct SearchResults {
    pub hits: Vec<SearchHit>,
    pub failed: Vec<&'static str>,
}
fn contains(query: &str, fields: &str) -> bool {
    crate::shortcuts::normalize(fields).contains(query)
}
pub(crate) async fn search(
    api: &ApiClient,
    query: String,
    products: bool,
    clients: bool,
    sales: bool,
    vendors: bool,
) -> Result<SearchResults, ApiError> {
    let needle = crate::shortcuts::normalize(query.trim());
    if needle.is_empty() {
        return Err(ApiError::client("Saisissez une recherche."));
    }
    let (p, c, i, v, r) = tokio::join!(
        async {
            if products {
                Some(api.products(&query, 200).await)
            } else {
                None
            }
        },
        async {
            if clients {
                Some(api.clients().await)
            } else {
                None
            }
        },
        async {
            if sales {
                Some(api.invoices("all").await)
            } else {
                None
            }
        },
        async {
            if vendors {
                Some(api.vendors().await)
            } else {
                None
            }
        },
        async {
            if sales {
                Some(crate::client_invoices::fetch_receipts(api).await)
            } else {
                None
            }
        },
    );
    let mut result = SearchResults::default();
    if let Some(p) = p {
        match p {
            Ok(rows) => {
                for row in rows.into_iter().take(200) {
                    result.hits.push(SearchHit {
                        route: Route::Products,
                        id: row.id,
                        label: row.name,
                        subtitle: row.sku.unwrap_or_default(),
                        receipt: false,
                    });
                }
            }
            Err(_) => result.failed.push("Products"),
        }
    }
    if let Some(c) = c {
        match c {
            Ok(rows) => {
                for row in rows
                    .into_iter()
                    .filter(|r| {
                        contains(
                            &needle,
                            &format!("{} {} {} {}", r.name, r.phone, r.ice, r.email),
                        )
                    })
                    .take(40)
                {
                    result.hits.push(SearchHit {
                        route: Route::Clients,
                        id: row.id,
                        label: row.name,
                        subtitle: row.phone,
                        receipt: false,
                    });
                }
            }
            Err(_) => result.failed.push("Clients"),
        }
    }
    if let Some(i) = i {
        match i {
            Ok(rows) => {
                for row in rows
                    .into_iter()
                    .filter(|r| {
                        contains(
                            &needle,
                            &format!(
                                "{} {}",
                                r.number.as_deref().unwrap_or(""),
                                r.client_name.as_deref().unwrap_or("")
                            ),
                        )
                    })
                    .take(40)
                {
                    result.hits.push(SearchHit {
                        route: Route::Sales,
                        id: row.id,
                        label: row.number.unwrap_or_else(|| format!("#{}", row.id)),
                        subtitle: row.client_name.unwrap_or_default(),
                        receipt: false,
                    });
                }
            }
            Err(_) => result.failed.push("Sales"),
        }
    }
    if let Some(v) = v {
        match v {
            Ok(rows) => {
                for row in rows
                    .into_iter()
                    .filter(|r| {
                        contains(
                            &needle,
                            &format!("{} {} {} {}", r.name, r.phone, r.ice, r.email),
                        )
                    })
                    .take(40)
                {
                    result.hits.push(SearchHit {
                        route: Route::Vendors,
                        id: row.id,
                        label: row.name,
                        subtitle: row.phone,
                        receipt: false,
                    });
                }
            }
            Err(_) => result.failed.push("Vendors"),
        }
    }
    if let Some(r) = r {
        match r {
            Ok(rows) => {
                for row in rows
                    .into_iter()
                    .filter(|r| {
                        contains(
                            &needle,
                            &format!(
                                "{} {} {}",
                                r.number,
                                r.client_name,
                                r.invoice_number.as_deref().unwrap_or("")
                            ),
                        )
                    })
                    .take(40)
                {
                    result.hits.push(SearchHit {
                        route: Route::Sales,
                        id: row.id,
                        label: row.number,
                        subtitle: row.client_name,
                        receipt: true,
                    });
                }
            }
            Err(_) => result.failed.push("Receipts"),
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    #[test]
    fn matching_ignores_accents_and_case() {
        assert!(super::contains("cafe", "CAFÉ du centre"));
        assert!(!super::contains("autre", "CAFÉ"));
    }
}
