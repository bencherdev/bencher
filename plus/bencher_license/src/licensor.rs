use std::str::FromStr;

use bencher_json::organization::plan::JsonLicense;
use bencher_json::{Entitlements, Jwt, OrganizationUuid, PlanLevel, Secret};
use bencher_plus::BENCHER_DEV;
use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, Header, TokenData, Validation};
use jsonwebtoken::{DecodingKey, EncodingKey};
use once_cell::sync::Lazy;

use crate::{audience::Audience, billing_cycle::BillingCycle, claims::Claims, LicenseError};

#[cfg(debug_assertions)]
pub const PUBLIC_PEM: &str = include_str!("./test/public.pem");
#[cfg(not(debug_assertions))]
pub const PUBLIC_PEM: &str = include_str!("../public.pem");

static ALGORITHM: Lazy<Algorithm> = Lazy::new(|| Algorithm::ES256);
static HEADER: Lazy<Header> = Lazy::new(|| Header::new(*ALGORITHM));

pub enum Licensor {
    SelfHosted {
        decoding: DecodingKey,
    },
    BencherCloud {
        encoding: EncodingKey,
        decoding: DecodingKey,
    },
}

impl Licensor {
    pub fn self_hosted() -> Result<Self, LicenseError> {
        let decoding = decoding_key()?;
        Ok(Self::SelfHosted { decoding })
    }

    pub fn bencher_cloud(private_pem: &Secret) -> Result<Self, LicenseError> {
        let encoding = encoding_key(private_pem.as_ref())?;
        let decoding = decoding_key()?;
        Ok(Self::BencherCloud { encoding, decoding })
    }

    fn encoding(&self) -> Result<&EncodingKey, LicenseError> {
        match self {
            Self::SelfHosted { .. } => Err(LicenseError::SelfHosted),
            Self::BencherCloud { encoding, .. } => Ok(encoding),
        }
    }

    fn decoding(&self) -> &DecodingKey {
        match self {
            Self::SelfHosted { decoding } | Self::BencherCloud { decoding, .. } => decoding,
        }
    }

    fn new_license(
        &self,
        audience: Audience,
        billing_cycle: BillingCycle,
        organization: OrganizationUuid,
        plan_level: PlanLevel,
        entitlements: Entitlements,
    ) -> Result<Jwt, LicenseError> {
        let claims = Claims::new(
            audience,
            billing_cycle,
            organization,
            plan_level,
            entitlements,
        )?;
        let encoding = self.encoding()?;
        Ok(Jwt::from_str(&encode(&HEADER, &claims, encoding)?)?)
    }

    pub fn new_monthly_license(
        &self,
        organization: OrganizationUuid,
        plan_level: PlanLevel,
        entitlements: Entitlements,
    ) -> Result<Jwt, LicenseError> {
        self.new_license(
            Audience::Bencher,
            BillingCycle::Monthly,
            organization,
            plan_level,
            entitlements,
        )
    }

    pub fn new_annual_license(
        &self,
        organization: OrganizationUuid,
        plan_level: PlanLevel,
        entitlements: Entitlements,
    ) -> Result<Jwt, LicenseError> {
        self.new_license(
            Audience::Bencher,
            BillingCycle::Annual,
            organization,
            plan_level,
            entitlements,
        )
    }

    pub fn validate(&self, license: &Jwt) -> Result<TokenData<Claims>, LicenseError> {
        let mut validation = Validation::new(*ALGORITHM);
        validation.set_audience(&[Audience::Bencher]);
        validation.set_issuer(&[BENCHER_DEV]);
        validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);

        let token_data: TokenData<Claims> = decode(license.as_ref(), self.decoding(), &validation)?;
        check_expiration(token_data.claims.exp)?;

