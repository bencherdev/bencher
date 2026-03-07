use std::collections::HashMap;

use bencher_json::system::config::{JsonProduct, JsonProducts};
use stripe::Client as StripeClient;
use stripe_product::{
    Price as StripePrice, PriceId, Product as StripeProduct, ProductId, price::RetrievePrice,
    product::RetrieveProduct,
};

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

        let product_id: ProductId = id.parse().unwrap();
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
            let price_id: PriceId = price_id.parse().unwrap();
            let price = RetrievePrice::new(price_id).send(client).await?;
            biller_pricing.insert(price_name, price);
        }
        Ok(biller_pricing)
    }
}
