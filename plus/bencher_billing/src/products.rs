use std::collections::{HashMap, HashSet};

use bencher_json::{
    organization::plan::DEFAULT_PRICE_NAME,
    system::config::{JsonProduct, JsonProducts},
};
use stripe::Client as StripeClient;
use stripe_product::{
    Price as StripePrice, PriceId, Product as StripeProduct, ProductId, price::RetrievePrice,
    product::RetrieveProduct,
};

use crate::BillingError;

pub struct Products {
    pub team: Product,
    pub enterprise: Product,
    pub bare_metal: Product,
}

impl Products {
    pub async fn new(client: &StripeClient, products: JsonProducts) -> Result<Self, BillingError> {
        let JsonProducts {
            team,
            enterprise,
            bare_metal,
        } = products;

        Ok(Self {
            team: Product::new(client, team).await?,
            enterprise: Product::new(client, enterprise).await?,
            bare_metal: Product::new(client, bare_metal).await?,
        })
    }

    // Returns the price IDs for Team and Enterprise plans only (excluding
    // bare_metal), used by `get_plan` to filter subscription items to the
    // main plan item.
    //
    // During the metered billing migration, a subscription may temporarily have
    // multiple subscription items (old metered + new metered). The config holds
    // both price IDs under different keys: the currently-active price under
    // "default" and the upcoming price under "metrics".
    //
    // This method returns only the price IDs for the given `preferred` key,
    // falling back to "default" if the preferred key is not found.
    // Once the migration cutover is complete and the old subscription items are
    // removed, this filtering becomes a no-op (one item in, one item out).
    pub fn plan_price_ids(&self, preferred: &str) -> HashSet<&PriceId> {
        self.team
            .preferred_price_ids(preferred)
            .into_iter()
            .chain(self.enterprise.preferred_price_ids(preferred))
            .collect()
    }
}

pub struct Product {
    #[expect(dead_code, clippy::struct_field_names)]
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

    // Returns the price IDs for the given `preferred` key, falling back to
    // "default" if the preferred key is not found.
    // See `Products::plan_price_ids` for migration context.
    fn preferred_price_ids(&self, preferred: &str) -> Vec<&PriceId> {
        let metered_id = self
            .metered
            .get(preferred)
            .or_else(|| self.metered.get(DEFAULT_PRICE_NAME))
            .map(|p| &p.id);
        let licensed_id = self
            .licensed
            .get(preferred)
            .or_else(|| self.licensed.get(DEFAULT_PRICE_NAME))
            .map(|p| &p.id);
        metered_id.into_iter().chain(licensed_id).collect()
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
