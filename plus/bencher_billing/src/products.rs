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
    pub pro: Product,
    // Legacy self-serve paid tier, retained for grandfathered customers.
    pub team: Product,
    pub enterprise: Product,
    // Holds the Pro plan's metered metrics price (its own product so checkout
    // shows a distinct "Bencher Metrics" line item).
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

    // The price IDs that identify a subscription's plan item (excluding flat base
    // fees and bare_metal), used by `get_plan` to filter subscription items down to
    // that one item. These are the metered + licensed prices across the `metrics`
    // (shared metered metrics), `team`, and `enterprise` products. The Team and
    // Enterprise metered prices are retained so a subscription still on its own
    // product (not yet moved to `metrics`) continues to resolve.
    pub fn plan_price_ids(&self) -> HashSet<&PriceId> {
        [&self.metrics, &self.team, &self.enterprise]
            .into_iter()
            .flat_map(|product| product.metered.values().chain(product.licensed.values()))
            .map(|price| &price.id)
            .collect()
    }

    /// All configured flat base prices across every product. The included usage
    /// credit equals the base fee of whichever base price a subscription carries.
    pub fn all_base_prices(&self) -> impl Iterator<Item = &StripePrice> {
        self.pro
            .base
            .values()
            .chain(self.team.base.values())
            .chain(self.enterprise.base.values())
            .chain(self.bare_metal.base.values())
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
    // Stripe coupon ID for this product's free trial (waives the first month's
    // base fee). `None` if the product has no trial.
    pub trial_coupon: Option<String>,
    // Flat recurring base prices. Excluded from `plan_price_ids` (never the metered
    // "plan item"), but used by `get_plan` to detect the Pro base fee and so resolve
    // the plan level.
    pub base: HashMap<String, StripePrice>,
    pub metered: HashMap<String, StripePrice>,
    pub licensed: HashMap<String, StripePrice>,
}

impl Product {
    async fn new(client: &StripeClient, product: JsonProduct) -> Result<Self, BillingError> {
        let JsonProduct {
            id,
            metered,
            licensed,
            base,
            trial_coupon,
        } = product;

        let product_id: ProductId = id.as_str().into();
        let product = RetrieveProduct::new(product_id).send(client).await?;
        let metered = Self::pricing(client, metered).await?;
        let licensed = Self::pricing(client, licensed).await?;
        let base = Self::pricing(client, base).await?;

        Ok(Self {
            product,
            trial_coupon,
            base,
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
            let price = RetrievePrice::new(price_id).send(client).await?;
            biller_pricing.insert(price_name, price);
        }
        Ok(biller_pricing)
    }
}
