use crate::BillingError;

pub enum Biller {
    SelfHosted,
    BencherCloud {},
}

impl Biller {
    pub fn self_hosted() -> Self {
        Self::SelfHosted
    }

    pub fn bencher_cloud() -> Result<Self, BillingError> {
        Ok(Self::BencherCloud {})
    }
}
