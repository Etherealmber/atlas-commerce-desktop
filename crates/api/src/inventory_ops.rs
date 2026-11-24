//! Selected inventory operations. Contracts follow Worker `routes/inventoryOps.ts`.
//! Mutations are never retried automatically: a lost response is ambiguous.

use crate::models::{ApiStatus, Product};
use crate::{ApiClient, ApiError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct StockLocation {
    pub id: i64,
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub is_default: i64,
    #[serde(default)]
    pub total_units: Option<f64>,
    #[serde(default)]
    pub sku_count: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct InventoryOverview {
    pub locations: Vec<StockLocation>,
    pub products: Vec<Product>,
}

#[derive(Serialize)]
struct NewLocation<'a> {
    code: &'a str,
    name: &'a str,
}

#[derive(Serialize)]
struct CountRequest<'a> {
    product_id: i64,
    counted_qty: f64,
    note: &'a str,
}

impl ApiClient {
    pub async fn inventory_overview(&self) -> Result<InventoryOverview, ApiError> {
        let (locations, products) = tokio::join!(
            self.get_data::<Vec<StockLocation>>("/api/inventory/locations"),
            self.catalog_products("", 300),
        );
        Ok(InventoryOverview {
            locations: locations?,
            products: products?,
        })
    }

    pub async fn create_stock_location(
        &self,
        code: &str,
        name: &str,
    ) -> Result<StockLocation, ApiError> {
        let code = code.trim();
        let name = name.trim();
        if code.is_empty()
            || name.is_empty()
            || code.chars().count() > 16
            || name.chars().count() > 80
        {
            return Err(ApiError::client("Code ou nom d’emplacement invalide."));
        }
        self.post_data("/api/inventory/locations", &NewLocation { code, name })
            .await
    }

    /// Count the consolidated product stock, without an optional location.
    /// The Worker computes and applies the delta to global stock.
    pub async fn cycle_count(
        &self,
        product_id: i64,
        counted_qty: f64,
        note: &str,
    ) -> Result<(), ApiError> {
        if product_id <= 0
            || !counted_qty.is_finite()
            || counted_qty < 0.0
            || note.chars().count() > 200
        {
            return Err(ApiError::client("Produit, quantité ou note invalide."));
        }
        let response: ApiStatus = self
            .post_json(
                "/api/inventory/cycle-count",
                &CountRequest {
                    product_id,
                    counted_qty,
                    note,
                },
            )
            .await?;
        if response.success {
            Ok(())
        } else {
            Err(ApiError {
                status: 200,
                code: response.code,
                message: response
                    .error
                    .unwrap_or_else(|| "Comptage refusé par le serveur.".to_owned()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn location_response_accepts_optional_totals() {
        let loc: StockLocation =
            serde_json::from_str(r#"{"id":1,"code":"MAG","name":"Magasin","is_default":1}"#)
                .unwrap();
        assert_eq!(loc.id, 1);
        assert_eq!(loc.total_units, None);
    }
}
