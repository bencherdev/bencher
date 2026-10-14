#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for OAuth access tokens at the MCP endpoint (POST /mcp).

use std::sync::Arc;

use bencher_api_tests::{
    TestProject, TestServer, TestServerOptions, TestUser,
    helpers::{TestOAuthGrant, oauth_resource, oauth_server, seed_oauth_grant},
};
use bencher_json::{
    Clock, DateTime, JsonUserKeyCreated, OAuthAccessToken, OAuthResource, UserKeyUuid,
    project::Visibility,
    system::config::{JsonOAuthServer, JsonRateLimiting},
};
use bencher_schema::{
    context::{OAuthServer, RateLimiting},
    model::user::oauth::{OAuthGrantId, OAuthTokenKind},
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;
use reqwest::{
    Url,
    header::{
        ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS, HOST, HeaderMap, HeaderName,
        USER_AGENT, WWW_AUTHENTICATE,
    },
};
use serde_json::{Value, json};

/// The time every test server here is frozen at.
const NOW: i64 = 1_800_000_000;
/// Where every challenge from a test server here points: the metadata of its only resource.
const RESOURCE_METADATA: &str = "https://api.example.com/.well-known/oauth-protected-resource/mcp";
const REMOTE_IP: &str = "203.0.113.7";

// An access token acts as its user, and a write scope also reads
#[tokio::test]
async fn oauth_access_token_lists_projects() {
    let server = server_with(Some(oauth_server())).await;
    let user = server.signup("OAuth User", "oauth-user@example.com").await;
    let org = server.create_org(&user, "OAuth Org").await;
    let project = server.create_project(&user, &org, "OAuth Project").await;

    for scopes in ["projects:read", "projects:write"] {
        let grant = grant(&server, &user, scopes);
        let content = tool_content(
            call_tool(
                &server,
                grant.access_token.as_ref(),
                "list_projects",
                &json!({}),
            )
            .await,
        )
        .await;
        let slug: &str = project.slug.as_ref();
        assert!(
            content["data"]
                .as_array()
                .expect("Missing data array")
                .iter()
                .any(|p| p["slug"] == slug),
            "Missing project with {scopes}: {content}"
        );
    }
}

// A read-only token is refused a write tool with a challenge for every scope, before the tool runs
#[tokio::test]
async fn oauth_read_scope_cannot_write() {
    let server = server_with(Some(oauth_server())).await;
    let user = server
        .signup("OAuth Reader", "oauth-reader@example.com")
        .await;
    let org = server.create_org(&user, "OAuth Reader Org").await;
    let project = server
        .create_project(&user, &org, "OAuth Reader Project")
        .await;
    let slug: &str = project.slug.as_ref();
    let create_testbed = json!({"project": slug, "name": "OAuth Testbed"});

    let read = grant(&server, &user, "projects:read");
    let resp = call_tool(
        &server,
        read.access_token.as_ref(),
        "create_testbed",
        &create_testbed,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        header(resp.headers(), &WWW_AUTHENTICATE),
        format!(
            r#"Bearer error="insufficient_scope", scope="projects:read projects:write", resource_metadata="{RESOURCE_METADATA}""#
        )
    );
    assert_challenge_exposed(resp.headers());
    let body: Value = resp.json().await.expect("Invalid error body");
    assert_eq!(body["error"], "insufficient_scope");

    let testbeds = tool_content(
        call_tool(
            &server,
            read.access_token.as_ref(),
            "list_testbeds",
            &json!({"project": slug}),
        )
        .await,
    )
    .await;
    assert!(
        !testbeds["data"]
            .as_array()
            .expect("Missing data array")
            .iter()
            .any(|testbed| testbed["name"] == "OAuth Testbed"),
        "The refused tool ran: {testbeds}"
    );

    let write = grant(&server, &user, "projects:write");
    let testbed = tool_content(
        call_tool(
            &server,
            write.access_token.as_ref(),
            "create_testbed",
            &create_testbed,
        )
        .await,
    )
    .await;
    assert_eq!(testbed["name"], "OAuth Testbed");
}

// An admin's OAuth token carries none of the admin's server authority
#[tokio::test]
async fn oauth_token_strips_server_admin() {
    let server = server_with(Some(oauth_server())).await;
    // The first user to sign up is the server admin
    let admin = server
        .signup("OAuth Admin", "oauth-admin@example.com")
        .await;
    let other = server
        .signup("OAuth Other", "oauth-other@example.com")
        .await;
    let org = server.create_org(&other, "OAuth Other Org").await;
    let project = server
        .create_project(&other, &org, "OAuth Private Project")
        .await;
    make_private(&server, &project);
    let view_project = json!({"project": project.slug});

    let content =
        tool_content(call_tool(&server, &admin.token, "view_project", &view_project).await).await;
    assert_eq!(content["uuid"], json!(project.uuid));

    let grant = grant(&server, &admin, "projects:write");
    let error = tool_error(
        call_tool(
            &server,
            grant.access_token.as_ref(),
            "view_project",
            &view_project,
        )
        .await,
    )
    .await;
    assert!(error.starts_with("404"), "Expected 404 error: {error}");
}

// A token that matches nothing is challenged, not served as anonymous,
// and with OAuth on so is any other refused credential
#[tokio::test]
async fn oauth_unknown_token() {
    let server = server_with(Some(oauth_server())).await;

    assert_refused(&server, OAuthAccessToken::generate().as_ref()).await;
    assert_refused(&server, "bencher_oauth_short").await;
    assert_refused(&server, "bencher_run_invalid").await;
}

// A refresh token shares the token table but is never a bearer credential
#[tokio::test]
async fn oauth_refresh_token_is_not_a_bearer() {
    let server = server_with(Some(oauth_server())).await;
    let user = server
        .signup("OAuth Refresh", "oauth-refresh@example.com")
        .await;
    let grant = grant(&server, &user, "projects:read");

    assert_accepted(&server, grant.access_token.as_ref()).await;
    assert_refused(&server, grant.refresh_token.as_ref()).await;
}

// An access token is refused from the moment it expires
#[tokio::test]
async fn oauth_expired_access_token() {
    let server = server_with(Some(oauth_server())).await;
    let user = server
        .signup("OAuth Expired", "oauth-expired@example.com")
        .await;
    let grant = grant(&server, &user, "projects:read");
    assert_accepted(&server, grant.access_token.as_ref()).await;

    let mut conn = server.db_conn();
    let grant_id = grant_id(&mut conn, &grant);
    diesel::update(
        schema::oauth_token::table
            .filter(schema::oauth_token::grant_id.eq(grant_id))
            .filter(schema::oauth_token::kind.eq(OAuthTokenKind::Access)),
    )
    .set(schema::oauth_token::expiration.eq(now()))
    .execute(&mut conn)
    .expect("Failed to expire the access token");

    assert_refused(&server, grant.access_token.as_ref()).await;
}

// A refused token is challenged with the metadata of the MCP server the client reached, by its
// `Host`, where the client finds how to get a new access token.
#[tokio::test]
async fn oauth_challenge_points_at_resource_metadata() {
    let json: JsonOAuthServer = serde_json::from_value(json!({
        "issuer": "https://api.example.com",
        "resources": ["https://api.example.com/mcp", "https://mcp.example.com/mcp"],
    }))
    .expect("Invalid OAuth config");
    let server = server_with(Some(
        OAuthServer::try_from(json).expect("Invalid OAuth server"),
    ))
    .await;
    let user = server
        .signup("OAuth Challenged", "oauth-challenged@example.com")
        .await;
    let grant = grant(&server, &user, "projects:read");
    let mut conn = server.db_conn();
    diesel::update(
        schema::oauth_token::table
            .filter(schema::oauth_token::grant_id.eq(grant_id(&mut conn, &grant))),
    )
    .set(schema::oauth_token::expiration.eq(now()))
    .execute(&mut conn)
    .expect("Failed to expire the tokens");

    for (host, resource) in [
        ("mcp.example.com", "https://mcp.example.com/mcp"),
        ("api.example.com", "https://api.example.com/mcp"),
        ("unknown.example", "https://api.example.com/mcp"),
    ] {
        let resp = server
            .client
            .post(server.api_url("/mcp"))
            .header(HOST, host)
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(grant.access_token.as_ref()),
            )
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {"name": "list_projects", "arguments": {}}
            }))
            .send()
            .await
            .expect("MCP request failed");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_challenge_exposed(resp.headers());
        let challenge = header(resp.headers(), &WWW_AUTHENTICATE);
        let origin = resource.trim_end_matches("/mcp");
        let resource_metadata = format!("{origin}/.well-known/oauth-protected-resource/mcp");
        assert_eq!(
            challenge,
            format!(
                r#"Bearer error="invalid_token", error_description="The access token is invalid", resource_metadata="{resource_metadata}""#
            ),
            "{host}"
        );

        let url = Url::parse(&resource_metadata).expect("Invalid resource metadata URL");
        let metadata: Value = server
            .client
            .get(server.api_url(url.path()))
            .header(HOST, url.host_str().expect("Missing host"))
            .send()
            .await
            .expect("Failed to read the resource metadata")
            .json()
            .await
            .expect("Invalid resource metadata");
        assert_eq!(metadata["resource"], resource, "{host}");
        assert_eq!(
            metadata["authorization_servers"],
            json!(["https://api.example.com"])
        );
    }
}

