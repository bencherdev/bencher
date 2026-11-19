//! A browser session on a server that sends no email, signed in as the console signs in: by the
//! code in the sign-in email, which the API writes to its log.

use std::{
    collections::HashSet,
    fmt, fs,
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use bencher_json::{
    JsonOrganization, JsonProjectKeyCreated, JsonSignInEmail, JsonSignedIn, JsonUser,
    JsonUserKeyCreated, Url, UserKey,
};

use super::{
    is_dev,
    test::seed_test::{ADMIN_EMAIL, ADMIN_NAME, ADMIN_SLUG, USER_EMAIL, USER_NAME, USER_SLUG},
};

/// The Fly.io app that serves the dev API.
const DEV_FLY_APP: &str = "bencher-api-dev";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CODE_SUBJECT: &str = "Subject: Your Bencher code is ";
/// Where an invitation's secret starts in its email's link.
const INVITE_LINK: &str = "/auth/login#invite=";
/// The refusal of an action that needs a session signed in within the last 10 minutes.
const FRESH_SIGN_IN: &str = "fresh_sign_in";

/// Where the API under test writes each email it would send, in place of sending it.
#[derive(Debug, Clone)]
pub enum ApiLog {
    /// A file that holds the API's output, as the CI and local runs keep it.
    File(PathBuf),
    /// The logs of the Fly.io app that runs the API, read with `flyctl`.
    Fly(String),
}

/// Where an API log stood, so that only what the API logs after it counts.
pub enum Mark {
    /// The file's length.
    Len(u64),
    /// Every message Fly held.
    Seen(HashSet<String>),
}

impl ApiLog {
    /// The log of the API at `url`: the file `api_log` names, or else dev's Fly app.
    pub fn new(url: &Url, api_log: Option<PathBuf>) -> anyhow::Result<Self> {
        if let Some(api_log) = api_log {
            Ok(Self::File(api_log))
        } else if is_dev(Some(url)) {
            Ok(Self::Fly(DEV_FLY_APP.to_owned()))
        } else {
            anyhow::bail!(
                "Signing in needs the code the API logs in place of each email. Run the API with \
                 its output in a file, and pass that file to `--api-log`."
            )
        }
    }

    /// How long the API gets to log an email after it answers, and how often to look.
    fn wait(&self) -> (Duration, Duration) {
        match self {
            Self::File(_) => (Duration::from_secs(10), Duration::from_millis(100)),
            // Fly ships each line to its log store a few seconds after the app writes it.
            Self::Fly(_) => (Duration::from_mins(1), Duration::from_secs(2)),
        }
    }

    pub fn mark(&self) -> anyhow::Result<Mark> {
        Ok(match self {
            Self::File(path) => Mark::Len(fs::metadata(path)?.len()),
            Self::Fly(app) => Mark::Seen(fly_logs(app)?.into_iter().collect()),
        })
    }

    /// Each line the API logged after `mark`, oldest first.
    fn lines_after(&self, mark: &Mark) -> anyhow::Result<Vec<String>> {
        Ok(match (self, mark) {
            (Self::File(path), Mark::Len(len)) => {
                let logged = fs::read(path)?;
                let start = usize::try_from(*len)?;
                String::from_utf8_lossy(logged.get(start..).unwrap_or_default())
                    .lines()
                    .map(ToOwned::to_owned)
                    .collect()
            },
            (Self::Fly(app), Mark::Seen(seen)) => fly_logs(app)?
                .into_iter()
                .filter(|line| !seen.contains(line))
                .collect(),
            (Self::File(_), Mark::Seen(_)) | (Self::Fly(_), Mark::Len(_)) => {
                anyhow::bail!("A mark of another log")
            },
        })
    }

    /// The code in the newest sign-in email to `email` that the API logged after `mark`, once
    /// there is one.
    #[cfg(feature = "plus")]
    pub fn sign_in_code(&self, mark: &Mark, email: &str) -> anyhow::Result<String> {
        logged(self, mark, email, code)
    }

    /// How many sign-in emails to `email` the API logged after `mark`.
    #[cfg(feature = "plus")]
    pub fn sign_in_emails(&self, mark: &Mark, email: &str) -> anyhow::Result<usize> {
        Ok(emails(&self.lines_after(mark)?, email, code).count())
    }

    fn describe(&self) -> String {
        match self {
            Self::File(path) => path.display().to_string(),
            Self::Fly(app) => format!("the Fly.io logs of {app}"),
        }
    }
}

/// Every line the Fly.io app logged that Fly still holds, oldest first.
fn fly_logs(app: &str) -> anyhow::Result<Vec<String>> {
    let output = Command::new("flyctl")
        .args(["logs", "--app", app, "--no-tail", "--json"])
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "Failed to read the Fly.io logs of {app}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fly_lines(&String::from_utf8_lossy(&output.stdout))
}

/// The line each entry of `flyctl logs --json` holds, which writes one JSON object per entry.
fn fly_lines(output: &str) -> anyhow::Result<Vec<String>> {
    serde_json::Deserializer::from_str(output)
        .into_iter::<serde_json::Value>()
        .map(|entry| {
            entry?
                .get("message")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
                .ok_or_else(|| anyhow::anyhow!("A Fly.io log entry with no message"))
        })
        .collect()
}

/// What `read` finds in each email to `email` among the API's log `lines`, oldest first.
fn emails<F>(lines: &[String], email: &str, read: F) -> impl DoubleEndedIterator<Item = String>
where
    F: Fn(&str) -> Option<String>,
{
    let recipient = format!("<{email}>");
    lines.iter().filter_map(move |line| {
        let line = serde_json::from_str::<serde_json::Value>(line).ok()?;
        let message = line.get("msg")?.as_str()?;
        if !message.contains(&recipient) {
            return None;
        }
        read(message)
    })
}

/// What `read` finds in the newest email to `email` among the API's log `lines`.
fn newest_email<F>(lines: &[String], email: &str, read: F) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    emails(lines, email, read).next_back()
}

