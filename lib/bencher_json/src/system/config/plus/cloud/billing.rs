use std::collections::HashMap;

use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBilling {
    pub secret_key: Secret,
    pub products: JsonProducts,
}

impl Sanitize for JsonBilling {
    fn sanitize(&mut self) {
        self.secret_key.sanitize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProducts {
    pub pro: JsonProduct,
    // Legacy self-serve paid tier, retained for grandfathered customers.
    pub team: JsonProduct,
    pub enterprise: JsonProduct,
    // Shared metered metrics product for legacy Team/Enterprise plans. Pro bills on
    // its own tiered active-series price (its `metered` map), not on this meter.
    pub metrics: JsonProduct,
    pub bare_metal: JsonProduct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProduct {
    pub id: String,
    #[serde(default)]
    pub metered: HashMap<String, String>,
    #[serde(default)]
    pub licensed: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::{JsonProduct, JsonProducts};

    // A product given only an `id` still deserializes, defaulting `metered` and
    // `licensed` to empty maps (backward compatibility).
    #[test]
    fn product_without_pricing_defaults_empty() {
        let json = r#"{"id":"prod_x"}"#;
        let product: JsonProduct = serde_json::from_str(json).unwrap();
        assert!(product.metered.is_empty());
        assert!(product.licensed.is_empty());
    }

    // Pro carries a single tiered metered price (base fee plus per-series step-ups);
    // the shared metrics product carries the legacy Team/Enterprise metered price.
    #[test]
    fn products_with_pro_tiered_price() {
        let json = r#"{
            "pro": {"id":"p","metered":{"default":"price_pro_tiered"}},
            "team": {"id":"t","metered":{},"licensed":{}},
            "enterprise": {"id":"e","metered":{},"licensed":{}},
            "metrics": {"id":"m","metered":{"default":"price_mm"},"licensed":{"default":"price_ml"}},
            "bare_metal": {"id":"bm","metered":{},"licensed":{}}
        }"#;
        let products: JsonProducts = serde_json::from_str(json).unwrap();
        assert_eq!(
            products.pro.metered.get("default").map(String::as_str),
            Some("price_pro_tiered"),
        );
        assert!(products.pro.licensed.is_empty());
        assert_eq!(
            products.metrics.metered.get("default").map(String::as_str),
            Some("price_mm"),
        );
        assert!(products.team.metered.is_empty());
    }
}
