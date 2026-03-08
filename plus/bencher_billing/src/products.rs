use std::collections::{HashMap, HashSet};

use bencher_json::{
    organization::plan::DEFAULT_PRICE_NAME,
    system::config::{JsonProduct, JsonProducts},
};
use stripe::{Client as StripeClient, Price as StripePrice, PriceId, Product as StripeProduct};

use crate::BillingError;

pub struct Products {
    pub team: Product,
    pub enterprise: Product,
}

impl Products {
    pub async fn new(client: &StripeClient, products: JsonProducts) -> Result<Self, BillingError> {
        let JsonProducts { team, enterprise } = products;

        Ok(Self {
            team: Product::new(client, team).await?,
            enterprise: Product::new(client, enterprise).await?,
        })
    }

    // During the metered billing migration, a subscription may temporarily have
    // multiple subscription items (old metered + new metered). The config holds
    // both price IDs under different keys: the currently-active price under
    // "default" and the upcoming price under "metrics".
    //
    // This method returns only the "default" price IDs so we can filter
    // subscription items down to the one we should actually bill against.
    // Once the migration cutover is complete and the old subscription items are
    // removed, this filtering becomes a no-op (one item in, one item out).
    pub fn default_price_ids(&self) -> HashSet<&PriceId> {
        self.team
            .default_price_ids()
            .into_iter()
            .chain(self.enterprise.default_price_ids())
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

        let product = StripeProduct::retrieve(client, &id.parse()?, &[]).await?;
        let metered = Self::pricing(client, metered).await?;
        let licensed = Self::pricing(client, licensed).await?;

        Ok(Self {
            product,
            metered,
            licensed,
        })
    }

    // Returns only the price IDs associated with the "default" key for this
    // product level. See `Products::default_price_ids` for migration context.
    fn default_price_ids(&self) -> Vec<&PriceId> {
        self.metered
            .get(DEFAULT_PRICE_NAME)
            .into_iter()
            .chain(self.licensed.get(DEFAULT_PRICE_NAME))
            .map(|p| &p.id)
            .collect()
    }

    async fn pricing(
        client: &StripeClient,
        pricing: HashMap<String, String>,
    ) -> Result<HashMap<String, StripePrice>, BillingError> {
        let mut biller_pricing = HashMap::with_capacity(pricing.len());
        for (price_name, price_id) in pricing {
            let price = StripePrice::retrieve(client, &price_id.parse()?, &[]).await?;
            biller_pricing.insert(price_name, price);
        }
        Ok(biller_pricing)
    }
}