/// The code a sign-in email carries.
fn code(message: &str) -> Option<String> {
    message
        .lines()
        .find_map(|line| line.strip_prefix(CODE_SUBJECT))
        .map(ToOwned::to_owned)
}

/// What `read` finds in the newest email to `email` that the API logged after `mark`.
fn logged<F>(api_log: &ApiLog, mark: &Mark, email: &str, read: F) -> anyhow::Result<String>
where
    F: Fn(&str) -> Option<String>,
{
    let (wait, poll) = api_log.wait();
    let deadline = Instant::now() + wait;
    loop {
        if let Some(found) = newest_email(&api_log.lines_after(mark)?, email, &read) {
            return Ok(found);
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "No email to {email} in {}",
            api_log.describe()
        );
        thread::sleep(poll);
    }
}

/// A seeded reader: a browser session, and the user API key the CLI sends as them.
#[derive(Debug, Clone)]
pub struct Reader {
    pub session: BrowserSession,
    pub key: UserKey,
}

/// The seed's server admin, Eustace Bagge, and its user, Muriel Bagge.
#[derive(Debug, Clone)]
pub struct Readers {
    pub admin: Reader,
    pub user: Reader,
}

impl Readers {
    /// Sign Eustace in first, so he is the server admin, then Muriel, each named and with a key.
    pub fn sign_in(url: &Url, api_log: &ApiLog) -> anyhow::Result<Self> {
        Ok(Self {
            admin: Reader::sign_in(url, api_log, ADMIN_EMAIL, ADMIN_NAME, ADMIN_SLUG)?,
            user: Reader::sign_in(url, api_log, USER_EMAIL, USER_NAME, USER_SLUG)?,
        })
    }
}