// A revoked grant takes its access tokens with it
#[tokio::test]
async fn oauth_revoked_grant() {
    let server = server_with(Some(oauth_server())).await;
    let user = server
        .signup("OAuth Revoked", "oauth-revoked@example.com")
        .await;
    let grant = grant(&server, &user, "projects:read");
    assert_accepted(&server, grant.access_token.as_ref()).await;

    let mut conn = server.db_conn();
    diesel::update(schema::oauth_grant::table.filter(schema::oauth_grant::uuid.eq(grant.uuid)))
        .set(schema::oauth_grant::revoked.eq(now()))
        .execute(&mut conn)
        .expect("Failed to revoke the grant");

    assert_refused(&server, grant.access_token.as_ref()).await;
}

// An expired grant takes its access tokens with it
#[tokio::test]
async fn oauth_expired_grant() {
    let server = server_with(Some(oauth_server())).await;
    let user = server
        .signup("OAuth Lapsed", "oauth-lapsed@example.com")
        .await;
    let grant = grant(&server, &user, "projects:read");
    assert_accepted(&server, grant.access_token.as_ref()).await;

    let mut conn = server.db_conn();
    diesel::update(schema::oauth_grant::table.filter(schema::oauth_grant::uuid.eq(grant.uuid)))
        .set(schema::oauth_grant::expiration.eq(now()))
        .execute(&mut conn)
        .expect("Failed to expire the grant");

    assert_refused(&server, grant.access_token.as_ref()).await;
}

