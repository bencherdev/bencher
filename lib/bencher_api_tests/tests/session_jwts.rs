#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Only the tests that pin how the API treats a session JWT, the `client` audience the old sign-in
//! confirms with, mint or send one, so no other test rests on what goes with the old sign-in.

use std::{
    fs,
    path::{Path, PathBuf},
};

use bencher_api_tests::TestServer;
use bencher_json::UserKey;

/// Each way to come by a session JWT, and the only files that may hold it: where it is defined,
/// the old sign-in that mints one, and the tests that pin how the API treats one.
const SESSION_JWTS: [(&str, &[&str]); 7] = [
    (
        "new_client(",
        &[
            "lib/bencher_token/src/key.rs",
            "lib/api_auth/src/confirm.rs",
            "lib/bencher_api_tests/src/seed.rs",
            "lib/api_auth/tests/credentials.rs",
        ],
    ),
    (
        "session_jwt(",
        &[
            "lib/bencher_api_tests/src/seed.rs",
            "lib/api_auth/tests/credentials.rs",
            "lib/api_auth/tests/fresh.rs",
            "lib/api_auth/tests/invite.rs",
            "lib/api_auth/tests/session.rs",
        ],
    ),
    ("::test_token", &["lib/bencher_valid/src/jwt.rs"]),
    ("::test_admin_token", &["lib/bencher_valid/src/jwt.rs"]),
    (
        "/v0/auth/confirm\"",
        &[
            "lib/api_auth/src/confirm.rs",
            "lib/api_auth/tests/confirm.rs",
            // The API client, which its build generates with every route.
            "lib/bencher_client/codegen.rs",
        ],
    ),
    // A JWT whose payload opens with `{"aud":"client"`, as every session JWT's does.
    (
        "eyJhdWQiOiJjbGllbnQi",
        &[
            "lib/bencher_valid/src/jwt.rs",
            "lib/api_users/tests/user_key_auth.rs",
        ],
    ),
    ("\"aud\": \"client\"", &["plus/api_oauth/tests/oidc.rs"]),
];

/// The directories that hold the workspace's code, its tests, and its tasks.
const ROOTS: [&str; 4] = ["lib", "plus", "services", "tasks"];
/// What no source lives under.
const SKIPPED: [&str; 5] = ["target", "node_modules", "dist", "pkg", ".git"];
/// The files code and scripts are written in.
const EXTENSIONS: [&str; 3] = ["rs", "sh", "ps1"];
/// This file, which names every pattern.
const THIS_FILE: &str = "lib/bencher_api_tests/tests/session_jwts.rs";

// Kills a test that mints or sends a session JWT anywhere but where one is pinned, which would
// keep working only as long as the old sign-in does.
#[test]
fn session_jwts_stay_in_the_tests_that_pin_them() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for root in ROOTS {
        sources(&workspace.join(root), &mut files);
    }
    assert!(
        files.len() > 100,
        "Found only {} source files under {}",
        files.len(),
        workspace.display()
    );

    let mut strays = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(&workspace)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        if relative == THIS_FILE {
            continue;
        }
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for (pattern, allowed) in SESSION_JWTS {
            if text.contains(pattern) && !allowed.contains(&relative.as_str()) {
                strays.push(format!("{relative}: {pattern}"));
            }
        }
    }
    assert!(
        strays.is_empty(),
        "A session JWT outside the tests that pin one:\n{}",
        strays.join("\n")
    );
}

// The check above trusts the harness to make test users without a session JWT, as its own
// definitions are among the files it allows.
// Kills a harness that gives a test user anything but a user API key to send.
#[tokio::test]
async fn a_test_user_sends_a_user_api_key() {
    let server = TestServer::new().await;
    let user = server.signup("Ada", "ada@example.com").await;
    let _key: UserKey = user
        .token
        .parse()
        .expect("A test user's bearer is not a user API key");
    server.close().await;
}

#[expect(clippy::expect_used, reason = "test helper reading the workspace")]
fn sources(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("Failed to read a source directory") {
        let path = entry.expect("Failed to read a directory entry").path();
        let name = path.file_name().map(|name| name.to_string_lossy());
        if path.is_dir() {
            if !name.is_some_and(|name| SKIPPED.contains(&name.as_ref())) {
                sources(&path, files);
            }
        } else if path
            .extension()
            .is_some_and(|extension| EXTENSIONS.iter().any(|known| extension == *known))
        {
            files.push(path);
        }
    }
}