impl Reader {
    /// Sign `email` in, name the user `name` with the slug `slug`, which a new account's name from
    /// its email does not give, and make the user a key.
    fn sign_in(
        url: &Url,
        api_log: &ApiLog,
        email: &str,
        name: &str,
        slug: &str,
    ) -> anyhow::Result<Self> {
        let session = BrowserSession::sign_in(url, email, api_log)?;
        session.rename(&session.user.uuid.to_string(), name, slug)?;
        let key = session.create_user_key(slug, "seed")?.key;
        Ok(Self { session, key })
    }
}

/// A browser session, signed in by the emailed code.
#[derive(Clone)]
pub struct BrowserSession {
    agent: ureq::Agent,
    url: Url,
    email: String,
    cookie: String,
    api_log: ApiLog,
    /// Who signed in.
    user: JsonUser,
}

impl fmt::Debug for BrowserSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserSession")
            .field("url", &self.url)
            .field("email", &self.email)
            .finish_non_exhaustive()
    }
}

/// What the API answered: its status, the cookies it set, and its body.
struct Answer {
    status: u16,
    cookies: String,
    body: String,
}

impl BrowserSession {
    /// Sign `email` in by the code in the email the API writes to `api_log`.
    pub fn sign_in(url: &Url, email: &str, api_log: &ApiLog) -> anyhow::Result<Self> {
        let agent = agent();
        let mark = api_log.mark()?;
        let pending = emailed(start(&agent, url, email)?, email)?;
        let code = logged(api_log, &mark, email, code)?;
        let signed_in = send(
            &agent,
            url,
            "POST",
            "/v0/auth/code",
            Some(&pending),
            Some(&serde_json::json!({ "code": code })),
        )?
        .ok()?;
        let JsonSignedIn { user, .. } = serde_json::from_str(&signed_in.body)?;
        Ok(Self {
            agent,
            url: url.clone(),
            email: email.to_owned(),
            cookie: signed_in.cookies,
            api_log: api_log.clone(),
            user,
        })
    }

    /// Start a sign-in for `email` from a browser of its own, and return the API's answer.
    #[cfg(feature = "plus")]
    pub fn start_sign_in(url: &Url, email: &str) -> anyhow::Result<JsonSignInEmail> {
        Ok(serde_json::from_str(&start(&agent(), url, email)?.body)?)
    }

    /// Prove the session again by the code in the email the API writes to its log, as the fresh
    /// sign-in dialog does.
    fn prove_again(&self) -> anyhow::Result<()> {
        let mark = self.api_log.mark()?;
        let pending = emailed(self.post("/v0/auth/fresh/email", None)?.ok()?, &self.email)?;
        let code = logged(&self.api_log, &mark, &self.email, code)?;
        send(
            &self.agent,
            &self.url,
            "POST",
            "/v0/auth/fresh/code",
            Some(&format!("{}; {pending}", self.cookie)),
            Some(&serde_json::json!({ "code": code })),
        )?
        .ok()?;
        Ok(())
    }

    /// Name the session's user `name` with the slug `slug`.
    pub fn rename(&self, user: &str, name: &str, slug: &str) -> anyhow::Result<JsonUser> {
        let renamed = self.send_json(
            "PATCH",
            &format!("/v0/users/{user}"),
            &serde_json::json!({ "name": name, "slug": slug }),
        )?;
        Ok(serde_json::from_str(&renamed)?)
    }

    /// Invite `email` to `organization` as a leader, and return the secret in the invitation's
    /// link, which the API writes to its log.
    pub fn invite(&self, organization: &str, email: &str, name: &str) -> anyhow::Result<String> {
        let mark = self.api_log.mark()?;
        let body = serde_json::json!({ "name": name, "email": email, "role": "leader" });
        self.fresh(|| {
            self.post(
                &format!("/v0/organizations/{organization}/members"),
                Some(&body),
            )
        })?;
        logged(&self.api_log, &mark, email, |message| {
            message
                .split_once(INVITE_LINK)
                .and_then(|(_, rest)| rest.split_whitespace().next())
                .map(ToOwned::to_owned)
        })
    }

