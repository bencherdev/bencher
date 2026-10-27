#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for auth signup endpoint.

use bencher_api_tests::TestServer;
use bencher_json::{JsonSignup, system::auth::JsonAuthAck};
use http::StatusCode;

// POST /v0/auth/signup - successful signup
#[tokio::test]
async fn signup_success() {
    let server = TestServer::new().await;

    let body = JsonSignup {
        name: "Test User".parse().unwrap(),
        slug: None,
        email: "signup@example.com".parse().unwrap(),
        #[cfg(feature = "plus")]
        plan: None,
        invite: None,
        claim: None,
        i_agree: true,
        #[cfg(feature = "plus")]
        recaptcha_token: None,
        #[cfg(feature = "plus")]
        authorization: None,
    };

    let resp = server
        .client
        .post(server.api_url("/v0/auth/signup"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let ack: JsonAuthAck = resp.json().await.expect("Failed to parse response");
    assert_eq!(ack.email, body.email);
}

// POST /v0/auth/signup - custom slug
#[tokio::test]
async fn signup_with_custom_slug() {
    let server = TestServer::new().await;

    let body = JsonSignup {
        name: "Custom Slug User".parse().unwrap(),
        slug: Some("custom-slug-user".parse().unwrap()),
        email: "customslug@example.com".parse().unwrap(),
        #[cfg(feature = "plus")]
        plan: None,
        invite: None,
        claim: None,
        i_agree: true,
        #[cfg(feature = "plus")]
        recaptcha_token: None,
        #[cfg(feature = "plus")]
        authorization: None,
    };

    let resp = server
        .client
        .post(server.api_url("/v0/auth/signup"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

// POST /v0/auth/signup - duplicate email returns conflict
#[tokio::test]
async fn signup_duplicate_email() {
    let server = TestServer::new().await;

    let body = JsonSignup {
        name: "Test User".parse().unwrap(),
        slug: None,
        email: "duplicate@example.com".parse().unwrap(),
        #[cfg(feature = "plus")]
        plan: None,
        invite: None,
        claim: None,
        i_agree: true,
        #[cfg(feature = "plus")]
        recaptcha_token: None,
        #[cfg(feature = "plus")]
        authorization: None,
    };

    // First signup
    let resp = server
        .client
        .post(server.api_url("/v0/auth/signup"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Second signup with same email returns conflict
    let resp = server
        .client
        .post(server.api_url("/v0/auth/signup"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

// POST /v0/auth/signup - must agree to terms
#[tokio::test]
async fn signup_must_agree_to_terms() {
    let server = TestServer::new().await;

    let body = JsonSignup {
        name: "Test User".parse().unwrap(),
        slug: None,
        email: "noagree@example.com".parse().unwrap(),
        #[cfg(feature = "plus")]
        plan: None,
        invite: None,
        claim: None,
        i_agree: false,
        #[cfg(feature = "plus")]
        recaptcha_token: None,
        #[cfg(feature = "plus")]
        authorization: None,
    };

    let resp = server
        .client
        .post(server.api_url("/v0/auth/signup"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// POST /v0/auth/signup - invalid email format
#[tokio::test]
async fn signup_invalid_email() {
    let server = TestServer::new().await;

    let body = serde_json::json!({
        "name": "Test User",
        "email": "not-an-email",
        "i_agree": true
    });

    let resp = server
        .client
        .post(server.api_url("/v0/auth/signup"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// Kills a signup whose emailed link drops the app's request to connect, or carries one that does
// not wait for an answer. Only the email that returns to one warns of it.
#[cfg(feature = "plus")]
#[tokio::test]
async fn signup_link_returns_to_a_pending_authorization() {
    use bencher_api_tests::{
        LogCapture, TestServerOptions,
        helpers::{oauth_server, seed_oauth_authorization},
    };
    use bencher_json::{DateTime, OAuthAuthorizationUuid};

    let capture = LogCapture::default();
    let server = TestServer::new_with_options(TestServerOptions {
        oauth: Some(oauth_server()),
        log: Some(capture.logger()),
        ..TestServerOptions::default()
    })
    .await;
    let pending = seed_oauth_authorization(&server, DateTime::now());

    for (email, authorization, names) in [
        (
            "pending@example.com",
            pending,
            ["token", "authorization"].as_slice(),
        ),
        (
            "unknown@example.com",
            OAuthAuthorizationUuid::new(),
            ["token"].as_slice(),
        ),
    ] {
        let body = JsonSignup {
            name: "Connect User".parse().unwrap(),
            slug: None,
            email: email.parse().unwrap(),
            plan: None,
            invite: None,
            claim: None,
            i_agree: true,
            recaptcha_token: None,
            authorization: Some(authorization),
        };
        let resp = server
            .client
            .post(server.api_url("/v0/auth/signup"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::ACCEPTED);

        let link = capture.email_link("Confirm Email");
        assert_eq!(link.origin(), server.context().console_url.origin());
        let found = link
            .query_pairs()
            .map(|(name, _)| name.into_owned())
            .collect::<Vec<_>>();
        assert_eq!(found, names, "Link: {link}");
        let returns = names.contains(&"authorization");
        if returns {
            assert!(
                link.query_pairs()
                    .any(|(name, value)| name == "authorization" && value == pending.to_string()),
                "Link: {link}"
            );
        }
        assert_eq!(
            capture.email("Confirm Email").contains(
                "An app asked to connect to your Bencher account. If you did not start this, ignore this email."
            ),
            returns,
            "Only the email that returns to the request warns of it"
        );
    }
}

// POST /v0/auth/signup - claim an unclaimed organization
#[tokio::test]
async fn signup_claim_unclaimed_organization() {
    use bencher_api_tests::helpers::{base_timestamp, user_organizations};
    use bencher_json::OrganizationUuid;
    use bencher_schema::{model::organization::InsertOrganization, schema};
    use diesel::RunQueryDsl as _;

    let server = TestServer::new().await;
    let timestamp = base_timestamp();
    let organization = InsertOrganization {
        uuid: OrganizationUuid::new(),
        name: "Unclaimed".parse().unwrap(),
        slug: "unclaimed".parse().unwrap(),
        created: timestamp,
        modified: timestamp,
    };
    diesel::insert_into(schema::organization::table)
        .values(&organization)
        .execute(&mut server.db_conn())
        .unwrap();

    let user = server
        .signup_with_claim("Claim User", "claim@example.com", Some(organization.uuid))
        .await
        .uuid;
    assert_eq!(user_organizations(&server, user), [organization.uuid]);
}

// POST /v0/auth/signup - claim a claimed organization
#[tokio::test]
async fn signup_claim_claimed_organization() {
    use bencher_api_tests::helpers::user_organizations;
    use bencher_json::OrganizationUuid;

    let server = TestServer::new().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let organization = server.create_org(&leader, "Claimed").await;

    let user = server
        .signup_with_claim("Claim User", "claim@example.com", Some(organization.uuid))
        .await
        .uuid;
    assert_eq!(
        user_organizations(&server, user),
        [OrganizationUuid::from(user)]
    );
}
