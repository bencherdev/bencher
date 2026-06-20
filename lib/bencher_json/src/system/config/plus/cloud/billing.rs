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
    // Holds the Pro plan's metered metrics price. Kept as its own product so
    // checkout shows a distinct "Bencher Metrics" line item instead of a second
    // "Bencher Pro".
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
    // Flat recurring base prices.
    // Empty for products with no flat base fee.
    #[serde(default)]
    pub base: HashMap<String, String>,
    #[serde(default)]
    pub trial_coupon: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{JsonProduct, JsonProducts};

    // Existing team/enterprise/bare_metal config has no `base` key; it must still
    // deserialize, defaulting `base` to an empty map (backward compatibility).
    #[test]
    fn product_without_base_defaults_empty() {
        let json =
            r#"{"id":"prod_x","metered":{"default":"price_m"},"licensed":{"default":"price_l"}}"#;
        let product: JsonProduct = serde_json::from_str(json).unwrap();
        assert!(product.base.is_empty());
        assert!(product.trial_coupon.is_none());
        assert_eq!(
            product.metered.get("default").map(String::as_str),
            Some("price_m"),
        );
    }

    #[test]
    fn products_with_pro_base_and_trial_coupon() {
        let json = r#"{
            "pro": {"id":"p","base":{"default":"price_b"},"trial_coupon":"coupon_x"},
            "team": {"id":"t","metered":{},"licensed":{}},
            "enterprise": {"id":"e","metered":{},"licensed":{}},
            "metrics": {"id":"m","metered":{"default":"price_mm"},"licensed":{"default":"price_ml"}},
            "bare_metal": {"id":"bm","metered":{},"licensed":{}}
        }"#;
        let products: JsonProducts = serde_json::from_str(json).unwrap();
        assert_eq!(
            products.pro.base.get("default").map(String::as_str),
            Some("price_b"),
        );
        assert_eq!(products.pro.trial_coupon.as_deref(), Some("coupon_x"));
        // The Pro plan's metered metrics price now lives on the `metrics` product.
        assert!(products.pro.metered.is_empty());
        assert_eq!(
            products.metrics.metered.get("default").map(String::as_str),
            Some("price_mm"),
        );
        assert!(products.team.base.is_empty());
        assert!(products.team.trial_coupon.is_none());
    }
}