    /// Accept the invitation the secret names, as the session's user, and return its organization.
    pub fn accept_invite(&self, secret: &str) -> anyhow::Result<JsonOrganization> {
        let accepted = self.send_json(
            "POST",
            "/v0/auth/invite/accept",
            &serde_json::json!({ "secret": secret }),
        )?;
        Ok(serde_json::from_str(&accepted)?)
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
        let body = serde_json::json!({ "name": name });
        Ok(self.fresh(|| self.post(path, Some(&body)))?.body)
    }

    /// `action`'s answer, once the session is proved again if the action needs a fresh sign-in.
    fn fresh<F>(&self, action: F) -> anyhow::Result<Answer>
    where
        F: Fn() -> anyhow::Result<Answer>,
    {
        let answer = action()?;
        if answer.status == 403 && answer.body.contains(FRESH_SIGN_IN) {
            self.prove_again()?;
            return action()?.ok();
        }
        answer.ok()
    }

    /// The body of a GET of `path` in the session.
    pub fn get(&self, path: &str) -> anyhow::Result<String> {
        Ok(send(
            &self.agent,
            &self.url,
            "GET",
            path,
            Some(&self.cookie),
            None,
        )?
        .ok()?
        .body)
    }

    /// The body of `method` on `path` with `body` in the session.
    pub fn send_json(
        &self,
        method: &str,
        path: &str,
        body: &serde_json::Value,
    ) -> anyhow::Result<String> {
        Ok(send(
            &self.agent,
            &self.url,
            method,
            path,
            Some(&self.cookie),
            Some(body),
        )?
        .ok()?
        .body)
    }

    /// The status of a DELETE of `path` in the session.
    pub fn delete(&self, path: &str) -> anyhow::Result<u16> {
        Ok(send(
            &self.agent,
            &self.url,
            "DELETE",
            path,
            Some(&self.cookie),
            None,
        )?
        .status)
    }

    fn post(&self, path: &str, body: Option<&serde_json::Value>) -> anyhow::Result<Answer> {
        send(
            &self.agent,
            &self.url,
            "POST",
            path,
            Some(&self.cookie),
            body,
        )
    }
}

impl Answer {
    /// The answer, if it is a success.
    fn ok(self) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (200..300).contains(&self.status),
            "The API answered {}: {}",
            self.status,
            self.body
        );
        Ok(self)
    }
}

/// A client that reads every answer, a refusal included.
fn agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build(),
    )
}

/// The cookies of the pending sign-in a start answered with, if an email went out: none goes to an
/// address at its cap, so no code ever comes.
fn emailed(started: Answer, email: &str) -> anyhow::Result<String> {
    let JsonSignInEmail { wait, .. } = serde_json::from_str(&started.body)?;
    if let Some(wait) = wait {
        anyhow::bail!("Too many emails to {email}. Try again in {wait} seconds.");
    }
    Ok(started.cookies)
}

/// Start a sign-in by email for `email`.
fn start(agent: &ureq::Agent, url: &Url, email: &str) -> anyhow::Result<Answer> {
    send(
        agent,
        url,
        "POST",
        "/v0/auth/email",
        None,
        Some(&serde_json::json!({ "email": email })),
    )?
    .ok()
}

