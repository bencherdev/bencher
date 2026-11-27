use std::{path::Path, process::Command};

use assert_cmd::cargo::CommandCargoExt as _;
use bencher_json::{JsonUserKeyCreated, Jwt, Url, UserKey};

use crate::task::{
    session::BrowserSession,
    test::seed_test::{BENCHER_CMD, CLI_DIR, HOST_ARG, TOKEN_ARG},
};

pub mod oci;
pub mod runner;

/// Create a user API key named `name` for `user`, who signs in as `email`: from a browser session
/// signed in by the code the API writes to `api_log`, or else with `token`.
fn create_user_key(
    url: &Url,
    email: &str,
    token: &Jwt,
    api_log: Option<&Path>,
    user: &str,
    name: &str,
) -> anyhow::Result<UserKey> {
    if let Some(api_log) = api_log {
        let created = BrowserSession::sign_in(url, email, api_log)?.create_user_key(user, name)?;
        return Ok(created.key);
    }
    let output = Command::cargo_bin(BENCHER_CMD)?
        .args([
            "user",
            "key",
            "create",
            HOST_ARG,
            url.as_ref(),
            TOKEN_ARG,
            token.as_ref(),
            "--name",
            name,
            user,
        ])
        .current_dir(CLI_DIR)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "Failed to create user key:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let created: JsonUserKeyCreated = serde_json::from_slice(&output.stdout)?;
    Ok(created.key)
}