// A token is honored only for a resource the server still serves
#[tokio::test]
async fn oauth_resource_outside_allowlist() {
    let server = server_with(Some(oauth_server())).await;
    let user = server
        .signup("OAuth Elsewhere", "oauth-elsewhere@example.com")
        .await;
    let elsewhere: OAuthResource = "https://mcp.example.net/mcp"
        .parse()
        .expect("Invalid resource");
    let grant = seed_oauth_grant(
        &server,
        &user,
        &"projects:read".parse().expect("Invalid scopes"),
        &elsewhere,
        None,
        now(),
    );

    assert_refused(&server, grant.access_token.as_ref()).await;
}

// A locked user's tokens are refused
#[tokio::test]
async fn oauth_locked_user() {
    let server = server_with(Some(oauth_server())).await;
    let user = server
        .signup("OAuth Locked", "oauth-locked@example.com")
        .await;
    let grant = grant(&server, &user, "projects:read");
    assert_accepted(&server, grant.access_token.as_ref()).await;

    let mut conn = server.db_conn();
    diesel::update(schema::user::table.filter(schema::user::uuid.eq(user.uuid)))
        .set(schema::user::locked.eq(true))
        .execute(&mut conn)
        .expect("Failed to lock the user");

    assert_refused(&server, grant.access_token.as_ref()).await;
}

// A grant approved with a user key dies when the key is revoked
#[tokio::test]
async fn oauth_bound_key_revoked() {
    let server = server_with(Some(oauth_server())).await;
    let user = server
        .signup("OAuth Keyed", "oauth-keyed@example.com")
        .await;
    let key = create_user_key(&server, &user).await;
    let grant = keyed_grant(&server, &user, key);
    assert_accepted(&server, grant.access_token.as_ref()).await;

    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/users/{user_slug}/keys/{key}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Failed to revoke the user key");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    assert_refused(&server, grant.access_token.as_ref()).await;
}

// A grant approved with a user key dies when the key expires
#[tokio::test]
async fn oauth_bound_key_expired() {
    let server = server_with(Some(oauth_server())).await;
    let user = server
        .signup("OAuth Key Lapsed", "oauth-key-lapsed@example.com")
        .await;
    let key = create_user_key(&server, &user).await;
    let grant = keyed_grant(&server, &user, key);
    assert_accepted(&server, grant.access_token.as_ref()).await;

    let mut conn = server.db_conn();
    diesel::update(schema::user_key::table.filter(schema::user_key::uuid.eq(key)))
        .set(schema::user_key::expiration.eq(now()))
        .execute(&mut conn)
        .expect("Failed to expire the user key");

    assert_refused(&server, grant.access_token.as_ref()).await;
}

