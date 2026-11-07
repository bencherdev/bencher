//! A browser session on a server that sends no email, signed in as the console signs in: by the
//! code in the sign-in email, which the API writes to its log.

use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use bencher_json::{JsonProjectKeyCreated, JsonUserKeyCreated, Url};

/// How long the API gets to log the email after it answers.
const LOG_WAIT: Duration = Duration::from_secs(10);
const LOG_POLL: Duration = Duration::from_millis(100);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CODE_SUBJECT: &str = "Subject: Your Bencher code is ";

/// A session signed in within the last 10 minutes, which a new key needs.
pub struct BrowserSession {
    agent: ureq::Agent,
    url: Url,
    cookie: String,
}

impl BrowserSession {
    /// Sign `email` in by the code in the email the API writes to `api_log`.
    pub fn sign_in(url: &Url, email: &str, api_log: &Path) -> anyhow::Result<Self> {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build();
        let agent = ureq::Agent::new_with_config(config);
        let mark = fs::metadata(api_log)?.len();
        let (pending, _started) = post(
            &agent,
            url,
            "/v0/auth/email",
            None,
            &serde_json::json!({ "email": email }),
        )?;
        let code = logged_code(api_log, mark, email)?;
        let (cookie, _signed_in) = post(
            &agent,
            url,
            "/v0/auth/code",
            Some(&pending),
            &serde_json::json!({ "code": code }),
        )?;
        Ok(Self {
            agent,
            url: url.clone(),
            cookie,
        })
    }

    pub fn create_project_key(
        &self,
        project: &str,
        name: &str,
    ) -> anyhow::Result<JsonProjectKeyCreated> {
        let created = self.create_key(&format!("/v0/projects/{project}/keys"), name)?;
        Ok(serde_json::from_str(&created)?)
    }

    pub fn create_user_key(&self, user: &str, name: &str) -> anyhow::Result<JsonUserKeyCreated> {
        let created = self.create_key(&format!("/v0/users/{user}/keys"), name)?;
        Ok(serde_json::from_str(&created)?)
    }

    fn create_key(&self, path: &str, name: &str) -> anyhow::Result<String> {
        let (_cookie, created) = post(
            &self.agent,
            &self.url,
            path,
            Some(&self.cookie),
            &serde_json::json!({ "name": name }),
        )?;
        Ok(created)
    }
}

/// POST `body` to `path` as the console's own origin, with `cookie`, and return the cookies the
/// API sets and its answer.
fn post(
    agent: &ureq::Agent,
    url: &Url,
    path: &str,
    cookie: Option<&str>,
    body: &serde_json::Value,
) -> anyhow::Result<(String, String)> {
    let target = format!("{}{path}", url.as_ref().trim_end_matches('/'));
    let mut request = agent
        .post(&target)
        .header("Sec-Fetch-Site", "same-origin")
        .header("Content-Type", "application/json");
    if let Some(cookie) = cookie {
        request = request.header("Cookie", cookie);
    }
    let mut response = request.send(body.to_string())?;
    let status = response.status();
    let cookies = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|value| value.split(';').next())
        .filter(|pair| {
            pair.split_once('=')
                .is_some_and(|(_, value)| !value.is_empty())
        })
        .collect::<Vec<_>>()
        .join("; ");
    let answer = response.body_mut().read_to_string()?;
    anyhow::ensure!(
        status.is_success(),
        "POST {target} answered {status}: {answer}"
    );
    Ok((cookies, answer))
}

/// The code in the newest sign-in email to `email` that the API logged after `mark`.
fn logged_code(api_log: &Path, mark: u64, email: &str) -> anyhow::Result<String> {
    let start = usize::try_from(mark)?;
    let recipient = format!("<{email}>");
    let deadline = Instant::now() + LOG_WAIT;
    loop {
        let logged = fs::read(api_log)?;
        let code = String::from_utf8_lossy(logged.get(start..).unwrap_or_default())
            .lines()
            .rev()
            .find_map(|line| {
                let line = serde_json::from_str::<serde_json::Value>(line).ok()?;
                let message = line.get("msg")?.as_str()?;
                if !message.contains(&recipient) {
                    return None;
                }
                message
                    .lines()
                    .find_map(|line| line.strip_prefix(CODE_SUBJECT))
                    .map(ToOwned::to_owned)
            });
        if let Some(code) = code {
            return Ok(code);
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "No sign-in email to {email} in {}",
            api_log.display()
        );
        thread::sleep(LOG_POLL);
    }
}
