#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for reports to an organization with a Bencher Self-Hosted license.

use bencher_api_tests::{
    TestClock, TestOrg, TestProject, TestServer, TestServerOptions, TestUser,
    helpers::{post_series, set_visibility},
    license::{
        DAY, TEST_LICENSE_SERIES, after, sign_earlier_license, sign_foreign_license, sign_license,
    },
};
use bencher_json::{
    DateTime, Entitlements, JsonReport, OrganizationUuid,
    organization::{
        plan::{JsonLicenseGrace, JsonLicenseOver, JsonReportLicense},
        usage::{JsonLicensePeak, UsageKind},
    },
    project::Visibility,
};
use http::StatusCode;

/// A Self-Hosted test server on a clock that starts now, to the second, and that the test moves.
async fn server_on_a_clock() -> (TestServer, TestClock) {
    let now = DateTime::try_from(DateTime::now().timestamp()).expect("Invalid time");
    let clock = TestClock::new(now);
    let server = TestServer::new_with_options(TestServerOptions {
        clock: Some(clock.clock()),
        ..TestServerOptions::default()
    })
    .await;
    (server, clock)
}

struct Licensed {
    user: TestUser,
    org: TestOrg,
    private: TestProject,
    public: TestProject,
}

/// An organization with a private and a public project, holding `license` if one is given.
async fn licensed(server: &TestServer, label: &str) -> Licensed {
    let user = server
        .signup("Test User", &format!("license-{label}@example.com"))
        .await;
    let org = server
        .create_org(&user, &format!("License {label} Org"))
        .await;
    let private = server
        .create_project(&user, &org, &format!("License {label} Private"))
        .await;
    set_visibility(server, &private, Visibility::Private);
    let public = server
        .create_project(&user, &org, &format!("License {label} Public"))
        .await;
    Licensed {
        user,
        org,
        private,
        public,
    }
}

/// Post a report of `series` Benchmark Series, which must be accepted, and return it.
async fn accepted(
    server: &TestServer,
    user: &TestUser,
    project: &TestProject,
    series: usize,
) -> JsonReport {
    let (status, body) = post_series(server, user, project, series).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    serde_json::from_str(&body).expect("Failed to parse the report")
}

fn over(active_series: u32, series: u32) -> JsonReportLicense {
    JsonReportLicense {
        over: Some(JsonLicenseOver {
            active_series,
            series: Entitlements::try_from(series).expect("Invalid series"),
        }),
        grace: None,
    }
}

/// The peak of the term that ends at `term_end`.
fn peak(term_end: DateTime, active_series: u32, seen: DateTime) -> JsonLicensePeak {
    JsonLicensePeak {
        term_end,
        active_series,
        seen,
    }
}

// Fails if a report that takes the organization over its license is refused, carries no
// warning or the wrong numbers, or does not raise the peak; or if a later report with a lower
// count lowers the peak.
#[tokio::test]
async fn a_report_over_the_license_is_accepted_with_a_warning_and_raises_the_peak() {
    let (server, clock) = server_on_a_clock().await;
    let Licensed {
        user, org, private, ..
    } = licensed(&server, "over").await;
    let start = clock.now();
    let term_end = after(start, 365 * DAY);
    let license = sign_license(org.uuid, 2, start, term_end);
    server.license_org_with(&user, &org, &license).await;
    assert_eq!(server.usage(&user, &org).await.peak, None);

    // Within the license, a report carries no license warning.
    let report = accepted(&server, &user, &private, 2).await;
    assert_eq!(report.license, None);
    assert_eq!(
        server.usage(&user, &org).await.peak,
        Some(peak(term_end, 2, start))
    );

    // Over the license, the report is accepted and warns with the count and the license's.
    clock.advance(DAY);
    let over_at = clock.now();
    let report = accepted(&server, &user, &private, 3).await;
    assert_eq!(report.license, Some(over(3, 2)));
    let usage = server.usage(&user, &org).await;
    assert_eq!(usage.active_series, Some(3));
    assert_eq!(usage.peak, Some(peak(term_end, 3, over_at)));

    // Once the earlier Benchmark Series are more than 30 days old, a report counts fewer,
    // and the peak stands.
    clock.advance(31 * DAY);
    let report = accepted(&server, &user, &private, 1).await;
    assert_eq!(report.license, None);
    let usage = server.usage(&user, &org).await;
    assert_eq!(usage.active_series, Some(1));
    assert_eq!(usage.peak, Some(peak(term_end, 3, over_at)));
    assert_eq!(usage.previous_peak, None);
}