// A grant can only be bound to a key of its own user
#[tokio::test]
async fn oauth_bound_key_of_another_user() {
    let server = server_with(Some(oauth_server())).await;
    let user = server
        .signup("OAuth Borrower", "oauth-borrower@example.com")
        .await;
    let other = server
        .signup("OAuth Lender", "oauth-lender@example.com")
        .await;
    let key = create_user_key(&server, &other).await;
    let grant = keyed_grant(&server, &user, key);

    assert_refused(&server, grant.access_token.as_ref()).await;
}

// OAuth traffic is charged to its user, once per request whatever its method,
// and a signed-in tool call over its user's limit is refused rather than asked to sign in
#[tokio::test]
async fn oauth_token_charged_per_user() {
    let server = rate_limited_server(json!({"user": {"requests": {"minute": 2}}})).await;
    let user = server
        .signup("OAuth Charged", "oauth-charged@example.com")
        .await;
    let grant = grant(&server, &user, "projects:read");

    let mut statuses = Vec::new();
    for body in [
        tool_call("list_projects", &json!({})),
        tools_list(),
        tool_call("list_projects", &json!({})),
    ] {
        let resp = post(&server, Some(grant.access_token.as_ref()), &body).await;
        statuses.push(resp.status());
    }
    assert_eq!(
        statuses,
        [
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::TOO_MANY_REQUESTS
        ]
    );
}