        Ok(token_data)
    }

    pub fn validate_organization(
        &self,
        license: &Jwt,
        organization: OrganizationUuid,
    ) -> Result<TokenData<Claims>, LicenseError> {
        let token_data = self.validate(license)?;
        if token_data.claims.sub == organization {
            Ok(token_data)
        } else {
            Err(LicenseError::Subject {
                provided: organization,
                license: token_data.claims.sub,
            })
        }
    }

    pub fn validate_usage(
        &self,
        claims: &Claims,
        usage: u32,
    ) -> Result<Entitlements, LicenseError> {
        let entitlements = claims.ent;
        if usage > entitlements.into() {
            Err(LicenseError::Entitlements {
                usage,
                entitlements,
            })
        } else {
            Ok(entitlements)
        }
    }

    pub fn into_json(
        &self,
        license: Jwt,
        plan_organization_uuid: Option<OrganizationUuid>,
    ) -> Result<JsonLicense, LicenseError> {
        // Do not validate the organization here
        // In the case of a Self-Hosted license, the organizations will not match
        let token_data = self.validate(&license)?;
        // If there is a plan, check to see if the organization matches
        // Otherwise, this is a Self-Hosted license
        let self_hosted = if let Some(organization_uuid) = plan_organization_uuid {
            token_data.claims.organization() != organization_uuid
        } else {
            true
        };
        Ok(JsonLicense {
            key: license,
            organization: token_data.claims.organization(),
            entitlements: token_data.claims.entitlements(),
            issued_at: token_data.claims.issued_at(),
            expiration: token_data.claims.expiration(),
            self_hosted,
        })
    }
}

fn encoding_key(key: &str) -> Result<EncodingKey, LicenseError> {
    EncodingKey::from_ec_pem(key.as_bytes()).map_err(LicenseError::PrivatePem)
}

fn decoding_key() -> Result<DecodingKey, LicenseError> {
    DecodingKey::from_ec_pem(PUBLIC_PEM.as_bytes()).map_err(LicenseError::PublicPem)
}

fn check_expiration(time: i64) -> Result<(), LicenseError> {
    let now = Utc::now().timestamp();
    if time < now {
        Err(
            jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::ExpiredSignature)
                .into(),
        )
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod test {
    use bencher_json::{OrganizationUuid, PlanLevel, Secret};
    use bencher_plus::BENCHER_DEV;
    use once_cell::sync::Lazy;
    use pretty_assertions::assert_eq;

    use crate::{audience::Audience, licensor::BillingCycle, Licensor};

    pub const PRIVATE_PEM: &str = include_str!("./test/private.pem");
    static PRIVATE_PEM_SECRET: Lazy<Secret> = Lazy::new(|| PRIVATE_PEM.parse().unwrap());

    #[test]
    fn test_self_hosted() {
        let licensor = Licensor::self_hosted().unwrap();
        let organization = OrganizationUuid::new();
        let entitlements = 1_000.try_into().unwrap();

        for plan_level in [PlanLevel::Free, PlanLevel::Team, PlanLevel::Enterprise] {
            assert!(licensor
                .new_monthly_license(organization, plan_level, entitlements)
                .is_err());
            assert!(licensor
                .new_annual_license(organization, plan_level, entitlements)
                .is_err());
        }
    }

    #[test]
    fn test_bencher_cloud_monthly() {
        let licensor = Licensor::bencher_cloud(&PRIVATE_PEM_SECRET).unwrap();
        let organization = OrganizationUuid::new();
        let entitlements = 1_000.try_into().unwrap();

        for plan_level in [PlanLevel::Free, PlanLevel::Team, PlanLevel::Enterprise] {
            let license = licensor
                .new_monthly_license(organization, plan_level, entitlements)
                .unwrap();

            let token_data = licensor.validate(&license).unwrap();

            assert_eq!(token_data.claims.aud, Audience::Bencher);
            assert_eq!(token_data.claims.iss, BENCHER_DEV);
            assert_eq!(
                token_data.claims.iat,
                token_data.claims.exp - i64::from(BillingCycle::Monthly)
            );
            assert_eq!(token_data.claims.sub, organization);
            assert_eq!(token_data.claims.ent, entitlements);
        }
    }

    #[test]
    fn test_bencher_cloud_annual() {
        let licensor = Licensor::bencher_cloud(&PRIVATE_PEM_SECRET).unwrap();
        let organization = OrganizationUuid::new();
        let entitlements = 1_000.try_into().unwrap();

        for plan_level in [PlanLevel::Free, PlanLevel::Team, PlanLevel::Enterprise] {
            let license = licensor
                .new_annual_license(organization, plan_level, entitlements)
                .unwrap();

            let token_data = licensor.validate(&license).unwrap();

            assert_eq!(token_data.claims.aud, Audience::Bencher);
            assert_eq!(token_data.claims.iss, BENCHER_DEV);
            assert_eq!(
                token_data.claims.iat,
                token_data.claims.exp - i64::from(BillingCycle::Annual)
            );
            assert_eq!(token_data.claims.sub, organization);
            assert_eq!(token_data.claims.ent, entitlements);
        }
    }
}
