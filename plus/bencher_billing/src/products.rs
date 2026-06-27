use std::collections::{HashMap, HashSet};

use bencher_json::system::config::{JsonProduct, JsonProducts};
use bencher_json::{BigInt, JsonPriceTier};
use stripe::Client as StripeClient;
use stripe_product::{
    Price as StripePrice, PriceId, Product as StripeProduct, ProductId, price::RetrievePrice,
    product::RetrieveProduct,
};
use stripe_shared::PriceTier;

use crate::BillingError;

#[derive(Clone)]
pub struct Products {
    // Bencher Cloud self-serve tier. Its `metered` map holds the single tiered
    // active-series price (tier 1 flat base fee plus per-series step-ups) that bills
    // both the base monthly fee and the per-series overage.
    pub pro: Product,
    // Legacy self-serve paid tier, retained for grandfathered customers.
    pub team: Product,
    pub enterprise: Product,
    // Shared metered metrics product for legacy Team/Enterprise plans. Pro bills on
    // its own tiered active-series price, not on this meter.
    pub metrics: Product,
    pub bare_metal: Product,
}

impl Products {
    pub async fn new(client: &StripeClient, products: JsonProducts) -> Result<Self, BillingError> {
        let JsonProducts {
            pro,
            team,
            enterprise,
            metrics,
            bare_metal,
        } = products;

        Ok(Self {
            pro: Product::new(client, pro).await?,
            team: Product::new(client, team).await?,
            enterprise: Product::new(client, enterprise).await?,
            metrics: Product::new(client, metrics).await?,
            bare_metal: Product::new(client, bare_metal).await?,
        })
    }

    // The price IDs that identify a subscription's plan item (excluding bare_metal),
    // used by `get_plan` to filter subscription items down to that one item. These are
    // the metered + licensed prices across the `pro` (tiered active-series), `metrics`
    // (shared metered metrics), `team`, and `enterprise` products. The Team and
    // Enterprise metered prices are retained so a subscription still on its own
    // product (not yet moved to `metrics`) continues to resolve.
    pub fn plan_price_ids(&self) -> HashSet<&PriceId> {
        [&self.pro, &self.metrics, &self.team, &self.enterprise]
            .into_iter()
            .flat_map(|product| product.metered.values().chain(product.licensed.values()))
            .map(|price| &price.id)
            .collect()
    }

    /// The base monthly fee (in cents) for a tiered price, read from its first tier's
    /// `flat_amount`. The Pro tiered active-series price exposes no flat `unit_amount`,
    /// so `get_plan` reports this as the plan's `unit_amount`. `None` if the price is
    /// unknown or not tiered. Relies on prices being retrieved with `tiers` expanded
    /// (see [`Product::pricing`]).
    pub fn tier_base_fee_cents(&self, price_id: &PriceId) -> Option<i64> {
        [
            &self.pro,
            &self.metrics,
            &self.team,
            &self.enterprise,
            &self.bare_metal,
        ]
        .into_iter()
        .flat_map(|product| product.metered.values().chain(product.licensed.values()))
        .find(|price| &price.id == price_id)
        .and_then(|price| price.tiers.as_ref()?.first()?.flat_amount)
    }

    /// The full tiered price ladder for `price_id`, mapped to JSON so the Console can
    /// render it from the billed source of truth instead of hardcoding it. `None` if the
    /// price is unknown or not tiered. Relies on prices retrieved with `tiers` expanded
    /// (see [`Product::pricing`]).
    pub fn price_tiers(&self, price_id: &PriceId) -> Option<Vec<JsonPriceTier>> {
        [
            &self.pro,
            &self.metrics,
            &self.team,
            &self.enterprise,
            &self.bare_metal,
        ]
        .into_iter()
        .flat_map(|product| product.metered.values().chain(product.licensed.values()))
        .find(|price| &price.id == price_id)
        .and_then(|price| price.tiers.as_ref())
        .map(|tiers| tiers.iter().map(to_json_tier).collect())
    }
}