// Signed-in traffic never draws on the budget of the address it comes from,
// which the users of a hosted client all share, whatever its credential or method
#[tokio::test]
async fn oauth_signed_in_never_charged_per_ip() {
    let server = rate_limited_server(json!({"public": {"requests": {"minute": 1}}})).await;
    let user = server
        .signup("OAuth Shared", "oauth-shared@example.com")
        .await;
    let org = server.create_org(&user, "OAuth Shared Org").await;
    let project = server
        .create_project(&user, &org, "OAuth Shared Project")
        .await;
    let project_key = server
        .create_project_key(&user, &project, "oauth-key")
        .await;
    let user_key = create_user_key_secret(&server, &user).await;
    let grant = grant(&server, &user, "projects:read");
    let bearers: [&str; 4] = [
        grant.access_token.as_ref(),
        &user.token,
        &user_key,
        project_key.key.as_ref(),
    ];

    for bearer in bearers {
        for body in [tool_call("list_projects", &json!({})), tools_list()] {
            for _ in 0..2 {
                assert_eq!(
                    from_ip(&server, Some(bearer), &body).await.status(),
                    StatusCode::OK
                );
            }
        }
    }
    assert_eq!(
        from_ip(&server, None, &tools_list()).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        from_ip(&server, None, &tools_list()).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}

// A refused credential is answered with its challenge, never with the address's limit,
// and never draws on the address's budget
#[tokio::test]
async fn oauth_refused_credentials_not_charged_per_ip() {
    let server = rate_limited_server(json!({"public": {"requests": {"minute": 1}}})).await;
    let refused = [
        OAuthAccessToken::generate().to_string(),
        "bencher_run_invalid".to_owned(),
        "bencher_user_invalid".to_owned(),
        "not.a.jwt".to_owned(),
    ];
    let spray = async || {
        for bearer in &refused {
            for body in [tool_call("list_projects", &json!({})), tools_list()] {
                for _ in 0..2 {
                    let resp = from_ip(&server, Some(bearer), &body).await;
                    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{bearer}");
                }
            }
        }
    };

    spray().await;
    assert_eq!(
        from_ip(&server, None, &tools_list()).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        from_ip(&server, None, &tools_list()).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    spray().await;
}

// With OAuth unset, even a live OAuth token is just an invalid credential on every method,
// as before OAuth existed, and nothing challenges a client to sign in
#[tokio::test]
async fn oauth_unset_token_is_an_invalid_credential() {
    let server = server_with(None).await;
    let user = server
        .signup("OAuth Dormant", "oauth-dormant@example.com")
        .await;
    let grant = grant(&server, &user, "projects:write");

    let resp = call_tool(
        &server,
        grant.access_token.as_ref(),
        "list_projects",
        &json!({}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(
        resp.headers().get(WWW_AUTHENTICATE).is_none(),
        "Unexpected challenge: {resp:?}"
    );

    let resp = post(&server, Some(grant.access_token.as_ref()), &tools_list()).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(
        resp.headers().get(WWW_AUTHENTICATE).is_none(),
        "Unexpected challenge: {resp:?}"
    );
}

// An anonymous caller is asked to sign in with the scope the tool needs when the tool refuses it,
// and a standard client gets the challenge as an HTTP `401` in place of the tool result
#[tokio::test]
async fn oauth_anonymous_caller_signs_in() {
    let server = server_with(Some(oauth_server())).await;
    let user = server
        .signup("OAuth Owner", "oauth-owner@example.com")
        .await;
    let org = server.create_org(&user, "OAuth Owner Org").await;
    let project = server
        .create_project(&user, &org, "OAuth Public Project")
        .await;
    let private = server
        .create_project(&user, &org, "OAuth Hidden Project")
        .await;
    make_private(&server, &private);
    let slug: &str = project.slug.as_ref();

    let resp = post(
        &server,
        None,
        &tool_call(
            "create_testbed",
            &json!({"project": slug, "name": "Anonymous"}),
        ),
    )
    .await;
    assert_sign_in(
        resp,
        "projects:read projects:write",
        "Sign in to Bencher to change projects",
    )
    .await;
    let testbeds = tool_content(
        call_tool(
            &server,
            &user.token,
            "list_testbeds",
            &json!({"project": slug}),
        )
        .await,
    )
    .await;
    assert!(
        !testbeds["data"]
            .as_array()
            .expect("Missing data array")
            .iter()
            .any(|testbed| testbed["name"] == "Anonymous"),
        "The challenged tool ran: {testbeds}"
    );

    let resp = post(
        &server,
        None,
        &tool_call("view_project", &json!({"project": private.slug})),
    )
    .await;
    assert_sign_in(
        resp,
        "projects:read",
        "Sign in to Bencher to read private projects",
    )
    .await;

    // Any other tool error is answered as before, never as a challenge
    let resp = post(
        &server,
        None,
        &tool_call("view_project", &json!({"project": 5})),
    )
    .await;
    assert!(
        resp.headers().get(WWW_AUTHENTICATE).is_none(),
        "Unexpected challenge: {resp:?}"
    );
    let message: Value = resp.json().await.expect("Invalid JSON-RPC response");
    let error = tool_error_result(&message);
    assert!(error.starts_with("400"), "Expected 400 error: {message}");
    assert!(message["result"].get("_meta").is_none(), "{message}");
}

// The server instructions mention signing in only once OAuth is configured
#[tokio::test]
async fn oauth_instructions_mention_sign_in() {
    let mut instructions = Vec::new();
    for oauth in [None, Some(oauth_server())] {
        let server = server_with(oauth).await;
        let initialize = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "test-client", "version": "0.0.0"}
            }
        });
        let message: Value = post(&server, None, &initialize)
            .await
            .json()
            .await
            .expect("Invalid JSON-RPC response");
        instructions.push(message["result"]["instructions"].clone());
    }
    assert!(instructions[0].is_string(), "{instructions:?}");
    assert_ne!(instructions[0], instructions[1]);
}

// A private project and a missing one look the same to an anonymous caller, in either style,
// and both ask the caller to sign in
#[tokio::test]
async fn oauth_private_and_missing_projects_answer_alike() {
    let server = server_with(Some(oauth_server())).await;
    let user = server
        .signup("OAuth Hider", "oauth-hider@example.com")
        .await;
    let org = server.create_org(&user, "OAuth Hider Org").await;
    let private = server
        .create_project(&user, &org, "OAuth Hider Project")
        .await;
    make_private(&server, &private);
    let view = |project: &str| tool_call("view_project", &json!({"project": project}));

    let private_resp = post(&server, None, &view(private.slug.as_ref())).await;
    let missing_resp = post(&server, None, &view("oauth-no-such-project")).await;
    assert_eq!(private_resp.status(), StatusCode::UNAUTHORIZED);
    let (private_answer, missing_answer) = (answer(private_resp).await, answer(missing_resp).await);
    assert_eq!(private_answer, missing_answer);

    let private_resp = post_as_chatgpt(&server, None, view(private.slug.as_ref())).await;
    let missing_resp = post_as_chatgpt(&server, None, view("oauth-no-such-project")).await;
    let (private_answer, missing_answer) = (answer(private_resp).await, answer(missing_resp).await);
    assert_eq!(private_answer, missing_answer);
    let (status, _challenge, body) = private_answer;
    assert_eq!(status, StatusCode::OK);
    let message: Value = serde_json::from_slice(&body).expect("Invalid JSON-RPC response");
    assert!(
        message["result"]["_meta"]["mcp/www_authenticate"].is_array(),
        "Expected a challenge: {message}"
    );
}

