use bencher_json::{DateTime, OrganizationUuid};
use bencher_plus::BENCHER_DEV;
use serde::{Deserialize, Serialize};

use crate::{audience::Audience, billing_cycle::BillingCycle, licensor::now, LicenseError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Claims {
    pub aud: String,           // Audience
    pub exp: u64,              // Expiration time (as UTC timestamp)
    pub iat: u64,              // Issued at (as UTC timestamp)
    pub iss: String,           // Issuer
    pub sub: OrganizationUuid, // Subject (whom token refers to)
    pub ent: u64,              // Entitlements (max number of metrics allowed)
}

impl Claims {
    pub fn new(
        audience: Audience,
        billing_cycle: BillingCycle,
        organization: OrganizationUuid,
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

    pub fn organization(&self) -> OrganizationUuid {
        self.sub
    }

    pub fn issued_at(&self) -> DateTime {
        let timestamp = i64::try_from(self.iat);
        debug_assert!(timestamp.is_ok(), "Issued at time is invalid");
        let date_time = DateTime::try_from(timestamp.unwrap_or_default());
        debug_assert!(date_time.is_ok(), "Issued at time is invalid");
        date_time.unwrap_or_default()
    }

    pub fn expiration(&self) -> DateTime {
        let timestamp = i64::try_from(self.exp);
        debug_assert!(timestamp.is_ok(), "Expiration time is invalid");
        let date_time = DateTime::try_from(timestamp.unwrap_or_default());
        debug_assert!(date_time.is_ok(), "Expiration time is invalid");
        date_time.unwrap_or_default()
    }
}
