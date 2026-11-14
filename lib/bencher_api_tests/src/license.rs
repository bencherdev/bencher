use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use bencher_json::{
    BENCHER_URL_STR, Clock, DateTime, Entitlements, Jwt, OrganizationUuid, Secret,
    organization::usage::JsonUsage,
};
use bencher_license::Licensor;
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

use crate::{TestOrg, TestServer, TestUser};

// Debug builds trust this key's public half for Self-Hosted licenses.
const TEST_LICENSE_KEY: &str = include_str!("../../../plus/bencher_license/src/test/private.pem");
// No build trusts this key's public half.
const FOREIGN_LICENSE_KEY: &str =
    include_str!("../../../plus/bencher_license/src/test/foreign.pem");

/// The Benchmark Series a test license covers, unless a test names its own.
pub const TEST_LICENSE_SERIES: u32 = 1_000_000;
/// Seconds in a day.
pub const DAY: i64 = 24 * 60 * 60;
/// The paid term of a test license, unless a test names its own.
pub const TEST_LICENSE_TERM: i64 = 365 * DAY;

/// `seconds` after `date_time`, which may be negative for a time before it.
#[expect(clippy::expect_used, reason = "test helper moving a time")]
pub fn after(date_time: DateTime, seconds: i64) -> DateTime {
    DateTime::try_from(date_time.timestamp() + seconds).expect("Invalid test time")
}

/// A license for `series` Benchmark Series on `organization`, issued at `issued_at` for a
/// paid term that ends at `term_end`, signed by the key debug builds trust.
pub fn sign_license(
    organization: OrganizationUuid,
    series: u32,
    issued_at: DateTime,
    term_end: DateTime,
) -> Jwt {
    sign_with(TEST_LICENSE_KEY, organization, series, issued_at, term_end)
}

/// [`sign_license`], signed by a key that no build trusts.
pub fn sign_foreign_license(
    organization: OrganizationUuid,
    series: u32,
    issued_at: DateTime,
    term_end: DateTime,
) -> Jwt {
    sign_with(
        FOREIGN_LICENSE_KEY,
        organization,
        series,
        issued_at,
        term_end,
    )
}

#[expect(clippy::expect_used, reason = "test helper signing a license")]
fn sign_with(
    key: &str,
    organization: OrganizationUuid,
    series: u32,
    issued_at: DateTime,
    term_end: DateTime,
) -> Jwt {
    let key: Secret = key.parse().expect("Invalid test license key");
    Licensor::bencher_cloud(&key)
        .expect("Failed to load the test license key")
        .new_license(
            organization,
            Entitlements::try_from(series).expect("Invalid test license series"),
            issued_at,
            term_end,
        )
        .expect("Failed to sign the test license")
}

/// A license in the earlier per-metric format, with a plan level and metric entitlements in
/// place of Benchmark Series and a term, signed by the key debug builds trust.
#[expect(clippy::expect_used, reason = "test helper signing a license")]
pub fn sign_earlier_license(
    organization: OrganizationUuid,
    issued_at: DateTime,
    expiration: DateTime,
) -> Jwt {
    let claims = serde_json::json!({
        "aud": "bencher",
        "exp": expiration.timestamp(),
        "iat": issued_at.timestamp(),
        "iss": BENCHER_URL_STR,
        "sub": organization,
        "lvl": "enterprise",
        "ent": TEST_LICENSE_SERIES,
    });
    let key = jsonwebtoken::EncodingKey::from_ec_pem(TEST_LICENSE_KEY.as_bytes())
        .expect("Invalid test license key");
    jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256),
        &claims,
        &key,
    )
    .expect("Failed to sign the earlier license")
    .parse()
    .expect("Invalid earlier license")
}

impl TestServer {
    /// The time on the server's clock.
    pub fn now(&self) -> DateTime {
        self.context().clock.now()
    }

    /// A license for [`TEST_LICENSE_SERIES`] Benchmark Series on `org`, issued now by the
    /// server's clock for a term of [`TEST_LICENSE_TERM`].
    pub fn sign_license(&self, org: &TestOrg) -> Jwt {
        let now = self.now();
        sign_license(
            org.uuid,
            TEST_LICENSE_SERIES,
            now,
            after(now, TEST_LICENSE_TERM),
        )
    }

    /// Set a valid Self-Hosted license on the organization through the API, so its plan
    /// resolves to `Licensed` the way a licensed Self-Hosted organization's does.
    pub async fn license_org(&self, user: &TestUser, org: &TestOrg) -> Jwt {
        let license = self.sign_license(org);
        self.license_org_with(user, org, &license).await;
        license
    }

    /// Set `license` on the organization through the API, which must accept it.
    pub async fn license_org_with(&self, user: &TestUser, org: &TestOrg, license: &Jwt) {
        let (status, body) = self.patch_license(user, org, license).await;
        assert!(status.is_success(), "License org failed: {body}");
    }

    /// Ask the API to set `license` on the organization.
    #[expect(
        clippy::expect_used,
        reason = "test helper for licensing an organization"
    )]
    pub async fn patch_license(
        &self,
        user: &TestUser,
        org: &TestOrg,
        license: &Jwt,
    ) -> (StatusCode, String) {
        let org_slug: &str = org.slug.as_ref();
        let resp = self
            .client
            .patch(self.api_url(&format!("/v0/organizations/{org_slug}")))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .json(&serde_json::json!({ "license": license }))
            .send()
            .await
            .expect("Failed to send license request");
        let status = resp.status();
        (
            status,
            resp.text()
                .await
                .expect("Failed to read the license response"),
        )
    }

    /// The organization's usage, as a manager of it reads it.
    #[expect(clippy::expect_used, reason = "test helper reading usage")]
    pub async fn usage(&self, user: &TestUser, org: &TestOrg) -> JsonUsage {
        let org_slug: &str = org.slug.as_ref();
        let resp = self
            .client
            .get(self.api_url(&format!("/v0/organizations/{org_slug}/usage")))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .send()
            .await
            .expect("Failed to send usage request");
        assert_eq!(resp.status(), StatusCode::OK, "Failed to read usage");
        resp.json().await.expect("Failed to parse usage")
    }

    /// Write `license` straight into the database, as one the API would refuse to set is.
    #[expect(clippy::expect_used, reason = "test helper storing a license")]
    pub fn store_license(&self, org: &TestOrg, license: &Jwt) {
        diesel::update(schema::organization::table.filter(schema::organization::uuid.eq(org.uuid)))
            .set(schema::organization::license.eq(license))
            .execute(&mut self.db_conn())
            .expect("Failed to store the license");
    }
}

/// A clock a test sets and moves by hand.
#[derive(Clone)]
pub struct TestClock(Arc<AtomicI64>);

impl TestClock {
    pub fn new(now: DateTime) -> Self {
        Self(Arc::new(AtomicI64::new(now.timestamp())))
    }

    /// The clock for a test server, which reads this one.
    #[expect(clippy::expect_used, reason = "test clock with a valid time")]
    pub fn clock(&self) -> Clock {
        let now = Arc::clone(&self.0);
        Clock::Custom(Arc::new(move || {
            DateTime::try_from(now.load(Ordering::SeqCst)).expect("Invalid test time")
        }))
    }

    #[expect(clippy::expect_used, reason = "test clock with a valid time")]
    pub fn now(&self) -> DateTime {
        DateTime::try_from(self.0.load(Ordering::SeqCst)).expect("Invalid test time")
    }

    /// Move the clock `seconds` ahead.
    pub fn advance(&self, seconds: i64) {
        self.0.fetch_add(seconds, Ordering::SeqCst);
    }
}