// `ChatGPT` reads a challenge from the tool result, recognized by its `openai/` request `_meta`
// keys or else by its `User-Agent`, for sign-in and for more scopes alike, and the tool never runs
#[tokio::test]
async fn oauth_chatgpt_reads_the_challenge_from_the_result() {
    let server = server_with(Some(oauth_server())).await;
    let user = server.signup("OAuth Chat", "oauth-chat@example.com").await;
    let org = server.create_org(&user, "OAuth Chat Org").await;
    let project = server
        .create_project(&user, &org, "OAuth Chat Project")
        .await;
    let slug: &str = project.slug.as_ref();
    let create_testbed = tool_call("create_testbed", &json!({"project": slug, "name": "Chat"}));
    let sign_in = format!(
        r#"Bearer resource_metadata="{RESOURCE_METADATA}", error="insufficient_scope", error_description="Sign in to Bencher to change projects", scope="projects:read projects:write""#
    );

    let resp = post_as_chatgpt(&server, None, create_testbed.clone()).await;
    assert_chatgpt_challenge(resp, &sign_in).await;
    let resp =
        send(mcp_request(&server, None, &create_testbed).header(USER_AGENT, "openai-mcp/1.0.0"))
            .await;
    assert_chatgpt_challenge(resp, &sign_in).await;

    let read = grant(&server, &user, "projects:read");
    let resp = post_as_chatgpt(&server, Some(read.access_token.as_ref()), create_testbed).await;
    assert_chatgpt_challenge(
        resp,
        &format!(
            r#"Bearer resource_metadata="{RESOURCE_METADATA}", error="insufficient_scope", error_description="The access token does not permit this tool", scope="projects:read projects:write""#
        ),
    )
    .await;

    let testbeds = tool_content(
        call_tool(
            &server,
            &user.token,
            "list_testbeds",
            &json!({"project": slug}),
        )
        .await,
    )
    .await;
    assert!(
        !testbeds["data"]
            .as_array()
            .expect("Missing data array")
            .iter()
            .any(|testbed| testbed["name"] == "Chat"),
        "A challenged tool ran: {testbeds}"
    );
}

// An anonymous tool call over its address's limit is asked to sign in for higher limits,
// in the client's style, while every other anonymous message over the limit is refused
#[tokio::test]
async fn oauth_anonymous_over_limit_signs_in() {
    let server = rate_limited_server(json!({"public": {"requests": {"minute": 1}}})).await;
    let user = server.signup("OAuth Busy", "oauth-busy@example.com").await;
    let org = server.create_org(&user, "OAuth Busy Org").await;
    let project = server
        .create_project(&user, &org, "OAuth Busy Project")
        .await;
    let view_project = tool_call("view_project", &json!({"project": project.slug}));

    let resp = from_ip(&server, None, &view_project).await;
    tool_content(resp).await;
    let resp = from_ip(&server, None, &view_project).await;
    assert_sign_in(
        resp,
        "projects:read",
        "Sign in to Bencher for higher request limits",
    )
    .await;
    let mut chatgpt = view_project.clone();
    chatgpt["params"]["_meta"] = json!({"openai/subject": "v1/anonymized-subject"});
    let resp = from_ip(&server, None, &chatgpt).await;
    assert_chatgpt_challenge(
        resp,
        &format!(
            r#"Bearer resource_metadata="{RESOURCE_METADATA}", error="insufficient_scope", error_description="Sign in to Bencher for higher request limits", scope="projects:read""#
        ),
    )
    .await;

    for body in [tools_list(), tool_call("no_such_tool", &json!({}))] {
        assert_eq!(
            from_ip(&server, None, &body).await.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "{body}"
        );
    }
    tool_content(from_ip(&server, Some(&user.token), &view_project).await).await;
}

async fn server_with(oauth: Option<OAuthServer>) -> TestServer {
    TestServer::new_with_options(TestServerOptions {
        clock: Some(Clock::Custom(Arc::new(now))),
        oauth,
        ..TestServerOptions::default()
    })
    .await
}

