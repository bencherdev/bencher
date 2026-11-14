#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Nothing mints, accepts, or holds a session JWT, the `client` audience the old sign-in confirmed
//! with, but the one forger the tests that pin its refusal call.

use std::{
    fs,
    path::{Path, PathBuf},
};

use bencher_api_tests::TestServer;
use bencher_json::UserKey;

/// The forger, the only file that may name the `client` audience.
const FORGER: &str = "lib/bencher_api_tests/src/seed.rs";

/// Each way to mint, accept, or hold a session JWT, and the only files that may hold it.
const SESSION_JWTS: [(&str, &[&str]); 9] = [
    ("new_client(", &[]),
    ("validate_client(", &[]),
    ("Audience::Client", &[]),
    ("::test_token", &[]),
    ("::test_admin_token", &[]),
    ("aud: \"client\"", &[FORGER]),
    ("\"aud\": \"client\"", &[]),
    ("\"aud\":\"client\"", &[]),
    // A JWT whose payload opens with `{"aud":"client"`, as every session JWT's does.
    ("eyJhdWQiOiJjbGllbnQi", &[]),
];

/// What no source lives under.
const SKIPPED: [&str; 5] = ["target", "node_modules", "dist", "pkg", ".git"];
/// The files code, scripts, configs, and docs are written in.
const EXTENSIONS: [&str; 15] = [
    "rs", "sh", "ps1", "ts", "tsx", "js", "mjs", "astro", "md", "mdx", "json", "yml", "yaml",
    "toml", "html",
];
/// This file, which names every pattern.
const THIS_FILE: &str = "lib/bencher_api_tests/tests/session_jwts.rs";

// Kills a way back for the old sign-in's session JWT: a mint, an audience that accepts one, or a
// test or a doc that holds one, which would keep working only as long as the API took it.
#[test]
fn no_session_jwt_outside_its_forger() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    sources(&workspace, &mut files);
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
        "A session JWT outside its forger:\n{}",
        strays.join("\n")
    );
}

// The check above trusts the harness to make test users without a session JWT, as its forger is
// among the files it allows.
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
