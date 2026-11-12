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
    pub enterprise: JsonProduct,
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
    use super::{JsonBilling, JsonProduct, JsonProducts};

    // A product given only an `id` still deserializes, defaulting `metered` and
    // `licensed` to empty maps (backward compatibility).
    #[test]
    fn product_without_pricing_defaults_empty() {
        let json = r#"{"id":"prod_x"}"#;
        let product: JsonProduct = serde_json::from_str(json).unwrap();
        assert!(product.metered.is_empty());
        assert!(product.licensed.is_empty());
    }

    // Pro carries a single tiered metered price (base fee plus per-series step-ups).
    #[test]
    fn products_with_pro_tiered_price() {
        let json = r#"{
            "pro": {"id":"p","metered":{"default":"price_pro_tiered"}},
            "enterprise": {"id":"e","metered":{},"licensed":{}},
            "bare_metal": {"id":"bm","metered":{},"licensed":{}}
        }"#;
        let products: JsonProducts = serde_json::from_str(json).unwrap();
        assert_eq!(
            products.pro.metered.get("default").map(String::as_str),
            Some("price_pro_tiered"),
        );
        assert!(products.pro.licensed.is_empty());
        assert!(products.enterprise.metered.is_empty());
    }

    // A configuration that still names the retired `team` and `metrics` products
    // loads, so the server can deploy before its configuration drops those keys.
    #[test]
    fn billing_ignores_retired_products() {
        let json = r#"{
            "secret_key": "sk_test_key",
            "products": {
                "pro": {"id":"p","metered":{"default":"price_pro_tiered"}},
                "team": {"id":"t","metered":{"default":"price_tm"},"licensed":{"default":"price_tl"}},
                "enterprise": {"id":"e","licensed":{"default":"price_el"}},
                "metrics": {"id":"m","metered":{"default":"price_mm"},"licensed":{"default":"price_ml"}},
                "bare_metal": {"id":"bm","metered":{"default":"price_bm"}}
            }
        }"#;
        let billing: JsonBilling = serde_json::from_str(json).unwrap();

        let products = serde_json::to_value(&billing.products).unwrap();
        let mut keys: Vec<&str> = products
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["bare_metal", "enterprise", "pro"]);

        let JsonBilling { products, .. } = billing;
        assert_eq!(
            products.pro.metered.get("default").map(String::as_str),
            Some("price_pro_tiered"),
        );
        assert_eq!(
            products
                .enterprise
                .licensed
                .get("default")
                .map(String::as_str),
            Some("price_el"),
        );
        assert_eq!(
            products
                .bare_metal
                .metered
                .get("default")
                .map(String::as_str),
            Some("price_bm"),
        );
    }
}
