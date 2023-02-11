use bencher_valid::Secret;
use stripe::Client;

use crate::BillingError;

pub struct Biller {
    client: Client,
}

impl Biller {
    pub fn new(secret_key: Secret) -> Self {
        let client = Client::new(secret_key);
        Self { client }
    }
}