/// Send `method` to `path` as the console's own origin, with `cookie` and any JSON `body`.
fn send(
    agent: &ureq::Agent,
    url: &Url,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    body: Option<&serde_json::Value>,
) -> anyhow::Result<Answer> {
    let target = format!("{}{path}", url.as_ref().trim_end_matches('/'));
    let mut request = ureq::http::Request::builder()
        .method(method)
        .uri(&target)
        .header("Sec-Fetch-Site", "same-origin");
    if let Some(cookie) = cookie {
        request = request.header("Cookie", cookie);
    }
    let mut response = if let Some(body) = body {
        agent.run(
            request
                .header("Content-Type", "application/json")
                .body(body.to_string())?,
        )?
    } else {
        agent.run(request.body(())?)?
    };
    let status = response.status().as_u16();
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
    let body = response.body_mut().read_to_string()?;
    Ok(Answer {
        status,
        cookies,
        body,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{BufRead as _, BufReader, Read as _, Write as _},
        net::{TcpListener, TcpStream},
        thread,
    };

    use bencher_json::Url;

    use super::{ApiLog, BrowserSession, agent, code, fly_lines, newest_email};

    /// The answer to a sign-in start for an address at its cap, which sends no email.
    const CAPPED: &str = r#"{"email":"courage@nowhere.com","wait":40}"#;

    /// Answer the requests to an API at the URL `client` gets, each in turn with the next status
    /// and JSON body of `answers`, and return what `client` returns. The client may send each
    /// request on the connection it kept from the last or on a new one.
    fn with_api<T>(answers: &[(u16, &str)], client: impl FnOnce(&Url) -> T) -> T {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        thread::scope(|scope| {
            scope.spawn(|| {
                let mut connection = None;
                for (status, body) in answers {
                    loop {
                        let stream = connection
                            .get_or_insert_with(|| BufReader::new(listener.accept().unwrap().0));
                        if read_request(stream) {
                            break;
                        }
                        connection = None;
                    }
                    let stream = connection.as_mut().unwrap().get_mut();
                    write!(
                        stream,
                        "HTTP/1.1 {status} Answered\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\n\r\n{body}",
                        body.len()
                    )
                    .unwrap();
                }
            });
            client(&url)
        })
    }

    /// Read the next request off `stream`, its body whole, or return false if the client closed it
    /// first. A body left unread would make closing the connection reset it.
    fn read_request(stream: &mut BufReader<TcpStream>) -> bool {
        let mut length = 0;
        let mut chunked = false;
        let mut started = false;
        loop {
            let mut line = String::new();
            if stream.read_line(&mut line).unwrap_or_default() == 0 {
                assert!(!started, "The client closed a request before its end");
                return false;
            }
            if line == "\r\n" {
                break;
            }
            started = true;
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap();
            } else if name.eq_ignore_ascii_case("transfer-encoding") {
                chunked = value.trim().eq_ignore_ascii_case("chunked");
            }
        }
        if !chunked {
            stream.read_exact(&mut vec![0; length]).unwrap();
            return true;
        }
        // Each chunk is its size in hex on a line of its own, then its bytes and a line end, up to
        // a chunk of size 0.
        loop {
            let mut size = String::new();
            stream.read_line(&mut size).unwrap();
            let size = usize::from_str_radix(size.trim(), 16).unwrap();
            stream.read_exact(&mut vec![0; size]).unwrap();
            stream.read_line(&mut String::new()).unwrap();
            if size == 0 {
                return true;
            }
        }
    }

    /// Run `client` with an API log that never holds an email.
    fn with_empty_log<T>(name: &str, client: impl FnOnce(ApiLog) -> T) -> T {
        let path = std::env::temp_dir().join(format!("test_api-{}-{name}.log", std::process::id()));
        fs::write(&path, "").unwrap();
        let answer = client(ApiLog::File(path.clone()));
        fs::remove_file(path).unwrap();
        answer
    }

    // Kills a sign-in that waits for the email of a start the cap answered, which fails only once
    // the log's whole wait is out, and then as no email to the address.
    #[test]
    fn a_sign_in_at_the_cap_fails_at_once_with_the_wait() {
        let refused = with_empty_log("sign_in", |api_log| {
            with_api(&[(202, CAPPED)], |url| {
                BrowserSession::sign_in(url, "courage@nowhere.com", &api_log).unwrap_err()
            })
        });
        assert_eq!(
            refused.to_string(),
            "Too many emails to courage@nowhere.com. Try again in 40 seconds."
        );
    }

    // Kills a proof again that waits for the email of a start the cap answered.
    #[test]
    fn a_proof_again_at_the_cap_fails_at_once_with_the_wait() {
        let refused = with_empty_log("prove_again", |api_log| {
            with_api(
                &[(403, r#"{"error_code":"fresh_sign_in"}"#), (202, CAPPED)],
                |url| {
                    let session = BrowserSession {
                        agent: agent(),
                        url: url.clone(),
                        email: "courage@nowhere.com".to_owned(),
                        cookie: "bencher_session=signed-in".to_owned(),
                        api_log,
                        user: serde_json::from_value(serde_json::json!({
                            "uuid": "c0c0c0c0-c0c0-4c0c-8c0c-c0c0c0c0c0c0",
                            "name": "Courage",
                            "slug": "courage",
                            "email": "courage@nowhere.com",
                            "admin": false,
                            "locked": false,
                        }))
                        .unwrap(),
                    };
                    session.create_user_key("courage", "seed").unwrap_err()
                },
            )
        });
        assert_eq!(
            refused.to_string(),
            "Too many emails to courage@nowhere.com. Try again in 40 seconds."
        );
    }

    /// What `flyctl logs --app bencher-api-dev --no-tail --json` prints for an API that logged a
    /// request and sign-in emails to Muriel and to Katz, after a line of Fly's own: each entry as
    /// flyctl's Go encoder writes one, indented and with `<`, `>`, and `&` escaped, around a line
    /// the API logged.
    const FLY_LOGS: &str = r#"{
    "level": "info",
    "instance": "e2865d3ef24d68",
    "message": "INFO Main child exited normally with code: 0",
    "region": "iad",
    "timestamp": "2026-11-14T15:33:29.917243315Z",
    "meta": {
        "Instance": "",
        "Region": "",
        "Event": {
            "Provider": "runner"
        },
        "HTTP": {
            "Request": {
                "ID": "",
                "Method": "",
                "Version": ""
            },
            "Response": {
                "status_code": 0
            }
        },
        "Error": {
            "Code": 0,
            "Message": ""
        },
        "URL": {
            "Full": ""
        }
    }
}
{
    "level": "info",
    "instance": "e2865d3ef24d68",
    "message": "{\"msg\":\"request completed\",\"level\":\"INFO\",\"ts\":\"2026-11-14T15:33:32.447070723Z\",\"uri\":\"/v0/auth/email\",\"method\":\"POST\",\"req_id\":\"98eceb46-c108-428e-9166-033f9ae150f1\",\"remote_addr\":\"127.0.0.1:38502\",\"local_addr\":\"0.0.0.0:6610\",\"latency_us\":1147,\"response_code\":202}",
    "region": "iad",
    "timestamp": "2026-11-14T15:33:32.447812933Z",
    "meta": {
        "Instance": "",
        "Region": "",
        "Event": {
            "Provider": "app"
        },
        "HTTP": {
            "Request": {
                "ID": "",
                "Method": "",
                "Version": ""
            },
            "Response": {
                "status_code": 0
            }
        },
        "Error": {
            "Code": 0,
            "Message": ""
        },
        "URL": {
            "Full": ""
        }
    }
}
{
    "level": "info",
    "instance": "e2865d3ef24d68",
    "message": "{\"msg\":\"\\nTo: \u003cmuriel.bagge@nowhere.com\u003e\\nSubject: Your Bencher code is 612491\\nBody: \\nSign up for Bencher\\n\\n612491\\n\\nSign up: http://localhost:3000/auth/confirm#cMF62iXMk37dPqd54b5uVIhcb6VZEiTZtAR4mb9gLxY\\n\\nThe code and the button work once, in the next 30 minutes.\\nIf you did not ask to sign in, ignore this email.\\n\",\"level\":\"INFO\",\"ts\":\"2026-11-14T15:33:32.456393475Z\",\"uri\":\"/v0/auth/email\",\"method\":\"POST\",\"req_id\":\"fb0ba279-c4ef-4d53-ae8d-0a5e27d0e285\",\"remote_addr\":\"127.0.0.1:38514\",\"local_addr\":\"0.0.0.0:6610\"}",
    "region": "iad",
    "timestamp": "2026-11-14T15:33:32.457025511Z",
    "meta": {
        "Instance": "",
        "Region": "",
        "Event": {
            "Provider": "app"
        },
        "HTTP": {
            "Request": {
                "ID": "",
                "Method": "",
                "Version": ""
            },
            "Response": {
                "status_code": 0
            }
        },
        "Error": {
            "Code": 0,
            "Message": ""
        },
        "URL": {
            "Full": ""
        }
    }
}
{
    "level": "info",
    "instance": "e2865d3ef24d68",
    "message": "{\"msg\":\"\\nTo: \u003ckatz@nowhere.com\u003e\\nSubject: Your Bencher code is 368113\\nBody: \\nSign up for Bencher\\n\\n368113\\n\\nSign up: http://localhost:3000/auth/confirm#PmAUmwmutsvKCVk8Q0-r9xT7XtaClDu-rAKbuOJO9Dk\\n\\nThe code and the button work once, in the next 30 minutes.\\nIf you did not ask to sign in, ignore this email.\\n\",\"level\":\"INFO\",\"ts\":\"2026-11-14T15:33:32.547354736Z\",\"uri\":\"/v0/auth/email\",\"method\":\"POST\",\"req_id\":\"38197080-5a23-4cc4-a182-41ee8dad35aa\",\"remote_addr\":\"127.0.0.1:38584\",\"local_addr\":\"0.0.0.0:6610\"}",
    "region": "iad",
    "timestamp": "2026-11-14T15:33:32.548316250Z",
    "meta": {
        "Instance": "",
        "Region": "",
        "Event": {
            "Provider": "app"
        },
        "HTTP": {
            "Request": {
                "ID": "",
                "Method": "",
                "Version": ""
            },
            "Response": {
                "status_code": 0
            }
        },
        "Error": {
            "Code": 0,
            "Message": ""
        },
        "URL": {
            "Full": ""
        }
    }
}
"#;

    // Kills a reader of Fly's logs that misses an entry, takes its envelope for the API's line, or
    // takes a code from an email to someone else or from a line that is not the API's.
    #[test]
    fn fly_logs_give_the_code_of_the_newest_email_to_the_address() {
        let lines = fly_lines(FLY_LOGS).unwrap();
        assert_eq!(lines.len(), 4);
        assert_eq!(
            lines.first().map(String::as_str),
            Some("INFO Main child exited normally with code: 0")
        );
        assert_eq!(
            newest_email(&lines, "muriel.bagge@nowhere.com", code).as_deref(),
            Some("612491")
        );
        assert_eq!(
            newest_email(&lines, "katz@nowhere.com", code).as_deref(),
            Some("368113")
        );
        assert_eq!(newest_email(&lines, "courage@nowhere.com", code), None);
    }

    // Kills a reader that takes an older email to the address over its newest.
    #[test]
    fn the_newest_email_to_the_address_wins() {
        let line = |code: &str| {
            serde_json::json!({
                "msg": format!(
                    "\nTo: <muriel.bagge@nowhere.com>\nSubject: Your Bencher code is {code}\nBody: Your Bencher code is {code}."
                ),
            })
            .to_string()
        };
        let lines = [line("111111"), line("222222")];
        assert_eq!(
            newest_email(&lines, "muriel.bagge@nowhere.com", code).as_deref(),
            Some("222222")
        );
    }

    // Kills a reader that fails on a log that is not Fly's.
    #[test]
    fn fly_logs_refuse_an_entry_with_no_message() {
        fly_lines(r#"{"level": "info"}"#).unwrap_err();
        fly_lines("not json").unwrap_err();
        assert_eq!(fly_lines("").unwrap(), Vec::<String>::new());
    }
}
