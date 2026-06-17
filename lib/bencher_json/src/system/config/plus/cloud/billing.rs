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
    // Legacy self-serve paid tier, retained for grandfathered customers.
    pub team: JsonProduct,
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
    // Flat recurring base prices (e.g. the Pro `$20/mo` platform fee).
    // Empty for products with no flat base fee (team, enterprise, bare_metal).
    #[serde(default)]
    pub base: HashMap<String, String>,
    // Stripe coupon ID (`duration: once`, `percent_off: 100`) applied to new
    // subscriptions for this product to waive the first month's base fee (the
    // free trial). Metered usage still bills, offset by the monthly credit.
    // Only the Pro product sets this today.
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
            "team": {"id":"t","metered":{},"licensed":{}},
            "pro": {"id":"p","metered":{"default":"price_m"},"base":{"default":"price_b"},"trial_coupon":"coupon_x"},
            "enterprise": {"id":"e","metered":{},"licensed":{}},
            "bare_metal": {"id":"bm","metered":{},"licensed":{}}
        }"#;
        let products: JsonProducts = serde_json::from_str(json).unwrap();
        assert_eq!(
            products.pro.base.get("default").map(String::as_str),
            Some("price_b"),
        );
        assert_eq!(products.pro.trial_coupon.as_deref(), Some("coupon_x"));
        assert!(products.team.base.is_empty());
        assert!(products.team.trial_coupon.is_none());
    }
}