/// Map a Stripe price tier to its JSON form: `up_to` is the inclusive series upper bound
/// (`None` is the unbounded top tier), and `unit_amount`/`flat_amount` are mapped
/// independently from cents (a tier may carry both, additively).
fn to_json_tier(tier: &PriceTier) -> JsonPriceTier {
    let cents = |amount: Option<i64>| amount.and_then(|c| u64::try_from(c).ok()).map(BigInt::from);
    JsonPriceTier {
        up_to: tier.up_to.and_then(|n| u32::try_from(n).ok()),
        unit_amount: cents(tier.unit_amount),
        flat_amount: cents(tier.flat_amount),
    }
}

#[derive(Clone)]
pub struct Product {
    #[expect(
        dead_code,
        clippy::struct_field_names,
        reason = "retained for future Stripe API use"
    )]
    pub product: StripeProduct,
    pub metered: HashMap<String, StripePrice>,
    pub licensed: HashMap<String, StripePrice>,
}

impl Product {
    async fn new(client: &StripeClient, product: JsonProduct) -> Result<Self, BillingError> {
        let JsonProduct {
            id,
            metered,
            licensed,
        } = product;

        let product_id: ProductId = id.as_str().into();
        let product = RetrieveProduct::new(product_id).send(client).await?;
        let metered = Self::pricing(client, metered).await?;
        let licensed = Self::pricing(client, licensed).await?;

        Ok(Self {
            product,
            metered,
            licensed,
        })
    }

    async fn pricing(
        client: &StripeClient,
        pricing: HashMap<String, String>,
    ) -> Result<HashMap<String, StripePrice>, BillingError> {
        let mut biller_pricing = HashMap::with_capacity(pricing.len());
        for (price_name, price_id) in pricing {
            let price_id: PriceId = price_id.as_str().into();
            // Expand `tiers` so a tiered price (the Pro active-series price) exposes its
            // tier `flat_amount` (the base monthly fee). Harmless for non-tiered prices.
            let price = RetrievePrice::new(price_id)
                .expand(vec!["tiers".to_owned()])
                .send(client)
                .await?;
            biller_pricing.insert(price_name, price);
        }
        Ok(biller_pricing)
    }
}

#[cfg(test)]
mod tests {
    use stripe_shared::PriceTier;

    use super::to_json_tier;

    fn price_tier(
        up_to: Option<i64>,
        unit_amount: Option<i64>,
        flat_amount: Option<i64>,
    ) -> PriceTier {
        PriceTier {
            flat_amount,
            flat_amount_decimal: None,
            unit_amount,
            unit_amount_decimal: None,
            up_to,
        }
    }

    #[test]
    fn to_json_tier_flat_only() {
        let json = to_json_tier(&price_tier(Some(250), None, Some(10_000)));
        assert_eq!(json.up_to, Some(250));
        assert_eq!(json.unit_amount.map(u64::from), None);
        assert_eq!(json.flat_amount.map(u64::from), Some(10_000));
    }

    #[test]
    fn to_json_tier_unit_only() {
        let json = to_json_tier(&price_tier(Some(500), Some(50), None));
        assert_eq!(json.up_to, Some(500));
        assert_eq!(json.unit_amount.map(u64::from), Some(50));
        assert_eq!(json.flat_amount.map(u64::from), None);
    }

    #[test]
    fn to_json_tier_flat_and_unit() {
        // Stripe allows a tier to carry both a flat fee and a per-unit amount.
        let json = to_json_tier(&price_tier(Some(375), Some(25), Some(15_000)));
        assert_eq!(json.up_to, Some(375));
        assert_eq!(json.unit_amount.map(u64::from), Some(25));
        assert_eq!(json.flat_amount.map(u64::from), Some(15_000));
    }

    #[test]
    fn to_json_tier_unbounded_top() {
        // The unbounded top tier (`up_to: None`) is presented as "Get in Touch".
        let json = to_json_tier(&price_tier(None, None, Some(20_000)));
        assert_eq!(json.up_to, None);
        assert_eq!(json.flat_amount.map(u64::from), Some(20_000));
    }
}