// Fails if a renewal hides the ended term's peak, before or after the first report under the
// new license, or if the new term does not start a peak of its own.
#[tokio::test]
async fn a_renewal_keeps_the_ended_terms_peak() {
    let (server, clock) = server_on_a_clock().await;
    let Licensed {
        user, org, private, ..
    } = licensed(&server, "renewal").await;
    let start = clock.now();
    let first_term = after(start, 30 * DAY);
    server
        .license_org_with(&user, &org, &sign_license(org.uuid, 2, start, first_term))
        .await;
    accepted(&server, &user, &private, 3).await;
    let ended = Some(peak(first_term, 3, start));

    // The renewed license, pasted in the grace of the first.
    clock.advance(31 * DAY);
    let renewed_at = clock.now();
    let second_term = after(first_term, 30 * DAY);
    server
        .license_org_with(
            &user,
            &org,
            &sign_license(org.uuid, 2, renewed_at, second_term),
        )
        .await;
    let usage = server.usage(&user, &org).await;
    assert_eq!(usage.peak, None);
    assert_eq!(usage.previous_peak, ended);

    accepted(&server, &user, &private, 1).await;
    let usage = server.usage(&user, &org).await;
    assert_eq!(usage.peak, Some(peak(second_term, 1, renewed_at)));
    assert_eq!(usage.previous_peak, ended);
}

// Fails if a license signed again in the same term, as for more Benchmark Series, starts the
// term's peak over.
#[tokio::test]
async fn a_re_sign_in_the_same_term_keeps_its_peak() {
    let (server, clock) = server_on_a_clock().await;
    let Licensed {
        user, org, private, ..
    } = licensed(&server, "re-sign").await;
    let start = clock.now();
    let term_end = after(start, 365 * DAY);
    server
        .license_org_with(&user, &org, &sign_license(org.uuid, 2, start, term_end))
        .await;
    accepted(&server, &user, &private, 3).await;

    clock.advance(DAY);
    let upsell = sign_license(org.uuid, 5, clock.now(), term_end);
    server.license_org_with(&user, &org, &upsell).await;
    let usage = server.usage(&user, &org).await;
    assert_eq!(usage.peak, Some(peak(term_end, 3, start)));
    assert_eq!(usage.previous_peak, None);

    // A report under the new license counts no more, so the term's peak stands.
    assert_eq!(accepted(&server, &user, &private, 1).await.license, None);
    assert_eq!(
        server.usage(&user, &org).await.peak,
        Some(peak(term_end, 3, start))
    );
}

