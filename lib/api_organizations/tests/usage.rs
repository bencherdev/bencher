#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for organization usage endpoint.

use bencher_api_tests::TestServer;
use http::StatusCode;

// GET /v0/organizations/{organization}/usage - view usage
#[tokio::test]
async fn usage_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "usageget@example.com").await;
    let org = server.create_org(&user, "Usage Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/usage", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    // Usage endpoint may return NOT_FOUND if no usage data exists yet
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::NOT_FOUND,
        "Unexpected status: {}",
        resp.status()
    );
}

// GET /v0/organizations/{organization}/usage - requires auth
#[tokio::test]
async fn usage_requires_auth() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "usageauth@example.com").await;
    let org = server.create_org(&user, "Usage Auth Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/usage", org_slug)))
        .send()
        .await
        .expect("Request failed");

    // Without auth, should get an error (either 400 Bad Request or 404 Not Found)
    assert!(
        resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::NOT_FOUND,
        "Expected 400 or 404, got: {}",
        resp.status()
    );
}

// GET /v0/organizations/{organization}/usage - not found org
#[tokio::test]
async fn usage_org_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "usagenotfound@example.com")
        .await;

    let resp = server
        .client
        .get(server.api_url("/v0/organizations/nonexistent-org/usage"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v0/organizations/{organization}/usage - a Self-Hosted license reports its Benchmark Series
// over the last 30 days against the license's, its peak, its term end, and its expiration, and no
// metric figures.
#[cfg(feature = "plus")]
#[tokio::test]
async fn usage_self_hosted_license_counts_benchmark_series() {
    use bencher_api_tests::{
        TestClock, TestServerOptions,
        helpers::post_series,
        license::{DAY, after, sign_license},
    };
    use bencher_json::{
        DateTime, Entitlements,
        organization::usage::{JsonLicensePeak, UsageKind},
    };

    let now = DateTime::try_from(DateTime::now().timestamp()).expect("Invalid time");
    let clock = TestClock::new(now);
    let server = TestServer::new_with_options(TestServerOptions {
        clock: Some(clock.clock()),
        ..TestServerOptions::default()
    })
    .await;
    let user = server.signup("Test User", "usagelicense@example.com").await;
    let org = server.create_org(&user, "Usage License Org").await;
    let term_end = after(now, 90 * DAY);
    let license = sign_license(org.uuid, 5, now, term_end);
    server.license_org_with(&user, &org, &license).await;
    let project = server
        .create_project(&user, &org, "Usage License Project")
        .await;
    let (status, body) = post_series(&server, &user, &project, 3).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    clock.advance(DAY);

    let usage = server.usage(&user, &org).await;
    assert_eq!(usage.kind, UsageKind::SelfHostedLicensed);
    let json_license = usage.license.expect("No license");
    assert_eq!(json_license.key, license);
    assert_eq!(json_license.organization, org.uuid);
    assert_eq!(
        json_license.series,
        Entitlements::try_from(5).expect("Invalid series")
    );
    assert_eq!(json_license.issued_at, now);
    assert_eq!(json_license.term_end, term_end);
    assert_eq!(json_license.expiration, after(term_end, 30 * DAY));
    assert_eq!(usage.active_series, Some(3));
    assert_eq!(
        usage.peak,
        Some(JsonLicensePeak {
            term_end,
            active_series: 3,
            seen: now,
        })
    );
    assert_eq!(usage.previous_peak, None);
    assert_eq!(usage.start_time, after(clock.now(), -30 * DAY));
    assert_eq!(usage.end_time, clock.now());
    assert_eq!(usage.metrics, None);
    assert!(usage.plan.is_none());
}

// GET /v0/organizations/{organization}/usage - with no license, or one that is not valid, a
// Self-Hosted organization is Free.
#[cfg(feature = "plus")]
#[tokio::test]
async fn usage_self_hosted_without_a_valid_license_is_free() {
    use bencher_api_tests::license::{DAY, TEST_LICENSE_SERIES, after, sign_license};
    use bencher_json::organization::usage::UsageKind;

    let server = TestServer::new().await;
    let user = server.signup("Test User", "usagefree@example.com").await;
    let org = server.create_org(&user, "Usage Free Org").await;

    let usage = server.usage(&user, &org).await;
    assert_eq!(usage.kind, UsageKind::SelfHostedFree);
    assert!(usage.license.is_none());
    assert_eq!(usage.peak, None);

    let now = server.now();
    server.store_license(
        &org,
        &sign_license(
            org.uuid,
            TEST_LICENSE_SERIES,
            after(now, -400 * DAY),
            after(now, -31 * DAY),
        ),
    );
    let usage = server.usage(&user, &org).await;
    assert_eq!(usage.kind, UsageKind::SelfHostedFree, "an expired license");
    assert!(usage.license.is_none());

    server.license_org(&user, &org).await;
    assert_eq!(
        server.usage(&user, &org).await.kind,
        UsageKind::SelfHostedLicensed
    );
}

// GET /v0/organizations/{organization}/usage - a Self-Hosted license counts the bare metal runner
// minutes of the last 30 days, as it does its Benchmark Series.
#[cfg(feature = "plus")]
#[tokio::test]
async fn usage_self_hosted_license_counts_runner_minutes_over_30_days() {
    use bencher_api_tests::{
        helpers::seed_runner_minutes,
        license::{DAY, after},
    };

    let server = TestServer::new().await;
    let user = server.signup("Test User", "usageminutes@example.com").await;
    let org = server.create_org(&user, "Usage Minutes Org").await;
    server.license_org(&user, &org).await;
    let inside = server
        .create_project(&user, &org, "Usage Minutes Inside")
        .await;
    let outside = server
        .create_project(&user, &org, "Usage Minutes Outside")
        .await;
    let now = server.now();
    // 2 minutes inside the 30 days, and 5 minutes 1 second outside them.
    seed_runner_minutes(&server, &inside, after(now, -DAY), 120);
    seed_runner_minutes(&server, &outside, after(now, -30 * DAY - 1), 300);

    assert_eq!(server.usage(&user, &org).await.runner_minutes, Some(2));
}
