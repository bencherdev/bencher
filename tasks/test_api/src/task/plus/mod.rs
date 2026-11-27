use bencher_json::{Url, UserKey};

use crate::task::session::{ApiLog, BrowserSession};

pub mod oci;
pub mod runner;

/// Create a user API key named `name` for `user`, who signs in as `email`, from a browser session
/// signed in by the code the API writes to `api_log`.
fn create_user_key(
    url: &Url,
    email: &str,
    api_log: &ApiLog,
    user: &str,
    name: &str,
) -> anyhow::Result<UserKey> {
    Ok(BrowserSession::sign_in(url, email, api_log)?
        .create_user_key(user, name)?
        .key)
}
