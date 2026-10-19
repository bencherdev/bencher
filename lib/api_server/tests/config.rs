#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for server config endpoints.

use bencher_api_tests::TestServer;
use bencher_json::system::config::JsonConsole;
use http::StatusCode;

/// What a secret reads as once the server has redacted it.
const REDACTED: &str = "************";

// GET /v0/server/config/console - public endpoint
#[tokio::test]
async fn config_console_get() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/config/console"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let console: JsonConsole = resp.json().await.expect("Failed to parse response");
    // Console URL should be set
    assert_ne!(console.url.as_ref(), "");
}

// GET /v0/server/config - requires admin auth
#[tokio::test]
async fn config_get_requires_auth() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/config"))
        .send()
        .await
        .expect("Request failed");

    // Should require authentication
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// GET /v0/server/config - admin can view config, with its secrets redacted
// Kills a config served as read, which hands every admin credential the key that signs sessions.
#[tokio::test]
async fn config_get_as_admin() {
    let server = TestServer::new().await;
    // First user is admin
    let admin = server.signup("Admin User", "configadmin@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/config"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");

    // Admin should be able to view config
    assert_eq!(resp.status(), StatusCode::OK);
    let config: serde_json::Value = resp.json().await.expect("Failed to parse response");
    assert_eq!(config["security"]["secret_key"], REDACTED);
}

// Kills a config served with the key that signs ID tokens, which would let any admin credential
// forge them.
#[cfg(feature = "plus")]
#[tokio::test]
async fn config_get_redacts_the_signing_key() {
    let path = std::env::temp_dir().join(format!("bencher-config-{}.json", std::process::id()));
    std::fs::write(
        &path,
        serde_json::json!({
            "console": {"url": "http://localhost:3000"},
            "security": {"issuer": "http://localhost:3000/", "secret_key": "not-the-default-secret-key"},
            "server": {"bind_address": "127.0.0.1:61016", "request_body_max_bytes": 1 << 22},
            "logging": {"name": "Bencher API", "log": {"stderr_terminal": {"level": "info"}}},
            "database": {"file": "data/bencher.db"},
            "plus": {"oauth": {
                "issuer": "https://api.example.com",
                "signing_key": bencher_api_tests::helpers::OAUTH_SIGNING_KEY,
            }},
        })
        .to_string(),
    )
    .expect("Failed to write the config file");
    #[expect(
        unsafe_code,
        reason = "the endpoint reads its config file path from the environment"
    )]
    // SAFETY: nextest runs every test in its own process, so no other thread reads the environment.
    unsafe {
        std::env::set_var(bencher_config::BENCHER_CONFIG_PATH, &path);
    }
    let server = TestServer::new().await;
    let admin = server
        .signup("Admin User", "signingadmin@example.com")
        .await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/config"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");
    let _removed = std::fs::remove_file(&path);

    assert_eq!(resp.status(), StatusCode::OK);
    let config: serde_json::Value = resp.json().await.expect("Failed to parse response");
    assert_eq!(config["security"]["secret_key"], REDACTED);
    assert_eq!(config["plus"]["oauth"]["signing_key"], REDACTED);
}

// GET /v0/server/config - non-admin cannot view config
#[tokio::test]
async fn config_get_forbidden_for_non_admin() {
    let server = TestServer::new().await;
    // First user is admin
    let _admin = server.signup("Admin User", "admin@example.com").await;
    // Second user is NOT admin
    let user = server.signup("Regular User", "regular@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/config"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    // Non-admin should be forbidden
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
