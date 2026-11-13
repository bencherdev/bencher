use std::str::FromStr as _;
use std::sync::LazyLock;

use bencher_json::organization::plan::JsonLicense;
use bencher_json::{BENCHER_URL_STR, DateTime, Entitlements, Jwt, OrganizationUuid, Secret};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};

use crate::{LicenseError, audience::Audience, claims::Claims};

pub const TEST_PUBLIC_PEM: &str = include_str!("./test/public.pem");
pub const LIVE_PUBLIC_PEM: &str = include_str!("../public.pem");

#[cfg(debug_assertions)]
pub const PUBLIC_PEM: &str = TEST_PUBLIC_PEM;
#[cfg(not(debug_assertions))]
pub const PUBLIC_PEM: &str = LIVE_PUBLIC_PEM;

static ALGORITHM: LazyLock<Algorithm> = LazyLock::new(|| Algorithm::ES256);
static HEADER: LazyLock<Header> = LazyLock::new(|| Header::new(*ALGORITHM));

#[derive(Clone)]
pub enum Licensor {
    SelfHosted {
        decoding: DecodingKey,
    },
    BencherCloud {
        encoding: EncodingKey,
        decoding: DecodingKey,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum PublicKey {
    Test,
    Live,
}

impl Licensor {
    pub fn self_hosted() -> Result<Self, LicenseError> {
        let decoding = decoding_key(None)?;
        Ok(Self::SelfHosted { decoding })
    }

    pub fn bencher_cloud(private_pem: &Secret) -> Result<Self, LicenseError> {
        Self::bencher_cloud_with_public_key(private_pem, None)
    }

    pub fn bencher_cloud_with_public_key(
        private_pem: &Secret,
        public_key: Option<PublicKey>,
    ) -> Result<Self, LicenseError> {
        let encoding = encoding_key(private_pem.as_ref())?;
        let decoding = decoding_key(public_key)?;
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

    /// Sign a license for `series` Benchmark Series on the Self-Hosted `organization`,
    /// issued at `issued_at`, for a paid term that ends at `term_end`.
    /// The license expires at the end of the term plus the grace period.
    pub fn new_license(
        &self,
        organization: OrganizationUuid,
        series: Entitlements,
        issued_at: DateTime,
        term_end: DateTime,
    ) -> Result<Jwt, LicenseError> {
        self.new_license_with_issuer(organization, series, issued_at, term_end, BENCHER_URL_STR)
    }

    pub fn new_license_with_issuer(
        &self,
        organization: OrganizationUuid,
        series: Entitlements,
        issued_at: DateTime,
        term_end: DateTime,
        issuer: &str,
    ) -> Result<Jwt, LicenseError> {
        let claims = Claims::new(organization, series, issued_at, term_end, issuer)?;
        let encoding = self.encoding()?;
        Ok(Jwt::from_str(&encode(&HEADER, &claims, encoding)?)?)
    }

    /// The claims of `license` when it is valid at `now`.
    pub fn validate(&self, license: &Jwt, now: DateTime) -> Result<Claims, LicenseError> {
        self.validate_with_issuer(license, BENCHER_URL_STR, now)
    }

    pub fn validate_with_issuer(
        &self,
        license: &Jwt,
        issuer: &str,
        now: DateTime,
    ) -> Result<Claims, LicenseError> {
        let mut validation = Validation::new(*ALGORITHM);
        validation.set_audience(&[Audience::Bencher]);
        validation.set_issuer(&[issuer]);
        validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);
        // The expiration is checked against `now`, not the system clock.
        validation.validate_exp = false;

        let claims = decode::<Claims>(license.as_ref(), self.decoding(), &validation)?.claims;
        claims.check_time(now)?;
        Ok(claims)
    }

    /// The claims of `license` when it is valid at `now` for `organization`.
    pub fn validate_organization(
        &self,
        license: &Jwt,
        organization: OrganizationUuid,
        now: DateTime,
    ) -> Result<Claims, LicenseError> {
        let claims = self.validate(license, now)?;
        if claims.organization() == organization {
            Ok(claims)
        } else {
            Err(LicenseError::Subject {
                provided: organization,
                license: claims.organization(),
            })
        }
    }

    pub fn into_json(&self, license: Jwt, now: DateTime) -> Result<JsonLicense, LicenseError> {
        // Do not validate the organization here
        // In the case of a Self-Hosted license, the organizations will not match
        Ok(self.validate(&license, now)?.into_json(license))
    }
}

fn encoding_key(key: &str) -> Result<EncodingKey, LicenseError> {
    EncodingKey::from_ec_pem(key.as_bytes()).map_err(LicenseError::PrivatePem)
}

fn decoding_key(public_key: Option<PublicKey>) -> Result<DecodingKey, LicenseError> {
    let key = match public_key {
        Some(PublicKey::Test) => TEST_PUBLIC_PEM,
        Some(PublicKey::Live) => LIVE_PUBLIC_PEM,
        None => PUBLIC_PEM,
    };
    DecodingKey::from_ec_pem(key.as_bytes()).map_err(LicenseError::PublicPem)
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use bencher_json::{BENCHER_URL_STR, DateTime, Entitlements, Jwt, OrganizationUuid, Secret};
    use chrono::Duration;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use pretty_assertions::assert_eq;
    use serde::Serialize;

    use crate::{GRACE_PERIOD, ISSUED_AT_LEEWAY, LicenseError, Licensor};

    const PRIVATE_PEM: &str = include_str!("./test/private.pem");
    const FOREIGN_PEM: &str = include_str!("./test/foreign.pem");
    static PRIVATE_PEM_SECRET: LazyLock<Secret> = LazyLock::new(|| PRIVATE_PEM.parse().unwrap());
    static FOREIGN_PEM_SECRET: LazyLock<Secret> = LazyLock::new(|| FOREIGN_PEM.parse().unwrap());

    fn licensor() -> Licensor {
        Licensor::bencher_cloud(&PRIVATE_PEM_SECRET).unwrap()
    }

    fn series() -> Entitlements {
        Entitlements::try_from(1_000).unwrap()
    }

    fn issued_at() -> DateTime {
        DateTime::TEST
    }

    fn term_end() -> DateTime {
        DateTime::TEST + Duration::days(365)
    }

    fn sign(licensor: &Licensor, organization: OrganizationUuid) -> Jwt {
        licensor
            .new_license(organization, series(), issued_at(), term_end())
            .unwrap()
    }

    // Fails if a Self-Hosted server, which holds no private key, can sign a license.
    #[test]
    fn self_hosted_cannot_sign() {
        let licensor = Licensor::self_hosted().unwrap();
        let error = licensor
            .new_license(OrganizationUuid::new(), series(), issued_at(), term_end())
            .unwrap_err();
        assert!(matches!(error, LicenseError::SelfHosted), "{error}");
    }

    // Fails if any claim is lost or changed between signing and verifying, or if the
    // expiration is anything but the end of the term plus the grace period.
    #[test]
    fn signs_and_verifies_every_claim() {
        let licensor = licensor();
        let organization = OrganizationUuid::new();
        let license = sign(&licensor, organization);

        let claims = licensor
            .validate(&license, issued_at() + Duration::days(1))
            .unwrap();

        assert_eq!(claims.aud, crate::Audience::Bencher);
        assert_eq!(claims.iss, BENCHER_URL_STR);
        assert_eq!(claims.sub, organization);
        assert_eq!(claims.ser, series());
        assert_eq!(claims.iat, issued_at().timestamp());
        assert_eq!(claims.trm, term_end().timestamp());
        assert_eq!(claims.exp, (term_end() + GRACE_PERIOD).timestamp());
        assert_eq!(GRACE_PERIOD, Duration::days(30));

        assert_eq!(claims.organization(), organization);
        assert_eq!(claims.series(), series());
        assert_eq!(claims.issued_at(), issued_at());
        assert_eq!(claims.term_end(), term_end());
        assert_eq!(claims.expiration(), term_end() + Duration::days(30));
    }

    // Fails if a license that ends its term when or before it starts is signed.
    #[test]
    fn term_ends_after_it_is_issued() {
        let licensor = licensor();
        for term_end in [issued_at(), issued_at() - Duration::seconds(1)] {
            let error = licensor
                .new_license(OrganizationUuid::new(), series(), issued_at(), term_end)
                .unwrap_err();
            assert!(matches!(error, LicenseError::Term { .. }), "{error}");
        }
        licensor
            .new_license(
                OrganizationUuid::new(),
                series(),
                issued_at(),
                issued_at() + Duration::seconds(1),
            )
            .unwrap();
    }

    /// The claims of the earlier per-metric license format.
    #[derive(Serialize)]
    struct EarlierClaims {
        aud: &'static str,
        exp: i64,
        iat: i64,
        iss: &'static str,
        sub: OrganizationUuid,
        lvl: &'static str,
        ent: u32,
    }

    // Fails if a license in the earlier format, with a plan level and metric entitlements
    // and no term or Benchmark Series, still verifies.
    #[test]
    fn earlier_format_does_not_verify() {
        let licensor = licensor();
        let organization = OrganizationUuid::new();
        let now = issued_at() + Duration::days(1);
        let earlier = EarlierClaims {
            aud: "bencher",
            exp: term_end().timestamp(),
            iat: issued_at().timestamp(),
            iss: BENCHER_URL_STR,
            sub: organization,
            lvl: "enterprise",
            ent: 1_000,
        };
        let key = EncodingKey::from_ec_pem(PRIVATE_PEM.as_bytes()).unwrap();
        let license: Jwt = encode(&Header::new(jsonwebtoken::Algorithm::ES256), &earlier, &key)
            .unwrap()
            .parse()
            .unwrap();

        licensor.validate(&license, now).unwrap_err();
        licensor
            .validate_organization(&license, organization, now)
            .unwrap_err();
        // The same key signs a license in the current format that verifies.
        licensor
            .validate(&sign(&licensor, organization), now)
            .unwrap();
    }

    // Fails if a license signed by a key other than Bencher's verifies.
    #[test]
    fn another_key_does_not_verify() {
        let licensor = licensor();
        let foreign = Licensor::bencher_cloud(&FOREIGN_PEM_SECRET).unwrap();
        let organization = OrganizationUuid::new();
        let now = issued_at() + Duration::days(1);

        licensor
            .validate(&sign(&foreign, organization), now)
            .unwrap_err();
        licensor
            .validate(&sign(&licensor, organization), now)
            .unwrap();
    }

    // Fails if a license verifies for an organization other than the one it names.
    #[test]
    fn another_organization_does_not_verify() {
        let licensor = licensor();
        let organization = OrganizationUuid::new();
        let license = sign(&licensor, organization);
        let now = issued_at() + Duration::days(1);

        let error = licensor
            .validate_organization(&license, OrganizationUuid::new(), now)
            .unwrap_err();
        assert!(matches!(error, LicenseError::Subject { .. }), "{error}");
        let claims = licensor
            .validate_organization(&license, organization, now)
            .unwrap();
        assert_eq!(claims.organization(), organization);
    }

    // Fails if a license verifies at or after its expiration, stops verifying before it,
    // or reads as in its grace outside of the time from the end of its term to its expiration.
    #[test]
    fn grace_runs_from_the_term_end_to_the_expiration() {
        let licensor = licensor();
        let license = sign(&licensor, OrganizationUuid::new());
        let expiration = term_end() + GRACE_PERIOD;

        let in_term = licensor
            .validate(&license, term_end() - Duration::seconds(1))
            .unwrap();
        assert!(!in_term.in_grace(term_end() - Duration::seconds(1)));

        for now in [term_end(), expiration - Duration::seconds(1)] {
            let claims = licensor.validate(&license, now).unwrap();
            assert!(claims.in_grace(now), "{now}");
        }

        for now in [expiration, expiration + Duration::days(1)] {
            let error = licensor.validate(&license, now).unwrap_err();
            assert!(matches!(error, LicenseError::Expired { .. }), "{error}");
        }
    }

    // Fails if a license verifies more than 5 minutes before it was issued, or a server whose
    // clock runs up to 5 minutes behind Bencher Cloud's refuses a license signed moments ago.
    #[test]
    fn issued_at_allows_5_minutes_of_clock_skew() {
        let licensor = licensor();
        let license = sign(&licensor, OrganizationUuid::new());

        assert_eq!(ISSUED_AT_LEEWAY, Duration::minutes(5));
        licensor
            .validate(&license, issued_at() - Duration::minutes(1))
            .unwrap();
        licensor
            .validate(&license, issued_at() - Duration::minutes(5))
            .unwrap();
        let error = licensor
            .validate(
                &license,
                issued_at() - Duration::minutes(5) - Duration::seconds(1),
            )
            .unwrap_err();
        assert!(
            matches!(error, LicenseError::NotYetIssued { .. }),
            "{error}"
        );
    }

    // Fails if the license JSON drops or changes a field, or still carries a level or
    // metric entitlements.
    #[test]
    fn into_json_names_the_series_and_the_term() {
        let licensor = licensor();
        let organization = OrganizationUuid::new();
        let license = sign(&licensor, organization);

        let json = licensor
            .into_json(license.clone(), issued_at() + Duration::days(1))
            .unwrap();
        assert_eq!(
            serde_json::to_value(&json).unwrap(),
            serde_json::json!({
                "key": license,
                "organization": organization,
                "series": 1_000,
                "issued_at": issued_at(),
                "term_end": term_end(),
                "expiration": term_end() + Duration::days(30),
            })
        );
    }
}
