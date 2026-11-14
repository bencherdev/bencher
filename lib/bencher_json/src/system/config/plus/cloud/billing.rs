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

/// The Stripe products a subscription's items belong to. The product of a
/// subscription's plan item names its plan level.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProducts {
    /// The Pro product, with the prices a self-serve checkout sells.
    pub pro: JsonProduct,
    /// The Enterprise product. Each deal has its own price on it.
    pub enterprise: JsonEnterpriseProduct,
    /// The bare metal product, with the prices a self-serve checkout adds.
    pub bare_metal: JsonProduct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProduct {
    pub id: String,
    #[serde(default)]
    pub metered: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonEnterpriseProduct {
    pub id: String,
}

#[cfg(test)]
mod tests {
    use super::{JsonBilling, JsonProduct, JsonProducts};

    // A product given only an `id` still deserializes, defaulting `metered` to an
    // empty map (backward compatibility).
    #[test]
    fn product_without_pricing_defaults_empty() {
        let json = r#"{"id":"prod_x"}"#;
        let product: JsonProduct = serde_json::from_str(json).unwrap();
        assert!(product.metered.is_empty());
    }

    // Pro carries a single tiered metered price (base fee plus per-series step-ups), and
    // the Enterprise product needs only its id.
    #[test]
    fn products_with_pro_tiered_price() {
        let json = r#"{
            "pro": {"id":"p","metered":{"default":"price_pro_tiered"}},
            "enterprise": {"id":"e"},
            "bare_metal": {"id":"bm"}
        }"#;
        let products: JsonProducts = serde_json::from_str(json).unwrap();
        assert_eq!(
            products.pro.metered.get("default").map(String::as_str),
            Some("price_pro_tiered"),
        );
        assert_eq!(products.enterprise.id, "e");
        assert!(products.bare_metal.metered.is_empty());
    }

    // A configuration that still names the retired `team` and `metrics` products, or
    // a licensed price on any product, loads, so the server can deploy before its
    // configuration drops those keys.
    #[test]
    fn billing_ignores_retired_products() {
        let json = r#"{
            "secret_key": "sk_test_key",
            "products": {
                "pro": {"id":"p","metered":{"default":"price_pro_tiered"}},
                "team": {"id":"t","metered":{"default":"price_tm"},"licensed":{"default":"price_tl"}},
                "enterprise": {"id":"e","metered":{"default":"price_em"},"licensed":{"default":"price_el"}},
                "metrics": {"id":"m","metered":{"default":"price_mm"},"licensed":{"default":"price_ml"}},
                "bare_metal": {"id":"bm","metered":{"default":"price_bm"},"licensed":{"default":"price_bl"}}
            }
        }"#;
        let billing: JsonBilling = serde_json::from_str(json).unwrap();

        assert_eq!(
            serde_json::to_value(&billing.products).unwrap(),
            serde_json::json!({
                "pro": {"id":"p","metered":{"default":"price_pro_tiered"}},
                "enterprise": {"id":"e"},
                "bare_metal": {"id":"bm","metered":{"default":"price_bm"}}
            })
        );
    }
}
