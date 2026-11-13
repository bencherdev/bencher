#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for signing in with GitHub and Google: the state row and the cookie that binds
//! it to the browser, PKCE, Google's ID token, the one linking rule, and single sign-on by Google's
//! hosted domain.

use std::{
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use bencher_api_tests::{
    LogCapture, TestServer, TestServerOptions, TestUser,
    github::{GitHubServer, HOLD, Received, Unscripted},
    helpers::{
        github_client, google_client, oauth_server, seed_oauth_authorization, user_organizations,
    },
    session_cookie,
};
use bencher_json::{
    BindingSecret, BindingSecretHash, Clock, DateTime, JsonOAuthUrl, JsonProviderInbox,
    JsonProviderSignIn, JsonSession, JsonSignedIn, OAuthCodeChallenge, OAuthCodeVerifier,
    OrganizationUuid, PlanLevel, ProviderState, ProviderStateHash, SessionMethod, SessionToken,
    SessionTokenHash, SignInCode, UserSlug, system::config::JsonRateLimiting,
};
use bencher_schema::{
    context::RateLimiting,
    model::user::session::cookie::{PendingCookie, ProviderCookie, SessionCookie},
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::{HeaderMap, StatusCode, header};
use serde_json::{Value, json};
use url::Url;

const SAME_ORIGIN: (&str, &str) = ("Sec-Fetch-Site", "same-origin");
const CODE: &str = "code-marker";
const GITHUB_TOKEN: &str = "github-token-marker";
const GOOGLE_TOKEN: &str = "google-token-marker";
const GITHUB_REDIRECT: &str = "http://localhost:3000/auth/github";
const GOOGLE_REDIRECT: &str = "http://localhost:3000/auth/google";
const GOOGLE_CLIENT_ID: &str = "test-client";
const MINUTE: i64 = 60;
const RELAY_SECRET: &str = "a-relay-secret-for-tests";

// GitHub's authorize URL carries the S256 challenge of the sign-in's verifier, the state, and the
// console's page, and asks for no scope; the exchange proves the verifier and names the same page.
// The start binds the browser by a cookie of the session cookie's form, for ten minutes, in an
// answer no cache keeps.
// Kills a start without a challenge, a challenge of another verifier, an exchange that proves no
// verifier, a cookie that is not HttpOnly, not Secure, not Strict, or outlives the sign-in, and a
// start a cache may answer with another's state.
#[tokio::test]
async fn github_sign_in_proves_its_s256_challenge() {
    let github = GitHubServer::start(github_account(1, "octo-cat", "octo@example.com")).await;
    let google = Unscripted::bind().await;
    let Harness { server, .. } = harness(&github.base(), &google.base()).await;

    let resp = start_raw(&server, "github", &json!({}), &[SAME_ORIGIN]).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()[header::CACHE_CONTROL], "no-store");
    let set_cookies = set_cookies(resp.headers());
    let JsonOAuthUrl { url } = resp.json().await.expect("Invalid answer");
    let [oauth_cookie] = set_cookies.as_slice() else {
        panic!("Not one cookie: {set_cookies:?}");
    };
    let (binding, attributes) = oauth_cookie
        .strip_prefix("__Host-bencher_oauth=")
        .and_then(|cookie| cookie.split_once("; "))
        .expect("No provider cookie");
    assert_eq!(
        attributes,
        "HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=600"
    );
    assert_eq!(
        (url.host_str(), url.path()),
        (Some("github.com"), "/login/oauth/authorize")
    );
    assert_eq!(
        query(&url, "code_challenge_method").as_deref(),
        Some("S256")
    );
    assert_eq!(
        query(&url, "redirect_uri").as_deref(),
        Some(GITHUB_REDIRECT)
    );
    assert_eq!(query(&url, "scope"), None);
    let challenge: OAuthCodeChallenge = query(&url, "code_challenge")
        .expect("No challenge")
        .parse()
        .expect("Invalid challenge");
    let state = query(&url, "state").expect("No state");

    let resp = finish(&server, "github", Some(binding), &state, None).await;
    signed_in(&server, resp, SessionMethod::Github).await;

    let [exchange, _user, _emails]: [Received; 3] = github
        .received()
        .await
        .try_into()
        .ok()
        .expect("Wrong request count");
    let form = exchange.form();
    let verifier: OAuthCodeVerifier = form
        .get("code_verifier")
        .expect("No verifier")
        .parse()
        .expect("Invalid verifier");
    assert!(verifier.verifies(&challenge));
    assert_eq!(form.get("code").map(String::as_str), Some(CODE));
    assert_eq!(
        form.get("redirect_uri").map(String::as_str),
        Some(GITHUB_REDIRECT)
    );
}

// Google's authorize URL asks for an ID token with OpenID Connect's scopes, the sign-in's nonce,
// the S256 challenge of its verifier, and the registered redirect; the exchange proves the
// verifier, and the ID token it returns names the account, in place of a userinfo read.
// Kills a start without the nonce, the scopes, or the challenge, and an exchange that proves no
// verifier or reads anything but the token endpoint.
#[tokio::test]
async fn google_sign_in_reads_its_id_token_and_proves_its_challenge() {
    let github = Unscripted::bind().await;
    let google = Unscripted::bind().await;
    let Harness { server, clock, .. } = harness(&github.base(), &google.base()).await;

    let started = start(&server, "google").await;
    let url = &started.url;
    assert_eq!(
        (url.host_str(), url.path()),
        (Some("accounts.google.com"), "/o/oauth2/v2/auth")
    );
    assert_eq!(query(url, "scope").as_deref(), Some("openid email profile"));
    assert_eq!(query(url, "code_challenge_method").as_deref(), Some("S256"));
    assert_eq!(query(url, "redirect_uri").as_deref(), Some(GOOGLE_REDIRECT));
    let challenge: OAuthCodeChallenge = query(url, "code_challenge")
        .expect("No challenge")
        .parse()
        .expect("Invalid challenge");
    let nonce = query(url, "nonce").expect("No nonce");
    let google = google.script(vec![google_token(&google_claims(
        &nonce,
        &clock,
        "110169484474386276334",
        "ada@example.com",
    ))]);

    let resp = finish(
        &server,
        "google",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    let (json, _session) = signed_in(&server, resp, SessionMethod::Google).await;
    assert!(json.new);
    assert_eq!(json.user.email.as_ref(), "ada@example.com");
    assert_eq!(json.user.name.as_ref(), "Ada Lovelace");
    assert_eq!(
        pins(&server),
        [(
            json.user.uuid.to_string(),
            1,
            "110169484474386276334".to_owned(),
            "ada@example.com".to_owned()
        )]
    );

    let [token]: [Received; 1] = google
        .received()
        .await
        .try_into()
        .ok()
        .expect("Wrong request count");
    assert!(
        token.request_line().starts_with("POST /token "),
        "{}",
        token.request_line()
    );
    let form = token.form();
    let verifier: OAuthCodeVerifier = form
        .get("code_verifier")
        .expect("No verifier")
        .parse()
        .expect("Invalid verifier");
    assert!(verifier.verifies(&challenge));
    assert_eq!(form.get("code").map(String::as_str), Some(CODE));
}

// An ID token whose nonce is another sign-in's, one that has expired by the server's clock, and one
// whose email Google has not verified, each refuse, and none makes an account or a session.
// Kills a nonce check against anything but the sign-in's own nonce, an expiry read off any clock
// but the server's, and a sign-in with an unverified email.
#[tokio::test]
async fn google_id_token_must_be_for_its_own_sign_in() {
    let github = Unscripted::bind().await;
    let google = Unscripted::bind().await;
    let Harness { server, clock, .. } = harness(&github.base(), &google.base()).await;
    let users_before = users(&server);

    let first = start(&server, "google").await;
    let second = start(&server, "google").await;
    let third = start(&server, "google").await;
    let nonce = |started: &Started| query(&started.url, "nonce").expect("No nonce");
    let other_nonce = google_claims(&nonce(&second), &clock, "1", "ada@example.com");
    let mut expired = google_claims(&nonce(&second), &clock, "2", "grace@example.com");
    expired["exp"] = json!(clock.now().timestamp() + MINUTE);
    let mut unverified = google_claims(&nonce(&third), &clock, "3", "hedy@example.com");
    unverified["email_verified"] = json!(false);
    let google = google.script(vec![
        google_token(&other_nonce),
        google_token(&expired),
        google_token(&unverified),
    ]);

    let resp = finish(
        &server,
        "google",
        Some(&first.binding),
        &first.state(),
        None,
    )
    .await;
    assert_refused(resp).await;
    clock.advance(2 * MINUTE);
    let resp = finish(
        &server,
        "google",
        Some(&second.binding),
        &second.state(),
        None,
    )
    .await;
    assert_refused(resp).await;
    let resp = finish(
        &server,
        "google",
        Some(&third.binding),
        &third.state(),
        None,
    )
    .await;
    assert_refused(resp).await;

    assert_eq!(google.received().await.len(), 3);
    assert_eq!(users(&server), users_before);
    assert_eq!(sessions(&server), 0);
}

// A state works once, and only with the cookie of the browser that started it: no cookie, another
// browser's, and the cookie of a sign-in that has ended are each refused, and the provider is never
// asked. Nor does a GitHub state finish a Google sign-in.
// Kills a finish that skips the binding check, checks it after the exchange, or lets a state work
// twice, and one that ignores the provider the state was started for.
#[tokio::test]
async fn state_works_once_and_only_in_the_browser_that_started_it() {
    let github = GitHubServer::start(
        [
            github_account(1, "octo-cat", "octo@example.com"),
            github_account(2, "mona", "mona@example.com"),
        ]
        .concat(),
    )
    .await;
    let google = Unscripted::bind().await;
    let Harness { server, .. } = harness(&github.base(), &google.base()).await;

    let first = start(&server, "github").await;
    let second = start(&server, "github").await;
    for binding in [None, Some(second.binding.as_str())] {
        let resp = finish(&server, "github", binding, &first.state(), None).await;
        assert_refused(resp).await;
    }
    let resp = finish(
        &server,
        "google",
        Some(&first.binding),
        &first.state(),
        None,
    )
    .await;
    assert_refused(resp).await;
    let resp = finish(
        &server,
        "github",
        Some(&first.binding),
        &first.state(),
        None,
    )
    .await;
    signed_in(&server, resp, SessionMethod::Github).await;
    let resp = finish(
        &server,
        "github",
        Some(&first.binding),
        &first.state(),
        None,
    )
    .await;
    assert_refused(resp).await;

    let resp = finish(
        &server,
        "github",
        Some(&first.binding),
        &second.state(),
        None,
    )
    .await;
    assert_refused(resp).await;
    let resp = finish(
        &server,
        "github",
        Some(&second.binding),
        &second.state(),
        None,
    )
    .await;
    signed_in(&server, resp, SessionMethod::Github).await;

    assert_eq!(github.received().await.len(), 6);
}

// A state works once even when its browser sends it many times at once: one finish signs in, and
// every other is refused.
// Kills a state found outside the write that ends it.
#[tokio::test]
async fn state_works_once_under_concurrent_finishes() {
    const FINISHES: usize = 8;
    let github = GitHubServer::start(
        (0..FINISHES)
            .flat_map(|_| github_account(1, "octo-cat", "octo@example.com"))
            .collect(),
    )
    .await;
    let google = Unscripted::bind().await;
    let Harness { server, .. } = harness(&github.base(), &google.base()).await;
    let started = start(&server, "github").await;

    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..FINISHES {
        let request = finish_request(
            &server,
            "github",
            Some(&started.binding),
            &started.state(),
            None,
        );
        requests.spawn(async move { request.send().await.expect("Request failed").status() });
    }
    let statuses = requests.join_all().await;
    let count = |status: StatusCode| statuses.iter().filter(|s| **s == status).count();
    assert_eq!(count(StatusCode::OK), 1, "{statuses:?}");
    assert_eq!(
        count(StatusCode::UNAUTHORIZED),
        FINISHES - 1,
        "{statuses:?}"
    );
    assert_eq!(sessions(&server), 1);
}

// A state ends ten minutes after its start, by the server's clock, though GitHub would still
// answer it.
// Kills a sign-in that lives past ten minutes, or not as long.
#[tokio::test]
async fn state_ends_ten_minutes_after_its_start() {
    let github = GitHubServer::start(
        [
            github_account(1, "octo-cat", "octo@example.com"),
            github_account(2, "mona", "mona@example.com"),
        ]
        .concat(),
    )
    .await;
    let google = Unscripted::bind().await;
    let Harness { server, clock, .. } = harness(&github.base(), &google.base()).await;

    let first = start(&server, "github").await;
    let second = start(&server, "github").await;
    clock.advance(10 * MINUTE - 1);
    let resp = finish(
        &server,
        "github",
        Some(&first.binding),
        &first.state(),
        None,
    )
    .await;
    signed_in(&server, resp, SessionMethod::Github).await;
    clock.advance(1);
    let resp = finish(
        &server,
        "github",
        Some(&second.binding),
        &second.state(),
        None,
    )
    .await;
    assert_refused(resp).await;
}

// A provider that never answers its exchange holds no finish open: once the exchange's time is up,
// the finish is refused, as any provider failure is.
// Kills an exchange that waits on either provider for as long as it takes.
#[tokio::test]
async fn finish_gives_up_on_a_provider_that_never_answers() {
    let held = || vec![(200, vec![(HOLD, String::new())], String::new())];
    let github = GitHubServer::start_with_headers(held()).await;
    let google = GitHubServer::start_with_headers(held()).await;
    let Harness { server, .. } = harness(&github.base(), &google.base()).await;
    let started_and_finished = |provider: &'static str| {
        let server = &server;
        async move {
            let started = start(server, provider).await;
            finish(
                server,
                provider,
                Some(&started.binding),
                &started.state(),
                None,
            )
            .await
        }
    };

    let (github_resp, google_resp) = tokio::time::timeout(Duration::from_secs(30), async {
        tokio::join!(
            started_and_finished("github"),
            started_and_finished("google")
        )
    })
    .await
    .expect("A finish waited on its provider past the exchange's time");
    github.until_held().await;
    google.until_held().await;
    assert_refused(github_resp).await;
    assert_refused(google_resp).await;
}

// A provider account pinned to a user signs that user in, whatever email the provider names now,
// keeps its handle current, and ends the session the browser presented.
// Kills a pinned sign-in that matches by email, makes another account, keeps a stale handle, or
// leaves the presented session alive.
#[tokio::test]
async fn pinned_account_signs_its_user_in() {
    let github = GitHubServer::start(
        [
            github_account(42, "octo-cat", "octo@example.com"),
            github_account(42, "octo-renamed", "octo-new@example.com"),
        ]
        .concat(),
    )
    .await;
    let google = Unscripted::bind().await;
    let Harness { server, admin, .. } = harness(&github.base(), &google.base()).await;

    let (first, _session) = sign_in(&server, "github", None).await;
    assert!(first.new);
    let users_before = users(&server);
    let presented = server.session(&admin);

    let (second, session) = sign_in(&server, "github", Some(&presented)).await;
    assert!(!second.new);
    assert_eq!(second.user.uuid, first.user.uuid);
    assert_eq!(second.user.email.as_ref(), "octo@example.com");
    assert_eq!(session.method, SessionMethod::Github);
    assert_eq!(users(&server), users_before);
    assert_eq!(
        pins(&server),
        [(
            first.user.uuid.to_string(),
            0,
            "42".to_owned(),
            "octo-renamed".to_owned()
        )]
    );
    assert_eq!(
        read_session(&server, &presented).await,
        StatusCode::UNAUTHORIZED
    );

    github.received().await;
}

// A provider account pinned to no one whose email has no account signs up: named by the provider,
// its email verified, pinned at birth, in its own organization, carrying the plan and the app's
// request it started with.
// Kills a sign-up that is not pinned, unverified, misnamed, or drops what the sign-in carries.
#[tokio::test]
async fn unknown_email_signs_up_pinned() {
    let github =
        GitHubServer::start(github_named(7, "ada-l", "Ada Lovelace", "ada@example.com")).await;
    let google = Unscripted::bind().await;
    let Harness { server, .. } = harness_with(
        &github.base(),
        &google.base(),
        TestServerOptions {
            oauth: Some(oauth_server()),
            ..TestServerOptions::default()
        },
    )
    .await;
    let authorization = seed_oauth_authorization(&server, DateTime::now());

    let resp = start_raw(
        &server,
        "github",
        &json!({"plan": "team", "authorization": authorization}),
        &[SAME_ORIGIN],
    )
    .await;
    let started = Started::read(resp).await;
    let resp = finish(
        &server,
        "github",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    let (json, _session) = signed_in(&server, resp, SessionMethod::Github).await;

    assert!(json.new);
    assert_eq!(json.plan, Some(PlanLevel::Team));
    assert_eq!(json.authorization, Some(authorization));
    assert_eq!(json.user.name.as_ref(), "Ada Lovelace");
    assert!(email_verified(&server, "ada@example.com"));
    assert_eq!(
        user_organizations(&server, json.user.uuid),
        [OrganizationUuid::from(json.user.uuid)]
    );
    assert_eq!(
        pins(&server),
        [(
            json.user.uuid.to_string(),
            0,
            "7".to_owned(),
            "ada-l".to_owned()
        )]
    );

    github.received().await;
}

// A provider account that shows no name names its new user by its login, which a provider may spell
// with an underscore, such as the `handle_shortcode` login of a GitHub Enterprise Managed User.
// Kills a sign-up that refuses such a login, and one that cannot name the new user from it.
#[tokio::test]
async fn sign_up_takes_a_login_with_an_underscore() {
    let github = GitHubServer::start(github_account(
        7,
        "muriel_bagge",
        "muriel_bagge@example.com",
    ))
    .await;
    let google = Unscripted::bind().await;
    let Harness { server, .. } = harness(&github.base(), &google.base()).await;

    let (json, _session) = sign_in(&server, "github", None).await;

    assert!(json.new);
    assert_eq!(json.user.name.as_ref(), "muriel_bagge");
    assert_eq!(
        json.user.slug,
        "muriel-bagge".parse::<UserSlug>().expect("Invalid slug")
    );

    github.received().await;
}

// An invite joins its organization only when the provider verified the address it is for; any other
// invite is skipped, and the new account gets its own organization.
// Kills a provider sign-in that accepts an invite for an address the provider did not verify.
#[tokio::test]
async fn invite_joins_only_for_the_address_the_provider_verified() {
    let github = GitHubServer::start(
        [
            github_account(1, "grace", "grace@example.com"),
            github_account(2, "carol", "carol@example.com"),
        ]
        .concat(),
    )
    .await;
    let google = Unscripted::bind().await;
    let Harness { server, admin, .. } = harness(&github.base(), &google.base()).await;
    let org = server.create_org(&admin, "Invited").await;

    for (email, joins) in [("grace@example.com", true), ("bob@example.com", false)] {
        let invite = server.invite(&admin, &org, email);
        let resp = start_raw(
            &server,
            "github",
            &json!({"invite": invite.as_ref()}),
            &[SAME_ORIGIN],
        )
        .await;
        let started = Started::read(resp).await;
        let resp = finish(
            &server,
            "github",
            Some(&started.binding),
            &started.state(),
            None,
        )
        .await;
        let (json, _session) = signed_in(&server, resp, SessionMethod::Github).await;
        let organizations = user_organizations(&server, json.user.uuid);
        assert_eq!(organizations.contains(&org.uuid), joins, "{email}");
        assert_eq!(
            organizations.contains(&OrganizationUuid::from(json.user.uuid)),
            !joins,
            "{email}"
        );
    }

    github.received().await;
}

// A pinned account whose provider now names an address other than its user's still signs in, and
// an invite for the provider's address joins nothing: the invite is for another account's address.
// Kills a pinned sign-in that fails on such an invite, and one that accepts it.
#[tokio::test]
async fn pinned_sign_in_skips_an_invite_for_the_providers_other_address() {
    let github = GitHubServer::start(
        [
            github_account(4, "ada", "ada@example.com"),
            github_account(4, "ada", "ada@elsewhere.example"),
        ]
        .concat(),
    )
    .await;
    let google = Unscripted::bind().await;
    let Harness { server, admin, .. } = harness(&github.base(), &google.base()).await;
    let org = server.create_org(&admin, "Invited").await;
    let (ada, _session) = sign_in(&server, "github", None).await;
    let invite = server.invite(&admin, &org, "ada@elsewhere.example");

    let resp = start_raw(
        &server,
        "github",
        &json!({"invite": invite.as_ref()}),
        &[SAME_ORIGIN],
    )
    .await;
    let started = Started::read(resp).await;
    let resp = finish(
        &server,
        "github",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    let (json, _session) = signed_in(&server, resp, SessionMethod::Github).await;
    assert_eq!(json.user.uuid, ada.user.uuid);
    assert!(!user_organizations(&server, ada.user.uuid).contains(&org.uuid));

    github.received().await;
}

// The same pinned account, now with an invite for its user's own address: the provider names
// another address, so it did not verify the invited one, and the invite joins nothing.
// Kills a pinned sign-in that accepts an invite for any address but the one the provider verified.
#[tokio::test]
async fn pinned_sign_in_skips_an_invite_for_the_users_unverified_address() {
    let github = GitHubServer::start(
        [
            github_account(4, "ada", "ada@example.com"),
            github_account(4, "ada", "ada@elsewhere.example"),
        ]
        .concat(),
    )
    .await;
    let google = Unscripted::bind().await;
    let Harness { server, admin, .. } = harness(&github.base(), &google.base()).await;
    let org = server.create_org(&admin, "Invited").await;
    let (ada, _session) = sign_in(&server, "github", None).await;
    let invite = server.invite(&admin, &org, "ada@example.com");

    let resp = start_raw(
        &server,
        "github",
        &json!({"invite": invite.as_ref()}),
        &[SAME_ORIGIN],
    )
    .await;
    let started = Started::read(resp).await;
    let resp = finish(
        &server,
        "github",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    let (json, _session) = signed_in(&server, resp, SessionMethod::Github).await;
    assert_eq!(json.user.uuid, ada.user.uuid);
    assert!(!user_organizations(&server, ada.user.uuid).contains(&org.uuid));

    github.received().await;
}

// A revoke during a provider sign-in drops the invite it carries and keeps the sign-in, which then
// signs the reader in with an organization of their own.
// Kills a revoke that ends the provider sign-ins carrying the invite.
#[tokio::test]
async fn revoke_during_a_provider_sign_in_drops_only_the_invite() {
    let github = GitHubServer::start(github_account(1, "grace", "grace@example.com")).await;
    let google = Unscripted::bind().await;
    let Harness { server, admin, .. } = harness(&github.base(), &google.base()).await;
    let org = server.create_org(&admin, "Invited").await;
    let invite = server.invite(&admin, &org, "grace@example.com");
    let resp = start_raw(
        &server,
        "github",
        &json!({"invite": invite.as_ref()}),
        &[SAME_ORIGIN],
    )
    .await;
    let started = Started::read(resp).await;

    let uuid: String = schema::invite::table
        .select(schema::invite::uuid)
        .first(&mut server.db_conn())
        .expect("No invitation");
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{}/invites/{uuid}", org.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let carried: Vec<Option<i32>> = schema::provider_state::table
        .select(schema::provider_state::invite_id)
        .load(&mut server.db_conn())
        .expect("Failed to read the provider sign-ins");
    assert_eq!(carried, [None]);

    let resp = finish(
        &server,
        "github",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    let (json, _session) = signed_in(&server, resp, SessionMethod::Github).await;
    let organizations = user_organizations(&server, json.user.uuid);
    assert!(!organizations.contains(&org.uuid));
    assert!(organizations.contains(&OrganizationUuid::from(json.user.uuid)));

    github.received().await;
}

// A provider account pinned to no one whose email has an account does not sign in: it starts a
// sign-in by email for that address, carrying the pin, and the console shows Check your inbox with
// the account's handle. A second account with the same email waits the same way. Proving the code
// pins the account in place of the user's earlier pin and signs in by the provider; then the
// account signs in at once, and the earlier one must prove the email again.
// Kills a known email signed in on the provider's word, a proof that does not pin, one that keeps
// the old pin, and a session recorded as the email's.
#[tokio::test]
async fn second_account_proves_the_email_then_pins_in_place_of_the_first() {
    let github = GitHubServer::start(
        [
            github_account(1, "ada", "ada@example.com"),
            github_account(2, "ada-work", "ada@example.com"),
            github_account(2, "ada-work", "ada@example.com"),
            github_account(2, "ada-work", "ada@example.com"),
            github_account(2, "ada-work", "ada@example.com"),
            github_account(1, "ada", "ada@example.com"),
        ]
        .concat(),
    )
    .await;
    let google = Unscripted::bind().await;
    let Harness {
        server,
        log,
        clock,
        admin,
    } = harness(&github.base(), &google.base()).await;
    let (ada, _session) = sign_in(&server, "github", None).await;
    let sessions_before = sessions(&server);

    for _ in 0..2 {
        let started = start(&server, "github").await;
        let resp = finish(
            &server,
            "github",
            Some(&started.binding),
            &started.state(),
            None,
        )
        .await;
        let (inbox, _pending) = inbox(resp).await;
        assert_eq!(inbox.handle.as_ref(), "ada-work");
        assert_eq!(inbox.email.as_ref(), "ada@example.com");
        assert_eq!(inbox.resend_after, Some(60));
        assert_eq!(sessions(&server), sessions_before);
        assert_eq!(
            pins(&server),
            [(
                ada.user.uuid.to_string(),
                0,
                "1".to_owned(),
                "ada".to_owned()
            )]
        );
        clock.advance(MINUTE);
    }
    let started = start(&server, "github").await;
    let resp = finish(
        &server,
        "github",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    let (_inbox, pending) = inbox(resp).await;
    let (code, _link) = log.sign_in();
    let presented = server.session(&admin);
    let resp = redeem_code_presenting(&server, &pending, &code, &presented).await;
    let (json, session) = signed_in_by_proof(&server, resp, SessionMethod::Github).await;
    assert_eq!(json.user.uuid, ada.user.uuid);
    assert_eq!(session.method, SessionMethod::Github);
    assert_eq!(
        pins(&server),
        [(
            ada.user.uuid.to_string(),
            0,
            "2".to_owned(),
            "ada-work".to_owned()
        )]
    );
    assert_eq!(
        read_session(&server, &presented).await,
        StatusCode::UNAUTHORIZED
    );

    let (again, session) = sign_in(&server, "github", None).await;
    assert_eq!(again.user.uuid, ada.user.uuid);
    assert_eq!(session.method, SessionMethod::Github);
    let started = start(&server, "github").await;
    let resp = finish(
        &server,
        "github",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    inbox(resp).await;

    github.received().await;
}

// A provider account that another user pinned while the address was being proven stays theirs: the
// proof signs in by email and takes no pin.
// Kills a proof that steals a pin, or records its session as the provider's without one.
#[tokio::test]
async fn proof_never_takes_a_pin_from_another_user() {
    let github = GitHubServer::start(github_account(2, "ada-work", "ada@example.com")).await;
    let google = Unscripted::bind().await;
    let Harness { server, log, .. } = harness(&github.base(), &google.base()).await;
    let ada = server.signup("Ada", "ada@example.com").await;
    let grace = server.signup("Grace", "grace@example.com").await;
    let started = start(&server, "github").await;
    let resp = finish(
        &server,
        "github",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    let (_inbox, pending) = inbox(resp).await;

    let grace_id: i32 = schema::user::table
        .filter(schema::user::uuid.eq(grace.uuid))
        .select(schema::user::id)
        .first(&mut server.db_conn())
        .expect("No user");
    diesel::insert_into(schema::provider_pin::table)
        .values((
            schema::provider_pin::user_id.eq(grace_id),
            schema::provider_pin::provider.eq(0),
            schema::provider_pin::account_id.eq("2"),
            schema::provider_pin::handle.eq("ada-work"),
            schema::provider_pin::created.eq(0i64),
        ))
        .execute(&mut server.db_conn())
        .expect("Failed to pin");
    let (code, _link) = log.sign_in();
    let resp = redeem_code(&server, &pending, &code).await;
    let (json, _session) = signed_in_by_proof(&server, resp, SessionMethod::Email).await;
    assert_eq!(json.user.uuid, ada.uuid);
    assert_eq!(
        pins(&server),
        [(
            grace.uuid.to_string(),
            0,
            "2".to_owned(),
            "ada-work".to_owned()
        )]
    );

    github.received().await;
}

// The email a provider's sign-in sends carries an invite only for its own address, which the proof
// then joins; an invite for any other address is dropped, so it can never fail the proof.
// Kills a provider's email that carries an invite for another address into the proof.
#[tokio::test]
async fn provider_email_carries_only_an_invite_for_its_address() {
    let github = GitHubServer::start(
        [
            github_account(2, "ada-work", "ada@example.com"),
            github_account(3, "ada-home", "ada@example.com"),
        ]
        .concat(),
    )
    .await;
    let google = Unscripted::bind().await;
    let Harness {
        server, log, admin, ..
    } = harness(&github.base(), &google.base()).await;
    let ada = server.signup("Ada", "ada@example.com").await;
    let org = server.create_org(&admin, "Invited").await;

    for (email, joins) in [("bob@example.com", false), ("ada@example.com", true)] {
        let invite = server.invite(&admin, &org, email);
        let resp = start_raw(
            &server,
            "github",
            &json!({"invite": invite.as_ref()}),
            &[SAME_ORIGIN],
        )
        .await;
        let started = Started::read(resp).await;
        let resp = finish(
            &server,
            "github",
            Some(&started.binding),
            &started.state(),
            None,
        )
        .await;
        let (_inbox, pending) = inbox(resp).await;
        let (code, _link) = log.sign_in();
        let resp = redeem_code(&server, &pending, &code).await;
        let (json, _session) = signed_in_by_proof(&server, resp, SessionMethod::Github).await;
        assert_eq!(json.user.uuid, ada.uuid, "{email}");
        assert_eq!(
            user_organizations(&server, ada.uuid).contains(&org.uuid),
            joins,
            "{email}"
        );
    }

    github.received().await;
}

// The email a provider's sign-in sends for an address with an account keeps under the address's
// cap: past it, no email goes out and no sign-in starts, and the answer carries the wait.
// Kills a provider's sign-in that emails past the cap, or hides the wait.
#[tokio::test]
async fn known_address_at_its_cap_answers_its_wait() {
    let github = GitHubServer::start(
        (0..3)
            .flat_map(|_| github_account(2, "ada-work", "ada@example.com"))
            .collect(),
    )
    .await;
    let google = Unscripted::bind().await;
    let Harness { server, log, .. } = harness_with(
        &github.base(),
        &google.base(),
        TestServerOptions {
            rate_limiting: Some(RateLimiting::default()),
            ..TestServerOptions::default()
        },
    )
    .await;
    server.signup("Ada", "ada@example.com").await;
    for _ in 0..2 {
        let started = start(&server, "github").await;
        let resp = finish(
            &server,
            "github",
            Some(&started.binding),
            &started.state(),
            None,
        )
        .await;
        inbox(resp).await;
    }
    let pending_before: i64 = schema::pending_sign_in::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count");

    let started = start(&server, "github").await;
    let resp = finish(
        &server,
        "github",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(set_cookies(resp.headers()), [provider_clear()]);
    let json: JsonProviderSignIn = resp.json().await.expect("Invalid answer");
    let inbox = json.inbox.expect("No inbox");
    assert_eq!(inbox.handle.as_ref(), "ada-work");
    assert_eq!(inbox.resend_after, None);
    assert!(
        inbox.wait.is_some_and(|wait| (1..=65).contains(&wait)),
        "{inbox:?}"
    );
    let pending_after: i64 = schema::pending_sign_in::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count");
    assert_eq!(pending_after, pending_before);
    assert_eq!(log.sign_in_emails().len(), 2);

    github.received().await;
}

// Resend of a provider's email starts the sign-in again with the same pin, so the new code still
// pins the account and signs in by the provider.
// Kills a Resend that drops the provider account its sign-in carried.
#[tokio::test]
async fn resend_keeps_the_provider_account_to_pin() {
    let github = GitHubServer::start(github_account(6, "ada-work", "ada@example.com")).await;
    let google = Unscripted::bind().await;
    let Harness {
        server, log, clock, ..
    } = harness(&github.base(), &google.base()).await;
    let ada = server.signup("Ada", "ada@example.com").await;
    let started = start(&server, "github").await;
    let resp = finish(
        &server,
        "github",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    let (_inbox, pending) = inbox(resp).await;

    clock.advance(MINUTE);
    let resp = send(
        &server,
        "/v0/auth/email/resend",
        &[
            SAME_ORIGIN,
            (header::COOKIE.as_str(), &pending_cookie_header(&pending)),
        ],
        &json!({}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let resent = cookie_value(resp.headers(), PendingCookie::new(&console_url()).name())
        .expect("No pending cookie");
    let (code, _link) = log.sign_in();
    let resp = redeem_code(&server, &resent, &code).await;
    let (json, _session) = signed_in_by_proof(&server, resp, SessionMethod::Github).await;
    assert_eq!(json.user.uuid, ada.uuid);
    assert_eq!(
        pins(&server),
        [(
            ada.uuid.to_string(),
            0,
            "6".to_owned(),
            "ada-work".to_owned()
        )]
    );

    github.received().await;
}

// The link in the email that a provider's sign-in sent, opened in the browser that ran that sign-in,
// proves the address as well as its code does: it pins the account and signs in by the provider.
// Kills a link proof that does not pin, or records its session as the email's.
#[tokio::test]
async fn link_proves_the_email_and_pins() {
    let github = Unscripted::bind().await;
    let google = Unscripted::bind().await;
    let Harness {
        server, log, clock, ..
    } = harness(&github.base(), &google.base()).await;
    let grace = server.signup("Grace Hopper", "grace@example.com").await;
    let started = start(&server, "google").await;
    let google = google.script(vec![google_token(&google_claims(
        &query(&started.url, "nonce").expect("No nonce"),
        &clock,
        "1234567890",
        "grace@example.com",
    ))]);

    let resp = finish(
        &server,
        "google",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    let (inbox, pending) = inbox(resp).await;
    assert_eq!(inbox.handle.as_ref(), "grace@example.com");
    let (_code, link) = log.sign_in();
    let resp = send(
        &server,
        "/v0/auth/link",
        &[
            SAME_ORIGIN,
            (header::COOKIE.as_str(), &pending_cookie_header(&pending)),
        ],
        &json!({"secret": link.as_ref(), "confirm": false}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let token = session_from(resp.headers()).expect("No session cookie");
    let session = read_session_json(&server, &token).await;
    assert_eq!(session.user.uuid, grace.uuid);
    assert_eq!(session.method, SessionMethod::Google);
    assert_eq!(
        pins(&server),
        [(
            grace.uuid.to_string(),
            1,
            "1234567890".to_owned(),
            "grace@example.com".to_owned()
        )]
    );

    google.received().await;
}

// The link in the email that a provider's sign-in sent, opened in a browser other than the one that
// ran the provider's sign-in, proves the address alone: once the reader confirms it, the reader
// signs in by email, and no provider account is pinned in place of their own.
// Kills a link confirmed in another browser that pins the provider account, or signs in by it.
#[tokio::test]
async fn link_in_another_browser_signs_in_by_email_and_pins_nothing() {
    let github = GitHubServer::start(
        [
            github_account(5, "victim-gh", "victim@example.com"),
            github_account(77, "attacker-gh", "victim@example.com"),
        ]
        .concat(),
    )
    .await;
    let google = Unscripted::bind().await;
    let Harness { server, log, .. } = harness(&github.base(), &google.base()).await;
    let (victim, _session) = sign_in(&server, "github", None).await;
    let pinned = pins(&server);
    assert_eq!(
        pinned,
        [(
            victim.user.uuid.to_string(),
            0,
            "5".to_owned(),
            "victim-gh".to_owned()
        )]
    );

    let started = start(&server, "github").await;
    let resp = finish(
        &server,
        "github",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    let (inbox, _pending) = inbox(resp).await;
    assert_eq!(inbox.handle.as_ref(), "attacker-gh");
    let (_code, link) = log.sign_in();

    let resp = send(
        &server,
        "/v0/auth/link",
        &[SAME_ORIGIN],
        &json!({"secret": link.as_ref(), "confirm": false}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(session_from(resp.headers()), None);
    let resp = send(
        &server,
        "/v0/auth/link",
        &[SAME_ORIGIN],
        &json!({"secret": link.as_ref(), "confirm": true}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let token = session_from(resp.headers()).expect("No session cookie");
    let session = read_session_json(&server, &token).await;
    assert_eq!(session.user.uuid, victim.user.uuid);
    assert_eq!(session.method, SessionMethod::Email);
    assert_eq!(pins(&server), pinned);

    github.received().await;
}

// The codes of a provider's sign-in count against the address's wrong-code budget, as every code
// for that address does; once it is spent, the right code is refused, and the link still proves
// the address, pinning the account.
// Kills provider sign-ins whose wrong codes the budget does not see.
#[tokio::test]
async fn provider_sign_in_codes_count_against_the_address_budget() {
    let github = GitHubServer::start(
        [
            github_account(9, "ada-work", "ada@example.com"),
            github_account(9, "ada-work", "ada@example.com"),
            github_account(9, "ada-work", "ada@example.com"),
        ]
        .concat(),
    )
    .await;
    let google = Unscripted::bind().await;
    let Harness {
        server, log, clock, ..
    } = harness(&github.base(), &google.base()).await;
    let ada = server.signup("Ada", "ada@example.com").await;

    for _ in 0..2 {
        let started = start(&server, "github").await;
        let resp = finish(
            &server,
            "github",
            Some(&started.binding),
            &started.state(),
            None,
        )
        .await;
        let (_inbox, pending) = inbox(resp).await;
        let (code, _link) = log.sign_in();
        let wrong = wrong_code(&code);
        for _ in 0..5 {
            let resp = redeem_code(&server, &pending, &wrong).await;
            assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        }
        clock.advance(MINUTE);
    }
    let started = start(&server, "github").await;
    let resp = finish(
        &server,
        "github",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    let (_inbox, pending) = inbox(resp).await;
    let (code, link) = log.sign_in();
    let resp = redeem_code(&server, &pending, &code).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(error_code(resp).await, "codes_paused");

    let resp = send(
        &server,
        "/v0/auth/link",
        &[
            SAME_ORIGIN,
            (header::COOKIE.as_str(), &pending_cookie_header(&pending)),
        ],
        &json!({"secret": link.as_ref(), "confirm": false}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let token = session_from(resp.headers()).expect("No session cookie");
    let session = read_session_json(&server, &token).await;
    assert_eq!(session.user.uuid, ada.uuid);
    assert_eq!(session.method, SessionMethod::Github);
    assert_eq!(pins(&server).len(), 1);

    github.received().await;
}

// A locked user's pinned account is refused as locked, and begins no session.
// Kills a pinned sign-in that skips the lock.
#[tokio::test]
async fn locked_users_pinned_account_is_refused() {
    let github = GitHubServer::start(
        [
            github_account(5, "octo-cat", "octo@example.com"),
            github_account(5, "octo-cat", "octo@example.com"),
        ]
        .concat(),
    )
    .await;
    let google = Unscripted::bind().await;
    let Harness { server, .. } = harness(&github.base(), &google.base()).await;
    let (octo, _session) = sign_in(&server, "github", None).await;
    diesel::update(schema::user::table.filter(schema::user::uuid.eq(octo.user.uuid)))
        .set(schema::user::locked.eq(true))
        .execute(&mut server.db_conn())
        .expect("Failed to lock the user");
    let sessions_before = sessions(&server);

    let started = start(&server, "github").await;
    let resp = finish(
        &server,
        "github",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(set_cookies(resp.headers()), [provider_clear()]);
    assert_eq!(error_code(resp).await, "locked");
    assert_eq!(sessions(&server), sessions_before);

    github.received().await;
}

// A GitHub account with no verified primary email is refused, and makes nothing.
// Kills a sign-in that takes an unverified email.
#[tokio::test]
async fn account_without_a_verified_email_is_refused() {
    let github = GitHubServer::start(github_with_emails(
        3,
        "octo-cat",
        r#"[{"email": "octo@example.com", "primary": true, "verified": false, "visibility": null}]"#,
    ))
    .await;
    let google = Unscripted::bind().await;
    let Harness { server, .. } = harness(&github.base(), &google.base()).await;
    let users_before = users(&server);

    let started = start(&server, "github").await;
    let resp = finish(
        &server,
        "github",
        Some(&started.binding),
        &started.state(),
        None,
    )
    .await;
    assert_eq!(set_cookies(resp.headers()), [provider_clear()]);
    assert_refused(resp).await;
    assert_eq!(users(&server), users_before);
    assert_eq!(sessions(&server), 0);

    github.received().await;
}

// A Google sign-in joins the organization whose SSO domain is its ID token's hosted domain, and
// only that: a consumer Google account made on a company's address joins nothing, and GitHub never
// joins.
// Kills a Google sign-in that joins by its email's domain, and a GitHub sign-in that joins at all.
#[tokio::test]
async fn google_joins_single_sign_on_only_by_its_hosted_domain() {
    let github = GitHubServer::start(github_account(8, "bob", "bob@example.com")).await;
    let google = Unscripted::bind().await;
    let Harness {
        server,
        clock,
        admin,
        ..
    } = harness(&github.base(), &google.base()).await;
    let org = server.create_org(&admin, "Example").await;
    sso_domain(&server, org.uuid, "example.com");

    let consumer = start(&server, "google").await;
    let workspace = start(&server, "google").await;
    let nonce = |started: &Started| query(&started.url, "nonce").expect("No nonce");
    let mut hosted = google_claims(&nonce(&workspace), &clock, "2", "grace@example.com");
    hosted["hd"] = json!("example.com");
    let google = google.script(vec![
        google_token(&google_claims(
            &nonce(&consumer),
            &clock,
            "1",
            "ada@example.com",
        )),
        google_token(&hosted),
    ]);

    let resp = finish(
        &server,
        "google",
        Some(&consumer.binding),
        &consumer.state(),
        None,
    )
    .await;
    let (ada, _session) = signed_in(&server, resp, SessionMethod::Google).await;
    assert!(!user_organizations(&server, ada.user.uuid).contains(&org.uuid));
    let resp = finish(
        &server,
        "google",
        Some(&workspace.binding),
        &workspace.state(),
        None,
    )
    .await;
    let (grace, _session) = signed_in(&server, resp, SessionMethod::Google).await;
    assert!(user_organizations(&server, grace.user.uuid).contains(&org.uuid));
    let (bob, _session) = sign_in(&server, "github", None).await;
    assert!(!user_organizations(&server, bob.user.uuid).contains(&org.uuid));

    github.received().await;
    google.received().await;
}

// No answer carries a state, the PKCE verifier, the binding secret, the provider's token, or a
// session token, but the provider's URL its own state and each cookie its own secret; and the row
// keeps the state and the binding secret only as hashes.
// Kills a start or a finish that echoes a secret, and a row that keeps one in plain.
#[tokio::test]
async fn no_answer_carries_a_secret() {
    let github = GitHubServer::start(
        [
            github_account(1, "octo-cat", "octo@example.com"),
            github_account(2, "octo-two", "octo@example.com"),
            github_account(3, "mona", "mona@example.com"),
        ]
        .concat(),
    )
    .await;
    let google = Unscripted::bind().await;
    let Harness { server, .. } = harness(&github.base(), &google.base()).await;
    let mut answers = Vec::new();
    let mut secrets = Vec::new();

    for _ in 0..3 {
        let resp = start_raw(&server, "github", &json!({}), &[SAME_ORIGIN]).await;
        let (headers, body) = answer(resp).await;
        let JsonOAuthUrl { url } = serde_json::from_str(&body).expect("Invalid answer");
        let state = query(&url, "state").expect("No state");
        let binding = provider_cookie(&headers).expect("No provider cookie");
        let (state_hash, binding_hash): (String, String) = schema::provider_state::table
            .select((
                schema::provider_state::state_hash,
                schema::provider_state::binding_hash,
            ))
            .order(schema::provider_state::id.desc())
            .first(&mut server.db_conn())
            .expect("No sign-in");
        let parsed_state: ProviderState = state.parse().expect("Invalid state");
        let parsed_binding: BindingSecret = binding.parse().expect("Invalid binding");
        assert_eq!(
            state_hash,
            ProviderStateHash::from(&parsed_state).to_string()
        );
        assert_eq!(
            binding_hash,
            BindingSecretHash::from(&parsed_binding).to_string()
        );
        answers.push((headers, body.replace(&state, "")));
        let resp = finish(&server, "github", Some(&binding), &state, None).await;
        answers.push(answer(resp).await);
        secrets.extend([state, binding]);
    }
    let resp = finish(&server, "github", Some(&secrets[1]), &secrets[0], None).await;
    answers.push(answer(resp).await);

    let verifiers = github
        .received()
        .await
        .iter()
        .filter_map(|request| request.form().get("code_verifier").cloned())
        .collect::<Vec<_>>();
    assert_eq!(verifiers.len(), 3);
    let session_tokens = answers
        .iter()
        .filter_map(|(headers, _)| session_from(headers))
        .map(|token| token.as_ref().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(session_tokens.len(), 2);
    let kept_cookies = [
        SessionCookie::Host.name(),
        PendingCookie::new(&console_url()).name(),
        ProviderCookie::new(&console_url()).name(),
    ];
    for (headers, body) in &answers {
        let mut seen = headers
            .iter()
            .filter(|(name, _)| *name != header::SET_COOKIE)
            .map(|(_, value)| value.to_str().unwrap_or_default().to_owned())
            .collect::<Vec<_>>()
            .join("\n");
        seen.push_str(body);
        for cookie in set_cookies(headers) {
            if !kept_cookies.contains(&cookie.split_once('=').map_or("", |(name, _)| name)) {
                seen.push_str(&cookie);
            }
        }
        for secret in secrets
            .iter()
            .chain(&verifiers)
            .chain(&session_tokens)
            .map(String::as_str)
            .chain([GITHUB_TOKEN, CODE])
        {
            assert!(!seen.contains(secret), "{seen}");
        }
    }
}

// Each provider endpoint takes a request from the console's own origin alone, before it asks the
// provider anything, and charges the reader's address against its attempts.
// Kills a start or a finish from another site, and one that is not charged.
#[tokio::test]
async fn provider_endpoints_need_the_console_origin_and_charge_the_reader() {
    let github = GitHubServer::start(Vec::new()).await;
    let google = Unscripted::bind().await;
    let Harness { server, .. } = harness(&github.base(), &google.base()).await;
    let started = start(&server, "github").await;
    for site in [None, Some("cross-site"), Some("same-site")] {
        let headers: Vec<(&str, &str)> = site
            .map(|site| ("Sec-Fetch-Site", site))
            .into_iter()
            .collect();
        let resp = start_raw(&server, "github", &json!({}), &headers).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{site:?}");
        assert_eq!(error_code(resp).await, "cross_origin");
        let mut headers = headers;
        let cookie = provider_cookie_header(&started.binding);
        headers.push((header::COOKIE.as_str(), &cookie));
        let resp = send(
            &server,
            "/v0/auth/providers/github/finish",
            &headers,
            &json!({"code": CODE, "state": started.state()}),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{site:?}");
        assert_eq!(error_code(resp).await, "cross_origin");
    }
    assert!(github.received().await.is_empty());

    let rate_limiting: JsonRateLimiting =
        serde_json::from_value(json!({"public": {"attempts": {"minute": 2}}}))
            .expect("Invalid rate limiting");
    // GitHub answers nothing, so a finish that should have been refused fails at once.
    let github = GitHubServer::start(Vec::new()).await;
    let google = Unscripted::bind().await;
    let Harness { server, .. } = harness_with(
        &github.base(),
        &google.base(),
        TestServerOptions {
            relay_secret: Some(RELAY_SECRET.parse().expect("Invalid secret")),
            rate_limiting: Some(RateLimiting::from(rate_limiting)),
            ..TestServerOptions::default()
        },
    )
    .await;
    let reader = relayed("203.0.113.7");
    let started = Started::read(start_raw(&server, "github", &json!({}), &reader).await).await;
    let finish_body = json!({"code": CODE, "state": started.state()});
    let finish_path = "/v0/auth/providers/github/finish";
    let resp = send(&server, finish_path, &reader, &finish_body).await;
    assert_refused(resp).await;
    let resp = start_raw(&server, "github", &json!({}), &reader).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(error_code(resp).await, "too_many_attempts");
    let cookie = provider_cookie_header(&started.binding);
    let with_cookie = [&reader[..], &[(header::COOKIE.as_str(), cookie.as_str())]].concat();
    let resp = send(&server, finish_path, &with_cookie, &finish_body).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(error_code(resp).await, "too_many_attempts");
    let resp = start_raw(&server, "github", &json!({}), &relayed("203.0.113.8")).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

// The start checks what the sign-in carries before it makes anything: an invitation must be one a
// row holds, and a claim's organization must exist.
// Kills a start that keeps an invalid invite or an unknown organization for the finish.
#[tokio::test]
async fn start_checks_what_the_sign_in_carries() {
    let github = Unscripted::bind().await;
    let google = Unscripted::bind().await;
    let Harness { server, .. } = harness(&github.base(), &google.base()).await;
    for (body, status, code) in [
        (
            json!({"invite": bencher_json::InviteSecret::generate().expect("Failed to make a secret").as_ref()}),
            StatusCode::BAD_REQUEST,
            "invalid_invite",
        ),
        (
            json!({"claim": OrganizationUuid::new()}),
            StatusCode::NOT_FOUND,
            "unknown_organization",
        ),
    ] {
        let resp = start_raw(&server, "github", &body, &[SAME_ORIGIN]).await;
        assert_eq!(resp.status(), status, "{body}");
        assert_eq!(set_cookies(resp.headers()), Vec::<String>::new());
        assert_eq!(error_code(resp).await, code);
    }
    let started: i64 = schema::provider_state::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count");
    assert_eq!(started, 0);
}

// A server without a provider's client, or a self-hosted one without an Enterprise license, has no
// sign-in with that provider; and the old routes are gone.
// Kills a provider sign-in on a server that may not offer it.
#[tokio::test]
async fn provider_needs_its_client_and_a_license() {
    let server = TestServer::new_with_options(TestServerOptions {
        github_client: Some(github_client()),
        google_client: Some(google_client()),
        ..TestServerOptions::default()
    })
    .await;
    let bare = TestServer::new().await;
    for (server, provider) in [
        (&server, "github"),
        (&server, "google"),
        (&bare, "github"),
        (&bare, "google"),
    ] {
        let resp = start_raw(server, provider, &json!({}), &[SAME_ORIGIN]).await;
        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED, "{provider}");
    }
    for provider in ["github", "google"] {
        for method in [http::Method::GET, http::Method::POST] {
            let resp = server
                .client
                .request(
                    method.clone(),
                    server.api_url(&format!("/v0/auth/{provider}")),
                )
                .header(SAME_ORIGIN.0, SAME_ORIGIN.1)
                .json(&json!({"code": CODE, "state": "state"}))
                .send()
                .await
                .expect("Request failed");
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{method} {provider}");
        }
    }
}

/// A clock the test moves, from now.
#[derive(Clone)]
struct TestClock(Arc<AtomicI64>);

impl TestClock {
    fn new() -> Self {
        Self(Arc::new(AtomicI64::new(DateTime::now().timestamp())))
    }

    fn clock(&self) -> Clock {
        let now = Arc::clone(&self.0);
        Clock::Custom(Arc::new(move || {
            DateTime::try_from(now.load(Ordering::SeqCst)).expect("Invalid time")
        }))
    }

    fn now(&self) -> DateTime {
        DateTime::try_from(self.0.load(Ordering::SeqCst)).expect("Invalid time")
    }

    fn advance(&self, seconds: i64) {
        self.0.fetch_add(seconds, Ordering::SeqCst);
    }
}

/// A server whose GitHub and Google are at `github` and `google`, licensed to sign in with them by
/// its first user, with its emails in its log and its clock in the test's hands.
struct Harness {
    server: TestServer,
    log: LogCapture,
    clock: TestClock,
    admin: TestUser,
}

async fn harness(github: &Url, google: &Url) -> Harness {
    harness_with(github, google, TestServerOptions::default()).await
}

async fn harness_with(github: &Url, google: &Url, options: TestServerOptions) -> Harness {
    let log = LogCapture::default();
    let clock = TestClock::new();
    let server = TestServer::new_with_options(TestServerOptions {
        clock: Some(clock.clock()),
        log: Some(log.logger()),
        github_client: Some(github_client().with_base_url(github.clone())),
        google_client: Some(google_client().with_base_url(google)),
        ..options
    })
    .await;
    let admin = server.signup("Admin User", "admin@license.example").await;
    let org = server.create_org(&admin, "Licensed").await;
    server
        .license_org(&admin, &org, PlanLevel::Enterprise)
        .await;
    Harness {
        server,
        log,
        clock,
        admin,
    }
}

/// A started sign-in: the provider's URL, and the binding secret its cookie holds.
struct Started {
    url: Url,
    binding: String,
}

impl Started {
    async fn read(resp: reqwest::Response) -> Self {
        assert_eq!(resp.status(), StatusCode::OK);
        let binding = provider_cookie(resp.headers()).expect("No provider cookie");
        let JsonOAuthUrl { url } = resp.json().await.expect("Invalid answer");
        Self { url, binding }
    }

    fn state(&self) -> String {
        query(&self.url, "state").expect("No state")
    }
}

async fn start(server: &TestServer, provider: &str) -> Started {
    Started::read(start_raw(server, provider, &json!({}), &[SAME_ORIGIN]).await).await
}

async fn start_raw(
    server: &TestServer,
    provider: &str,
    body: &Value,
    headers: &[(&str, &str)],
) -> reqwest::Response {
    send(
        server,
        &format!("/v0/auth/providers/{provider}/start"),
        headers,
        body,
    )
    .await
}

/// Finish a sign-in from the console, in the browser holding `binding`, presenting `session`.
async fn finish(
    server: &TestServer,
    provider: &str,
    binding: Option<&str>,
    state: &str,
    session: Option<&SessionToken>,
) -> reqwest::Response {
    finish_request(server, provider, binding, state, session)
        .send()
        .await
        .expect("Request failed")
}

fn finish_request(
    server: &TestServer,
    provider: &str,
    binding: Option<&str>,
    state: &str,
    session: Option<&SessionToken>,
) -> reqwest::RequestBuilder {
    let cookies = binding
        .map(provider_cookie_header)
        .into_iter()
        .chain(session.map(session_cookie))
        .collect::<Vec<_>>()
        .join("; ");
    let mut headers = vec![SAME_ORIGIN];
    if !cookies.is_empty() {
        headers.push((header::COOKIE.as_str(), &cookies));
    }
    request(
        server,
        &format!("/v0/auth/providers/{provider}/finish"),
        &headers,
        &json!({"code": CODE, "state": state}),
    )
}

/// A whole sign-in with `provider`, presenting `session`, that signs in.
async fn sign_in(
    server: &TestServer,
    provider: &str,
    session: Option<&SessionToken>,
) -> (JsonSignedIn, JsonSession) {
    let started = start(server, provider).await;
    let resp = finish(
        server,
        provider,
        Some(&started.binding),
        &started.state(),
        session,
    )
    .await;
    let method = if provider == "github" {
        SessionMethod::Github
    } else {
        SessionMethod::Google
    };
    signed_in(server, resp, method).await
}

async fn send(
    server: &TestServer,
    path: &str,
    headers: &[(&str, &str)],
    body: &Value,
) -> reqwest::Response {
    request(server, path, headers, body)
        .send()
        .await
        .expect("Request failed")
}

fn request(
    server: &TestServer,
    path: &str,
    headers: &[(&str, &str)],
    body: &Value,
) -> reqwest::RequestBuilder {
    let mut request = server.client.post(server.api_url(path)).json(body);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    request
}

/// Redeem `code` in the browser holding `pending` and signed in to `session`.
async fn redeem_code_presenting(
    server: &TestServer,
    pending: &str,
    code: &SignInCode,
    session: &SessionToken,
) -> reqwest::Response {
    let cookies = format!(
        "{}; {}",
        pending_cookie_header(pending),
        session_cookie(session)
    );
    send(
        server,
        "/v0/auth/code",
        &[SAME_ORIGIN, (header::COOKIE.as_str(), &cookies)],
        &json!({"code": code.as_ref()}),
    )
    .await
}

async fn redeem_code(server: &TestServer, pending: &str, code: &SignInCode) -> reqwest::Response {
    send(
        server,
        "/v0/auth/code",
        &[
            SAME_ORIGIN,
            (header::COOKIE.as_str(), &pending_cookie_header(pending)),
        ],
        &json!({"code": code.as_ref()}),
    )
    .await
}

/// Assert that a finish signed in by `method`: its session cookie reads a session of the user it
/// names, and the provider's cookie is cleared.
async fn signed_in(
    server: &TestServer,
    resp: reqwest::Response,
    method: SessionMethod,
) -> (JsonSignedIn, JsonSession) {
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(set_cookies(resp.headers()).contains(&provider_clear()));
    let token = session_from(resp.headers()).expect("No session cookie");
    let json: JsonProviderSignIn = resp.json().await.expect("Invalid answer");
    assert!(json.inbox.is_none());
    let signed_in = json.signed_in.expect("Not signed in");
    let session = read_session_json(server, &token).await;
    assert_eq!(session.user.uuid, signed_in.user.uuid);
    assert_eq!(session.method, method);
    assert_proved_now(server, &token);
    (signed_in, session)
}

/// Assert that a proof by code signed in by `method`.
async fn signed_in_by_proof(
    server: &TestServer,
    resp: reqwest::Response,
    method: SessionMethod,
) -> (JsonSignedIn, JsonSession) {
    assert_eq!(resp.status(), StatusCode::OK);
    let token = session_from(resp.headers()).expect("No session cookie");
    let json: JsonSignedIn = resp.json().await.expect("Invalid answer");
    let session = read_session_json(server, &token).await;
    assert_eq!(session.user.uuid, json.user.uuid);
    assert_eq!(session.method, method);
    assert_proved_now(server, &token);
    (json, session)
}

/// Assert that the session records its reader as proved when it began.
fn assert_proved_now(server: &TestServer, token: &SessionToken) {
    let (created, proved): (i64, i64) = schema::session::table
        .filter(schema::session::token_hash.eq(SessionTokenHash::from(token)))
        .select((schema::session::created, schema::session::proved))
        .first(&mut server.db_conn())
        .expect("No session");
    assert_eq!(proved, created);
}

/// Assert that a finish asked to prove the address first: no session, the provider's cookie
/// cleared, and a pending sign-in's cookie, returned with what the answer says.
async fn inbox(resp: reqwest::Response) -> (JsonProviderInbox, String) {
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(session_from(resp.headers()), None);
    assert!(set_cookies(resp.headers()).contains(&provider_clear()));
    let pending = cookie_value(resp.headers(), PendingCookie::new(&console_url()).name())
        .expect("No pending cookie");
    let json: JsonProviderSignIn = resp.json().await.expect("Invalid answer");
    assert!(json.signed_in.is_none());
    (json.inbox.expect("No inbox"), pending)
}

async fn assert_refused(resp: reqwest::Response) {
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(session_from(resp.headers()), None);
    assert_eq!(error_code(resp).await, "provider_refused");
}

async fn error_code(resp: reqwest::Response) -> String {
    let json: Value = resp.json().await.expect("Invalid answer");
    json.get("error_code")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

async fn answer(resp: reqwest::Response) -> (HeaderMap, String) {
    let headers = resp.headers().clone();
    (headers, resp.text().await.expect("No body"))
}

async fn read_session(server: &TestServer, token: &SessionToken) -> StatusCode {
    server
        .client
        .get(server.api_url("/v0/auth/session"))
        .header(header::COOKIE, session_cookie(token))
        .send()
        .await
        .expect("Request failed")
        .status()
}

async fn read_session_json(server: &TestServer, token: &SessionToken) -> JsonSession {
    let resp = server
        .client
        .get(server.api_url("/v0/auth/session"))
        .header(header::COOKIE, session_cookie(token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    resp.json().await.expect("Invalid session")
}

/// The headers of a request from the console that the relay forwards for the reader at `ip`.
fn relayed(ip: &'static str) -> [(&'static str, &'static str); 3] {
    [
        SAME_ORIGIN,
        (bencher_json::RELAY_SECRET, RELAY_SECRET),
        (bencher_json::RELAY_CLIENT_IP, ip),
    ]
}

fn console_url() -> Url {
    "http://localhost:3000".parse().expect("Invalid URL")
}

fn query(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find_map(|(k, v)| (k == key).then(|| v.into_owned()))
}

fn set_cookies(headers: &HeaderMap) -> Vec<String> {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().expect("Invalid header").to_owned())
        .collect()
}

/// The value a named cookie is set to, unless it is cleared.
fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    set_cookies(headers).into_iter().find_map(|cookie| {
        let (value, attributes) = cookie.strip_prefix(&format!("{name}="))?.split_once(';')?;
        (!attributes.contains("Max-Age=0")).then(|| value.to_owned())
    })
}

fn provider_cookie(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, ProviderCookie::new(&console_url()).name())
}

fn session_from(headers: &HeaderMap) -> Option<SessionToken> {
    cookie_value(headers, SessionCookie::Host.name())
        .map(|token| token.parse().expect("Invalid session token"))
}

fn provider_clear() -> String {
    ProviderCookie::new(&console_url())
        .clear()
        .to_str()
        .expect("Invalid header")
        .to_owned()
}

fn provider_cookie_header(binding: &str) -> String {
    format!("{}={binding}", ProviderCookie::new(&console_url()).name())
}

fn pending_cookie_header(pending: &str) -> String {
    format!("{}={pending}", PendingCookie::new(&console_url()).name())
}

/// A code other than `code`.
fn wrong_code(code: &SignInCode) -> SignInCode {
    let wrong = if code.as_ref() == "000000" {
        "000001"
    } else {
        "000000"
    };
    wrong.parse().expect("Invalid code")
}

/// Every pin: its user's UUID, its provider, its account, and its handle.
fn pins(server: &TestServer) -> Vec<(String, i32, String, String)> {
    schema::provider_pin::table
        .inner_join(schema::user::table)
        .select((
            schema::user::uuid,
            schema::provider_pin::provider,
            schema::provider_pin::account_id,
            schema::provider_pin::handle,
        ))
        .order(schema::provider_pin::id)
        .load(&mut server.db_conn())
        .expect("Failed to load pins")
}

fn users(server: &TestServer) -> i64 {
    schema::user::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count")
}

fn sessions(server: &TestServer) -> i64 {
    schema::session::table
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count")
}

fn email_verified(server: &TestServer, email: &str) -> bool {
    schema::user::table
        .filter(schema::user::email.eq(email))
        .select(schema::user::email_verified)
        .first::<Option<i64>>(&mut server.db_conn())
        .expect("No user")
        .is_some()
}

fn sso_domain(server: &TestServer, org: OrganizationUuid, domain: &str) {
    let org_id: i32 = schema::organization::table
        .filter(schema::organization::uuid.eq(org))
        .select(schema::organization::id)
        .first(&mut server.db_conn())
        .expect("No organization");
    diesel::insert_into(schema::sso::table)
        .values((
            schema::sso::uuid.eq("00000000-0000-4000-8000-0000000055a0"),
            schema::sso::organization_id.eq(org_id),
            schema::sso::domain.eq(domain),
            schema::sso::created.eq(0i64),
        ))
        .execute(&mut server.db_conn())
        .expect("Failed to add an SSO domain");
}

/// GitHub's answers to one sign-in of the account `id`, with no name.
fn github_account(id: u64, login: &str, email: &str) -> Vec<(u16, String)> {
    github_with_emails(
        id,
        login,
        &format!(
            r#"[{{"email": "{email}", "primary": true, "verified": true, "visibility": null}}]"#
        ),
    )
}

/// GitHub's answers to one sign-in of the account `id`, named `name`.
fn github_named(id: u64, login: &str, name: &str, email: &str) -> Vec<(u16, String)> {
    vec![
        github_exchange(),
        (
            200,
            json!({"id": id, "login": login, "name": name}).to_string(),
        ),
        (
            200,
            format!(
                r#"[{{"email": "{email}", "primary": true, "verified": true, "visibility": null}}]"#
            ),
        ),
    ]
}

/// GitHub's answers to one sign-in of the account `id`, whose emails are `emails`.
fn github_with_emails(id: u64, login: &str, emails: &str) -> Vec<(u16, String)> {
    vec![
        github_exchange(),
        (
            200,
            json!({"id": id, "login": login, "name": null}).to_string(),
        ),
        (200, emails.to_owned()),
    ]
}

fn github_exchange() -> (u16, String) {
    (
        200,
        json!({"access_token": GITHUB_TOKEN, "token_type": "bearer", "scope": ""}).to_string(),
    )
}

/// The claims of a Google ID token for the sign-in whose nonce is `nonce`, valid for an hour by
/// the server's clock.
fn google_claims(nonce: &str, clock: &TestClock, sub: &str, email: &str) -> Value {
    json!({
        "iss": "https://accounts.google.com",
        "aud": GOOGLE_CLIENT_ID,
        "exp": clock.now().timestamp() + 60 * MINUTE,
        "nonce": nonce,
        "sub": sub,
        "email": email,
        "email_verified": true,
        "name": "Ada Lovelace",
    })
}

/// Google's token endpoint answer with an ID token of `claims`.
fn google_token(claims: &Value) -> (u16, String) {
    let segment = |value: &Value| URL_SAFE_NO_PAD.encode(value.to_string());
    let id_token = format!(
        "{}.{}.c2lnbmF0dXJl",
        segment(&json!({"alg": "RS256", "typ": "JWT"})),
        segment(claims)
    );
    (
        200,
        json!({
            "access_token": GOOGLE_TOKEN,
            "token_type": "Bearer",
            "expires_in": 3599,
            "scope": "openid email profile",
            "id_token": id_token,
        })
        .to_string(),
    )
}
