use std::collections::{HashMap, HashSet};

use bencher_json::system::config::{JsonProduct, JsonProducts};
use stripe::Client as StripeClient;
use stripe_product::{
    Price as StripePrice, PriceId, Product as StripeProduct, ProductId, price::RetrievePrice,
    product::RetrieveProduct,
};

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