#[expect(clippy::expect_used, reason = "test helper parsing fixed rate limits")]
async fn rate_limited_server(rate_limiting: Value) -> TestServer {
    let rate_limiting: JsonRateLimiting =
        serde_json::from_value(rate_limiting).expect("Invalid rate limiting");
    TestServer::new_with_options(TestServerOptions {
        clock: Some(Clock::Custom(Arc::new(now))),
        rate_limiting: Some(RateLimiting::from(rate_limiting)),
        oauth: Some(oauth_server()),
        ..TestServerOptions::default()
    })
    .await
}

#[expect(clippy::expect_used, reason = "test helper with a fixed time")]
fn now() -> DateTime {
    DateTime::try_from(NOW).expect("Invalid time")
}

#[expect(clippy::expect_used, reason = "test helper parsing fixed scopes")]
fn grant(server: &TestServer, user: &TestUser, scopes: &str) -> TestOAuthGrant {
    seed_oauth_grant(
        server,
        user,
        &scopes.parse().expect("Invalid scopes"),
        &oauth_resource(),
        None,
        now(),
    )
}

#[expect(clippy::expect_used, reason = "test helper with fixed scopes")]
fn keyed_grant(server: &TestServer, user: &TestUser, key: UserKeyUuid) -> TestOAuthGrant {
    seed_oauth_grant(
        server,
        user,
        &"projects:read".parse().expect("Invalid scopes"),
        &oauth_resource(),
        Some(key),
        now(),
    )
}

async fn create_user_key(server: &TestServer, user: &TestUser) -> UserKeyUuid {
    create_user_key_created(server, user).await.uuid
}

async fn create_user_key_secret(server: &TestServer, user: &TestUser) -> String {
    create_user_key_created(server, user).await.key.to_string()
}

#[expect(clippy::expect_used, reason = "test helper creating a user key")]
async fn create_user_key_created(server: &TestServer, user: &TestUser) -> JsonUserKeyCreated {
    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{user_slug}/keys")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&json!({"name": "OAuth Key"}))
        .send()
        .await
        .expect("Failed to create a user key");
    assert_eq!(resp.status(), StatusCode::CREATED);
    resp.json().await.expect("Invalid user key")
}

#[expect(clippy::expect_used, reason = "test helper looking up a grant")]
fn grant_id(conn: &mut diesel::SqliteConnection, grant: &TestOAuthGrant) -> OAuthGrantId {
    schema::oauth_grant::table
        .filter(schema::oauth_grant::uuid.eq(grant.uuid))
        .select(schema::oauth_grant::id)
        .first(conn)
        .expect("Failed to find the grant")
}

async fn call_tool(
    server: &TestServer,
    bearer: &str,
    name: &str,
    arguments: &Value,
) -> reqwest::Response {
    post(server, Some(bearer), &tool_call(name, arguments)).await
}

async fn post(server: &TestServer, bearer: Option<&str>, body: &Value) -> reqwest::Response {
    send(mcp_request(server, bearer, body)).await
}

/// POST from the one address that every rate limited test charges.
async fn from_ip(server: &TestServer, bearer: Option<&str>, body: &Value) -> reqwest::Response {
    send(mcp_request(server, bearer, body).header("X-Forwarded-For", REMOTE_IP)).await
}

/// POST as `ChatGPT` does, marking the tool call with an `openai/` request `_meta` key.
#[expect(
    clippy::indexing_slicing,
    reason = "test helper; indexing a serde_json value never panics"
)]
async fn post_as_chatgpt(
    server: &TestServer,
    bearer: Option<&str>,
    mut body: Value,
) -> reqwest::Response {
    body["params"]["_meta"] = json!({"openai/subject": "v1/anonymized-subject"});
    post(server, bearer, &body).await
}

fn mcp_request(server: &TestServer, bearer: Option<&str>, body: &Value) -> reqwest::RequestBuilder {
    let mut request = server.client.post(server.api_url("/mcp")).json(body);
    if let Some(bearer) = bearer {
        request = request.header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bearer),
        );
    }
    request
}

#[expect(clippy::expect_used, reason = "test helper")]
async fn send(request: reqwest::RequestBuilder) -> reqwest::Response {
    request.send().await.expect("MCP request failed")
}

fn tool_call(name: &str, arguments: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments}
    })
}

fn tools_list() -> Value {
    json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})
}

async fn assert_accepted(server: &TestServer, token: &str) {
    tool_content(call_tool(server, token, "list_projects", &json!({})).await).await;
}

