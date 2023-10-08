use bencher_plus::BENCHER_DEV;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{audience::Audience, billing_cycle::BillingCycle, licensor::now, LicenseError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Claims {
    pub aud: String, // Audience
    pub exp: u64,    // Expiration time (as UTC timestamp)
    pub iat: u64,    // Issued at (as UTC timestamp)
    pub iss: String, // Issuer
    pub sub: Uuid,   // Subject (whom token refers to)
    pub ent: u64,    // Entitlements (max number of metrics allowed)
}

impl Claims {
    pub fn new(
        audience: Audience,
        billing_cycle: BillingCycle,
        organization: Uuid,
        entitlements: u64,
    ) -> Result<Self, LicenseError> {
        let now = now()?;
        Ok(Self {
            aud: audience.into(),
            exp: now.checked_add(u64::from(billing_cycle)).unwrap_or(now),
            iat: now,
            iss: BENCHER_DEV.into(),
            sub: organization,
            ent: entitlements,
        })
    }

    pub fn organization(&self) -> Uuid {
        self.sub
    }
}