// Fails if an older license pasted over a newer one is taken, which would hide the newer term and
// its peak, or if a license for the same term, as for an upsell, is refused.
#[tokio::test]
async fn an_older_license_is_refused_over_a_newer_one() {
    let (server, clock) = server_on_a_clock().await;
    let Licensed {
        user, org, private, ..
    } = licensed(&server, "older").await;
    let start = clock.now();
    let older_term = after(start, 30 * DAY);
    let newer_term = after(start, 60 * DAY);
    server
        .license_org_with(&user, &org, &sign_license(org.uuid, 2, start, newer_term))
        .await;
    accepted(&server, &user, &private, 3).await;

    clock.advance(DAY);
    let older = sign_license(org.uuid, 2, clock.now(), older_term);
    let (status, body) = server.patch_license(&user, &org, &older).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(body.contains("newer license"), "{body}");
    assert!(body.contains(&newer_term.to_string()), "{body}");
    assert!(body.contains(&older_term.to_string()), "{body}");
    let usage = server.usage(&user, &org).await;
    assert_eq!(
        usage.license.map(|license| license.term_end),
        Some(newer_term)
    );
    assert_eq!(usage.peak, Some(peak(newer_term, 3, start)));

    let upsell = sign_license(org.uuid, 5, clock.now(), newer_term);
    let (status, body) = server.patch_license(&user, &org, &upsell).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

// Fails if a held license in its grace lets an older license in, which would move the
// organization back a term.
#[tokio::test]
async fn a_license_in_its_grace_still_refuses_an_older_one() {
    let (server, clock) = server_on_a_clock().await;
    let Licensed { user, org, .. } = licensed(&server, "grace-older").await;
    let start = clock.now();
    let held_term = after(start, 10 * DAY);
    server
        .license_org_with(&user, &org, &sign_license(org.uuid, 2, start, held_term))
        .await;

    // Both licenses are in their grace, and the pasted one's term ended first.
    clock.advance(12 * DAY);
    let older = sign_license(org.uuid, 2, start, after(start, 5 * DAY));
    let (status, body) = server.patch_license(&user, &org, &older).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        server
            .usage(&user, &org)
            .await
            .license
            .map(|license| license.term_end),
        Some(held_term)
    );
}

// Fails if a held license that is not valid for the organization, as one for another
// organization, refuses a valid license for it with an earlier term.
#[tokio::test]
async fn a_license_for_another_organization_refuses_nothing() {
    let (server, clock) = server_on_a_clock().await;
    let Licensed { user, org, .. } = licensed(&server, "other-held").await;
    let start = clock.now();
    server.store_license(
        &org,
        &sign_license(OrganizationUuid::new(), 2, start, after(start, 365 * DAY)),
    );

    let term_end = after(start, 30 * DAY);
    let (status, body) = server
        .patch_license(&user, &org, &sign_license(org.uuid, 2, start, term_end))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        server
            .usage(&user, &org)
            .await
            .license
            .map(|license| license.term_end),
        Some(term_end)
    );
}

// Fails if an organization whose license has expired loses sight of the last term's peak.
#[tokio::test]
async fn an_expired_license_still_shows_its_last_terms_peak() {
    let (server, clock) = server_on_a_clock().await;
    let Licensed {
        user, org, private, ..
    } = licensed(&server, "expired-peak").await;
    let start = clock.now();
    let term_end = after(start, 10 * DAY);
    server
        .license_org_with(&user, &org, &sign_license(org.uuid, 2, start, term_end))
        .await;
    accepted(&server, &user, &private, 2).await;

    clock.advance(40 * DAY);
    let usage = server.usage(&user, &org).await;
    assert_eq!(usage.kind, UsageKind::SelfHostedFree);
    assert_eq!(usage.peak, None);
    assert_eq!(usage.previous_peak, Some(peak(term_end, 2, start)));
}

// Fails if the count of a license reaches back less than 30 days, or more.
#[tokio::test]
async fn the_count_reaches_back_30_days() {
    const HOUR: i64 = 60 * 60;

    let (server, clock) = server_on_a_clock().await;
    let Licensed {
        user,
        org,
        private,
        public,
    } = licensed(&server, "window").await;
    let start = clock.now();
    server
        .license_org_with(
            &user,
            &org,
            &sign_license(
                org.uuid,
                TEST_LICENSE_SERIES,
                start,
                after(start, 365 * DAY),
            ),
        )
        .await;
    // 1 Benchmark Series in each project.
    accepted(&server, &user, &private, 1).await;
    clock.advance(29 * DAY + 12 * HOUR);
    accepted(&server, &user, &public, 1).await;
    assert_eq!(server.usage(&user, &org).await.active_series, Some(2));

    // Past 30 days from the first report, its Benchmark Series drops out.
    clock.advance(12 * HOUR + 1);
    assert_eq!(server.usage(&user, &org).await.active_series, Some(1));
}

