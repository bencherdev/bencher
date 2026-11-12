use std::collections::HashMap;

use bencher_json::system::config::{JsonEnterpriseProduct, JsonProduct, JsonProducts};
use bencher_json::{BigInt, JsonPriceTier, PlanLevel};
use stripe::Client as StripeClient;
use stripe_product::{
    Price as StripePrice, PriceId, ProductId, price::RetrievePrice, product::RetrieveProduct,
};
use stripe_shared::PriceTier;

use crate::BillingError;

/// The Stripe products a subscription's items belong to, with the prices a self-serve
/// checkout sells.
#[derive(Clone)]
pub struct Products {
    /// The Pro product, whose prices are the tiered active-series prices a self-serve
    /// checkout sells.
    pub pro: Product,
    /// The Enterprise product. Each deal has its own price on it.
    pub enterprise: ProductId,
    /// The bare metal product, whose prices a self-serve checkout adds for runner
    /// minutes.
    pub bare_metal: Product,
}

impl Products {
    pub async fn new(client: &StripeClient, products: JsonProducts) -> Result<Self, BillingError> {
        let JsonProducts {
            pro,
            enterprise: JsonEnterpriseProduct { id: enterprise },
            bare_metal,
        } = products;

        Ok(Self {
            pro: Product::new(client, pro).await?,
            enterprise: retrieve_product(client, &enterprise).await?,
            bare_metal: Product::new(client, bare_metal).await?,
        })
    }

    /// The plan level that a price on `product` bills, or `None` when `product` names
    /// no plan.
    pub fn plan_level(&self, product: &ProductId) -> Option<PlanLevel> {
        if *product == self.pro.id {
            Some(PlanLevel::Pro)
        } else if *product == self.enterprise {
            Some(PlanLevel::Enterprise)
        } else {
            None
        }
    }

    pub fn is_bare_metal(&self, product: &ProductId) -> bool {
        *product == self.bare_metal.id
    }
}

/// Map a Stripe price tier to its JSON form: `up_to` is the inclusive series upper bound
/// (`None` is the unbounded top tier), and `unit_amount`/`flat_amount` are mapped
/// independently from cents (a tier may carry both, additively).
pub(crate) fn to_json_tier(tier: &PriceTier) -> JsonPriceTier {
    let cents = |amount: Option<i64>| amount.and_then(|c| u64::try_from(c).ok()).map(BigInt::from);
    JsonPriceTier {
        // An `up_to` outside `u32` maps to `None`, which downstream means the unbounded
        // top tier. Stripe bounds are far below `u32::MAX`, so this is unreachable in
        // practice; a real ladder is never silently reshaped.
        up_to: tier.up_to.and_then(|n| u32::try_from(n).ok()),
        unit_amount: cents(tier.unit_amount),
        flat_amount: cents(tier.flat_amount),
    }
}

#[derive(Clone)]
pub struct Product {
    pub id: ProductId,
    /// The product's prices by price name.
    pub metered: HashMap<String, StripePrice>,
}

impl Product {
    async fn new(client: &StripeClient, product: JsonProduct) -> Result<Self, BillingError> {
        let JsonProduct { id, metered } = product;

        Ok(Self {
            id: retrieve_product(client, &id).await?,
            metered: Self::pricing(client, metered).await?,
        })
    }

    pub fn price(&self, price_name: &str) -> Result<&StripePrice, BillingError> {
        self.metered
            .get(price_name)
            .ok_or_else(|| BillingError::PriceNotFound(price_name.to_owned()))
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

/// The id of the Stripe product `id`, once Stripe has it.
async fn retrieve_product(client: &StripeClient, id: &str) -> Result<ProductId, BillingError> {
    let product_id: ProductId = id.into();
    Ok(RetrieveProduct::new(product_id).send(client).await?.id)
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
        // The unbounded top tier has no upper bound.
        let json = to_json_tier(&price_tier(None, None, Some(20_000)));
        assert_eq!(json.up_to, None);
        assert_eq!(json.flat_amount.map(u64::from), Some(20_000));
    }
}
