use bencher_json::{DateTime, Entitlements, OrganizationUuid};
use bencher_plus::BENCHER_DEV;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{audience::Audience, billing_cycle::BillingCycle, LicenseError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Claims {
    pub aud: String,           // Audience
    pub exp: i64,              // Expiration time (as UTC timestamp)
    pub iat: i64,              // Issued at (as UTC timestamp)
    pub iss: String,           // Issuer
    pub sub: OrganizationUuid, // Subject (whom token refers to)
    pub ent: Entitlements,     // Entitlements (max number of metrics allowed)
}

impl Claims {
    pub fn new(
        audience: Audience,
        billing_cycle: BillingCycle,
        organization: OrganizationUuid,
        entitlements: Entitlements,
    ) -> Result<Self, LicenseError> {
        let now = Utc::now().timestamp();
        Ok(Self {
            aud: audience.into(),
            exp: now.checked_add(i64::from(billing_cycle)).unwrap_or(now),
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
        let date_time = DateTime::try_from(self.iat);
        debug_assert!(date_time.is_ok(), "Issued at time is invalid");
        date_time.unwrap_or_default()
    }

    pub fn expiration(&self) -> DateTime {
        let date_time = DateTime::try_from(self.exp);
        debug_assert!(date_time.is_ok(), "Expiration time is invalid");
        date_time.unwrap_or_default()
    }

    pub fn entitlements(&self) -> Entitlements {
        self.ent
    }
}
