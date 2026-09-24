use bencher_json::{Entitlements, PlanLevel, Secret};
use bencher_license::Licensor;

use crate::{TestOrg, TestServer, TestUser};

// Debug builds trust this key's public half for self-hosted licenses.
const TEST_LICENSE_KEY: &str = include_str!("../../../plus/bencher_license/src/test/private.pem");
const TEST_LICENSE_ENTITLEMENTS: u32 = 1_000_000;

impl TestServer {
    /// Set a self-hosted license at `level` on the organization through the API, so its plan
    /// resolves to `Licensed` the way a licensed self-hosted organization's does.
    #[expect(
        clippy::expect_used,
        reason = "test helper for licensing an organization"
    )]
    pub async fn license_org(&self, user: &TestUser, org: &TestOrg, level: PlanLevel) {
        let key: Secret = TEST_LICENSE_KEY.parse().expect("Invalid test license key");
        let entitlements = Entitlements::try_from(TEST_LICENSE_ENTITLEMENTS)
            .expect("Invalid test license entitlements");
        let license = Licensor::bencher_cloud(&key)
            .expect("Failed to load the test license key")
            .new_annual_license(org.uuid, level, entitlements)
            .expect("Failed to sign the test license");

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
        assert!(
            resp.status().is_success(),
            "License org failed: {}",
            resp.text().await.unwrap_or_default()
        );
    }
}