/// Assert a `401` that challenges a client for a new access token, readable by a browser,
/// whatever the method.
async fn assert_refused(server: &TestServer, token: &str) {
    for body in [tool_call("list_projects", &json!({})), tools_list()] {
        let resp = post(server, Some(token), &body).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{token}: {body}");
        let challenge = header(resp.headers(), &WWW_AUTHENTICATE);
        assert!(
            challenge.starts_with(r#"Bearer error="invalid_token""#),
            "Unexpected challenge: {challenge}"
        );
        assert_challenge_exposed(resp.headers());
    }
}

fn assert_challenge_exposed(headers: &HeaderMap) {
    assert_eq!(header(headers, &ACCESS_CONTROL_ALLOW_ORIGIN), "*");
    assert!(
        headers
            .get_all(ACCESS_CONTROL_EXPOSE_HEADERS)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .any(|name| name.trim().eq_ignore_ascii_case("WWW-Authenticate")),
        "WWW-Authenticate is not exposed: {headers:?}"
    );
}

#[expect(clippy::expect_used, reason = "test helper")]
fn header<'h>(headers: &'h HeaderMap, name: &HeaderName) -> &'h str {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .expect("Missing header")
}

/// Assert a successful tool call and return its `structuredContent`.
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helper; indexing a serde_json value never panics"
)]
async fn tool_content(resp: reqwest::Response) -> Value {
    assert_eq!(resp.status(), StatusCode::OK);
    let message: Value = resp.json().await.expect("Invalid JSON-RPC response");
    let result = &message["result"];
    assert_ne!(result["isError"], true, "Unexpected tool error: {message}");
    result["structuredContent"].clone()
}

/// The text of a failed tool call's JSON-RPC response.
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helper; indexing a serde_json value never panics"
)]
fn tool_error_result(message: &Value) -> &str {
    assert_eq!(
        message["result"]["isError"], true,
        "Expected tool error: {message}"
    );
    message["result"]["content"][0]["text"]
        .as_str()
        .expect("Missing tool error text")
}

/// Assert a failed tool call and return its text content.
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helper; indexing a serde_json value never panics"
)]
async fn tool_error(resp: reqwest::Response) -> String {
    assert_eq!(resp.status(), StatusCode::OK);
    let message: Value = resp.json().await.expect("Invalid JSON-RPC response");
    let result = &message["result"];
    assert_eq!(result["isError"], true, "Expected tool error: {message}");
    result["content"][0]["text"]
        .as_str()
        .expect("Missing tool error text")
        .to_owned()
}

/// Assert a standard client's challenge to sign in: a `401` without an error code, with a body
/// that says why, readable by a browser.
#[expect(clippy::expect_used, reason = "test helper")]
async fn assert_sign_in(resp: reqwest::Response, scope: &str, description: &str) {
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        header(resp.headers(), &WWW_AUTHENTICATE),
        format!(r#"Bearer resource_metadata="{RESOURCE_METADATA}", scope="{scope}""#)
    );
    assert_challenge_exposed(resp.headers());
    let body: Value = resp.json().await.expect("Invalid error body");
    assert_eq!(
        body,
        json!({"error": "login_required", "error_description": description})
    );
}

/// Assert `ChatGPT`'s challenge: a tool error whose `_meta` carries `challenge`.
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helper; indexing a serde_json value never panics"
)]
async fn assert_chatgpt_challenge(resp: reqwest::Response, challenge: &str) {
    assert_eq!(resp.status(), StatusCode::OK);
    let message: Value = resp.json().await.expect("Invalid JSON-RPC response");
    let result = &message["result"];
    assert_eq!(result["isError"], true, "{message}");
    assert_eq!(
        result["_meta"]["mcp/www_authenticate"],
        json!([challenge]),
        "{message}"
    );
}

/// What a client can tell apart in a response: everything but the per-request id.
#[expect(clippy::expect_used, reason = "test helper")]
async fn answer(resp: reqwest::Response) -> (StatusCode, Option<String>, Vec<u8>) {
    let status = resp.status();
    let challenge = resp
        .headers()
        .get(WWW_AUTHENTICATE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = resp.bytes().await.expect("Missing body").to_vec();
    (status, challenge, body)
}

#[expect(clippy::expect_used, reason = "test helper hiding a project")]
fn make_private(server: &TestServer, project: &TestProject) {
    let mut conn = server.db_conn();
    diesel::update(schema::project::table.filter(schema::project::uuid.eq(project.uuid)))
        .set(schema::project::visibility.eq(Visibility::Private))
        .execute(&mut conn)
        .expect("Failed to update project visibility");
}