// Fails if a report is not warned with both dates in the grace, a private report is
// accepted after the grace, or a public report is refused after it.
#[tokio::test]
async fn the_grace_warns_then_the_organization_falls_back_to_free() {
    let (server, clock) = server_on_a_clock().await;
    let Licensed {
        user,
        org,
        private,
        public,
    } = licensed(&server, "grace").await;
    let start = clock.now();
    let term_end = after(start, 10 * DAY);
    let expiration = after(term_end, 30 * DAY);
    let license = sign_license(org.uuid, TEST_LICENSE_SERIES, start, term_end);
    server.license_org_with(&user, &org, &license).await;

    // In the term, a report carries no warning.
    clock.advance(9 * DAY);
    assert_eq!(accepted(&server, &user, &private, 1).await.license, None);

    // In the grace, a private report is accepted and warns with the term end and the expiration.
    clock.advance(3 * DAY);
    let report = accepted(&server, &user, &private, 1).await;
    assert_eq!(
        report.license,
        Some(JsonReportLicense {
            over: None,
            grace: Some(JsonLicenseGrace {
                term_end,
                expiration,
            }),
        })
    );
    assert_eq!(
        server.usage(&user, &org).await.kind,
        UsageKind::SelfHostedLicensed
    );

    // At the expiration the organization is Free: a private report is refused,
    // a public report is accepted, and the private project stays readable.
    clock.advance(expiration.timestamp() - clock.now().timestamp());
    let (status, body) = post_series(&server, &user, &private, 1).await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED, "{body}");
    assert_eq!(accepted(&server, &user, &public, 1).await.license, None);
    assert_eq!(
        server.usage(&user, &org).await.kind,
        UsageKind::SelfHostedFree
    );
    assert_eq!(report_count(&server, &user, &private).await, 2);
}

// Fails if a license that is expired, signed by another key, for another organization, or in the
// earlier format blocks a report to a public project, or allows one to a private project.
#[tokio::test]
async fn an_invalid_license_never_blocks_a_public_project() {
    let (server, clock) = server_on_a_clock().await;
    let Licensed {
        user,
        org,
        private,
        public,
    } = licensed(&server, "invalid").await;
    let now = clock.now();
    for (license, what) in [
        (
            sign_license(
                org.uuid,
                TEST_LICENSE_SERIES,
                after(now, -400 * DAY),
                after(now, -31 * DAY),
            ),
            "an expired license",
        ),
        (
            sign_foreign_license(org.uuid, TEST_LICENSE_SERIES, now, after(now, 365 * DAY)),
            "a license signed by another key",
        ),
        (
            sign_license(
                OrganizationUuid::new(),
                TEST_LICENSE_SERIES,
                now,
                after(now, 365 * DAY),
            ),
            "a license for another organization",
        ),
        (
            sign_earlier_license(org.uuid, now, after(now, 365 * DAY)),
            "a license in the earlier format",
        ),
    ] {
        server.store_license(&org, &license);
        let (status, body) = post_series(&server, &user, &public, 1).await;
        assert_eq!(status, StatusCode::CREATED, "{what}: {body}");
        let (status, body) = post_series(&server, &user, &private, 1).await;
        assert_eq!(status, StatusCode::PAYMENT_REQUIRED, "{what}: {body}");
        assert_eq!(
            server.usage(&user, &org).await.kind,
            UsageKind::SelfHostedFree,
            "{what}"
        );
    }

    // A valid license stored the same way allows the private project.
    server.store_license(
        &org,
        &sign_license(org.uuid, TEST_LICENSE_SERIES, now, after(now, 365 * DAY)),
    );
    accepted(&server, &user, &private, 1).await;
}

/// How many reports the project holds, read by its member.
async fn report_count(server: &TestServer, user: &TestUser, project: &TestProject) -> usize {
    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/reports")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    resp.json::<Vec<serde_json::Value>>()
        .await
        .expect("Failed to parse the reports")
        .len()
}
