#![expect(
    unused_crate_dependencies,
    reason = "integration test of the `bencher` binary"
)]
#![cfg(feature = "plus")]
#![expect(
    clippy::tests_outside_test_module,
    reason = "integration test of the `bencher` binary"
)]
//! `bencher run` against a stand-in that answers as the API server and as the GitHub API, and records every request.

use std::{
    io::{BufRead as _, BufReader, Read as _, Write as _},
    net::{SocketAddr, TcpListener, TcpStream},
    path::Path,
    process::{Command, Output},
    thread,
    time::Duration,
};

const JOB: &str = "8d2b6c4e-5f3a-4b1c-9e7d-0a1b2c3d4e5f";
const UUID: &str = "4f6d1b2a-3c5e-4d7f-8a9b-0c1d2e3f4a5b";
const TIME: &str = "2026-01-01T00:00:00Z";
const URL: &str =
    "https://user-marker:pass-marker@receiver.example:8443/path-marker/hooks?query=query-marker";
const MARKERS: [&str; 6] = [
    "user-marker",
    "pass-marker",
    "path-marker",
    "query-marker",
    "token-marker",
    "key-marker",
];
const CALLBACK: [&str; 6] = [
    "--callback-url",
    URL,
    "--callback-header",
    "Authorization: Bearer token-marker",
    "--callback-header",
    "X-Key: key-marker",
];
const ECHO: &str = "Bencher New Report:\n";
const SKIPPED: &str = "callback skipped: requires a Bencher Plus plan";
const SUBMITTED: &str = "Remote job submitted successfully";
const JOB_PATH: &str = "/v0/projects/project/jobs/8d2b6c4e-5f3a-4b1c-9e7d-0a1b2c3d4e5f";
const REPORT_PATH: &str = "/v0/projects/project/reports/4f6d1b2a-3c5e-4d7f-8a9b-0c1d2e3f4a5b";
const CONSOLE_PATH: &str = "/v0/server/config/console";
const VERSION_PATH: &str = "/v0/server/version";
// One attempt per request, unless a test is about retries.
const ATTEMPTS: [&str; 2] = ["--attempts", "1"];
const PROJECT: [&str; 2] = ["--project", "project"];
const DETACH: [&str; 3] = ["--image", "alpine:3.18", "--detach"];
const STOP: &str = "STOP";
// Clap's exit status for a usage error.
const USAGE_ERROR: i32 = 2;

// The pull request's head, which the check belongs on.
const HEAD_SHA: &str = "f1e2d3c4b5a697887766554433221100ffeeddcc";
// `GITHUB_SHA`: a push's commit, or the default branch's head on a `repository_dispatch`.
const GITHUB_SHA: &str = "0011223344556677889900aabbccddeeff001122";
// The check a detached run started, which its `repository_dispatch` names.
const CHECK: u64 = 4242;
// The ID of every check the stand-in creates.
const NEW_CHECK: u64 = 5151;
const CHECK_RUNS: &str = "/repos/owner/repo/check-runs";
const GITHUB_TOKEN: &str = "github-token-marker";
const CALLBACK_TOKEN: &str = "callback-token-marker";
const RECEIVER: &str = "https://receiver.example/hooks";

/// How the stand-in answers.
#[derive(Clone, Copy)]
struct StandIn {
    job_read: JobRead,
    version: VersionRead,
    check_runs: CheckRuns,
}

/// How the stand-in answers a new check run.
#[derive(Clone, Copy)]
enum CheckRuns {
    Create,
    /// GitHub's validation error, which `octocrab` does not retry.
    Unprocessable,
}

/// How the stand-in answers the API version check.
#[derive(Clone, Copy)]
enum VersionRead {
    Current,
    ServerError,
}

/// The GitHub Actions environment of a run.
struct GitHub {
    event_name: &'static str,
    event: serde_json::Value,
    api: GitHubApi,
}

/// Where the run finds the GitHub API.
#[derive(Clone, Copy)]
enum GitHubApi {
    /// The stand-in, over `http`, which records every GitHub request.
    StandIn,
    /// An `https` URL that refuses connections: no check starts, and a composed dispatch validates.
    Unreachable,
}

/// How the stand-in answers a read of the job.
#[derive(Clone, Copy)]
enum JobRead {
    Callback(&'static str),
    /// A job without a callback in the given state.
    Status(&'static str),
    NotFound,
    ServerError,
    /// A proxy's error, which is JSON but not the API's.
    GatewayError,
}

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

struct Run {
    output: Output,
    requests: Vec<Request>,
    /// The GitHub API URL the run was given.
    github_api_url: Option<String>,
    /// What the run wrote to `GITHUB_STEP_SUMMARY`.
    step_summary: Option<String>,
}

impl Run {
    fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).into_owned()
    }

    fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).into_owned()
    }

    fn stderr_lines(&self) -> Vec<String> {
        self.stderr().lines().map(ToOwned::to_owned).collect()
    }

    fn requests_to(&self, method: &str, path: &str) -> Vec<&Request> {
        self.requests
            .iter()
            .filter(|request| request.method == method && request.path == path)
            .collect()
    }

    fn job_reads(&self) -> usize {
        self.requests_to("GET", JOB_PATH).len()
    }

    fn position(&self, method: &str, path: &str) -> Option<usize> {
        self.requests
            .iter()
            .position(|request| request.method == method && request.path == path)
    }

    fn assert_success(&self) {
        assert!(self.output.status.success(), "{}", self.stderr());
    }

    fn assert_no_marker(&self) {
        for printed in [self.stdout(), self.stderr()] {
            for marker in MARKERS {
                assert!(!printed.contains(marker), "{marker} in {printed}");
            }
        }
    }

    /// The echo prints `callback` through `sanitize_json`, which the test shares with the binary's build,
    /// so a debug build prints it in full and a release build sanitizes it. Nothing else prints a marker.
    #[expect(clippy::expect_used, reason = "test helper")]
    fn assert_echo(&self, callback: &bencher_json::JsonNewCallback) {
        let stdout = self.stdout();
        let (before, after) = stdout.split_once(ECHO).expect("The run printed no echo");
        let mut values = serde_json::Deserializer::from_str(after).into_iter::<serde_json::Value>();
        let mut echo = values
            .next()
            .expect("The run printed no echo")
            .expect("The echo is not JSON");
        let rest = after
            .get(values.byte_offset()..)
            .expect("The echo ends inside stdout");
        let printed = echo
            .pointer_mut("/job/callback")
            .map(serde_json::Value::take)
            .expect("The echo has no callback");
        assert_eq!(printed, bencher_json::sanitize_json(callback), "{stdout}");
        for printed in [before, &echo.to_string(), rest, &self.stderr()] {
            for marker in MARKERS.iter().chain(&[GITHUB_TOKEN, CALLBACK_TOKEN]) {
                assert!(!printed.contains(marker), "{marker} in {printed}");
            }
        }
    }

    /// The requests to the GitHub API.
    fn github(&self) -> Vec<(&str, &str)> {
        self.requests
            .iter()
            .filter(|request| request.path.starts_with("/repos/"))
            .map(|request| (request.method.as_str(), request.path.as_str()))
            .collect()
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn body(&self, method: &str, path: &str) -> serde_json::Value {
        let requests = self.requests_to(method, path);
        assert_eq!(requests.len(), 1, "one {method} {path}");
        let request = requests.first().expect("one request");
        serde_json::from_str(&request.body).expect("Failed to parse a request body")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn bodies(&self, method: &str, path: &str) -> Vec<serde_json::Value> {
        self.requests_to(method, path)
            .iter()
            .map(|request| {
                serde_json::from_str(&request.body).expect("Failed to parse a request body")
            })
            .collect()
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn sent_callback(&self) -> serde_json::Value {
        self.body("POST", "/v0/run")
            .pointer("/job/callback")
            .cloned()
            .expect("The run carries no callback")
    }
}

/// The callback that `CALLBACK` and `body` describe.
#[expect(clippy::expect_used, reason = "test helper")]
fn callback(body: Option<serde_json::Value>) -> bencher_json::JsonNewCallback {
    bencher_json::JsonNewCallback::new(
        URL,
        [
            ("Authorization", "Bearer token-marker"),
            ("X-Key", "key-marker"),
        ]
        .map(|(name, value)| (name.to_owned(), value.to_owned())),
        body,
    )
    .expect("The callback is valid")
}

/// Run `bencher run` with `args` against the stand-in.
fn bencher_run(job_read: JobRead, args: &[&str]) -> Run {
    bencher_run_with(
        StandIn {
            job_read,
            version: VersionRead::Current,
            check_runs: CheckRuns::Create,
        },
        None,
        args,
    )
}

/// Run `bencher run` with `args` against the stand-in, in the GitHub Actions environment if given.
#[expect(clippy::expect_used, reason = "test helper")]
fn bencher_run_with(stand_in: StandIn, github: Option<&GitHub>, args: &[&str]) -> Run {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind a free port");
    let addr = listener
        .local_addr()
        .expect("Failed to read the bound address");
    // Each test runs in its own process, and the port is unique while it is bound.
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let event_path = tmp.join(format!("event-{}-{}.json", std::process::id(), addr.port()));
    let summary_path = tmp.join(format!("summary-{}-{}.md", std::process::id(), addr.port()));
    let mut command = Command::new(env!("CARGO_BIN_EXE_bencher"));
    command
        .env_clear()
        .args(["run", "--host", &format!("http://{addr}")])
        .args(args);
    let github_api_url = github.map(|github| {
        std::fs::write(&event_path, github.event.to_string()).expect("Failed to write the event");
        let api_url = match github.api {
            GitHubApi::StandIn => format!("http://{addr}"),
            GitHubApi::Unreachable => format!("https://127.0.0.1:{}", refused_port()),
        };
        command
            .env("GITHUB_ACTIONS", "true")
            .env("GITHUB_EVENT_NAME", github.event_name)
            .env("GITHUB_EVENT_PATH", &event_path)
            .env("GITHUB_API_URL", &api_url)
            .env("GITHUB_SHA", GITHUB_SHA)
            .env("GITHUB_STEP_SUMMARY", &summary_path);
        api_url
    });
    thread::scope(|scope| {
        let server = scope.spawn(|| serve(&listener, stand_in));
        let output = command.output();
        stop(addr);
        let requests = server.join().expect("The stand-in server panicked");
        let step_summary = std::fs::read_to_string(&summary_path).ok();
        for path in [&event_path, &summary_path] {
            // A run that writes no summary leaves no file to remove.
            drop(std::fs::remove_file(path));
        }
        Run {
            output: output.expect("Failed to run `bencher`"),
            requests,
            github_api_url,
            step_summary,
        }
    })
}

#[expect(clippy::expect_used, reason = "test helper")]
fn refused_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("Failed to bind a free port")
        .port()
}

/// A run of `--project project` on GitHub Actions against the stand-in, one attempt per request.
fn github_run(job_read: JobRead, github: &GitHub, args: &[&str]) -> Run {
    github_run_with(
        StandIn {
            job_read,
            version: VersionRead::Current,
            check_runs: CheckRuns::Create,
        },
        github,
        args,
    )
}

fn github_run_with(stand_in: StandIn, github: &GitHub, args: &[&str]) -> Run {
    bencher_run_with(
        stand_in,
        Some(github),
        &[
            &ATTEMPTS[..],
            &PROJECT,
            &["--github-actions", GITHUB_TOKEN],
            args,
        ]
        .concat(),
    )
}

/// Workflow 1: a pull request.
fn pull_request(api: GitHubApi) -> GitHub {
    GitHub {
        event_name: "pull_request",
        event: serde_json::json!({
            "number": 7,
            "pull_request": {
                "head": { "sha": HEAD_SHA, "repo": { "full_name": "owner/repo" } }
            },
            "repository": { "full_name": "owner/repo" }
        }),
        api,
    }
}

/// Workflow 1: a push, where `GITHUB_SHA` is the commit.
fn push(api: GitHubApi) -> GitHub {
    GitHub {
        event_name: "push",
        event: serde_json::json!({ "ref": "refs/heads/main", "repository": { "full_name": "owner/repo" } }),
        api,
    }
}

/// Workflow 2: the `repository_dispatch` a detached run's callback sends.
fn dispatch(client_payload: &serde_json::Value) -> GitHub {
    GitHub {
        event_name: "repository_dispatch",
        event: serde_json::json!({
            "action": "bencher_run",
            "branch": "main",
            "client_payload": client_payload,
            "repository": { "full_name": "owner/repo" }
        }),
        api: GitHubApi::StandIn,
    }
}

/// A detached run of `--project project`, one attempt per request.
fn detach(job_read: JobRead, args: &[&str]) -> Run {
    bencher_run(job_read, &[&ATTEMPTS[..], &PROJECT, &DETACH, args].concat())
}

fn submitted() -> String {
    format!("{SUBMITTED}: {JOB}")
}

/// Answer each request in turn until the test sends `STOP`.
fn serve(listener: &TcpListener, stand_in: StandIn) -> Vec<Request> {
    let mut requests = Vec::new();
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        let Some(request) = read_request(&stream) else {
            continue;
        };
        if request.method == STOP {
            break;
        }
        let (status, body) = respond(&request, stand_in);
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        if stream.write_all(response.as_bytes()).is_err() {
            continue;
        }
        requests.push(request);
    }
    requests
}

fn read_request(stream: &TcpStream) -> Option<Request> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut request_line = line.split_whitespace();
    let method = request_line.next()?.to_owned();
    // The API client joins the host URL, which ends in a slash, to a path that starts with one.
    // The GitHub client pages its comment list with a query.
    let target = request_line.next().unwrap_or_default();
    let path = format!(
        "/{}",
        target
            .split_once('?')
            .map_or(target, |(path, _)| path)
            .trim_start_matches('/')
    );
    let mut content_length = 0;
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let (name, value) = (name.to_ascii_lowercase(), value.trim().to_owned());
            if name == "content-length" {
                content_length = value.parse().ok()?;
            }
            headers.push((name, value));
        }
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).ok()?;
    Some(Request {
        method,
        path,
        headers,
        body: String::from_utf8(body).ok()?,
    })
}

#[expect(clippy::expect_used, reason = "test helper")]
fn stop(addr: SocketAddr) {
    TcpStream::connect(addr)
        .and_then(|mut stream| stream.write_all(format!("{STOP}\r\n\r\n").as_bytes()))
        .expect("Failed to stop the stand-in server");
}

fn respond(request: &Request, stand_in: StandIn) -> (&'static str, String) {
    const OK: &str = "200 OK";
    const CREATED: &str = "201 Created";
    const NOT_FOUND: &str = "404 Not Found";
    const SERVER_ERROR: &str = "500 Internal Server Error";
    // The API's error shape, so the client sees an error response rather than an odd payload.
    let error = |status, message| {
        (
            status,
            serde_json::json!({ "request_id": "request", "message": message }).to_string(),
        )
    };
    let not_found = || error(NOT_FOUND, "Not Found");
    let StandIn {
        job_read,
        version,
        check_runs,
    } = stand_in;
    let (method, path) = (request.method.as_str(), request.path.as_str());
    if path.starts_with("/repos/owner/repo/") {
        return respond_github(method, path, check_runs).unwrap_or_else(not_found);
    }
    match (method, path) {
        ("GET", VERSION_PATH) => match version {
            VersionRead::Current => (
                OK,
                serde_json::json!({ "version": env!("CARGO_PKG_VERSION") }).to_string(),
            ),
            VersionRead::ServerError => error(SERVER_ERROR, "Internal Server Error"),
        },
        ("GET", "/v0/projects/project") => (
            OK,
            json_report()
                .get("project")
                .map(ToString::to_string)
                .unwrap_or_default(),
        ),
        ("GET", REPORT_PATH) => (OK, json_report().to_string()),
        ("GET", CONSOLE_PATH) => (
            OK,
            serde_json::json!({ "url": "http://localhost:3000" }).to_string(),
        ),
        ("POST", "/v0/run") => (CREATED, json_report().to_string()),
        ("GET", JOB_PATH) => match job_read {
            JobRead::Callback(state) => (
                OK,
                json_job(Some(serde_json::json!({ "state": state, "status": null }))).to_string(),
            ),
            JobRead::Status(status) => {
                let mut json_job = json_job(None);
                if let Some(json_job) = json_job.as_object_mut() {
                    json_job.insert("status".to_owned(), serde_json::json!(status));
                }
                (OK, json_job.to_string())
            },
            JobRead::NotFound => not_found(),
            JobRead::ServerError => error(SERVER_ERROR, "Internal Server Error"),
            JobRead::GatewayError => (
                "502 Bad Gateway",
                serde_json::json!({ "error": "upstream timeout" }).to_string(),
            ),
        },
        _ => not_found(),
    }
}

/// The GitHub API: check runs and pull request comments.
fn respond_github(
    method: &str,
    path: &str,
    check_runs: CheckRuns,
) -> Option<(&'static str, String)> {
    const OK: &str = "200 OK";
    const CREATED: &str = "201 Created";
    let comments = path
        .strip_prefix("/repos/owner/repo/issues/")
        .and_then(|path| path.strip_suffix("/comments"));
    Some(match (method, path.strip_prefix(CHECK_RUNS), comments) {
        ("POST", Some(""), _) => match check_runs {
            CheckRuns::Create => (CREATED, check_run(NEW_CHECK).to_string()),
            CheckRuns::Unprocessable => (
                "422 Unprocessable Entity",
                serde_json::json!({
                    "message": "Validation Failed",
                    "documentation_url": "https://docs.github.com/rest/checks/runs#create-a-check-run"
                })
                .to_string(),
            ),
        },
        ("PATCH", Some(id), _) => (
            OK,
            check_run(id.trim_start_matches('/').parse().ok()?).to_string(),
        ),
        ("GET", _, Some(_)) => (OK, "[]".to_owned()),
        ("POST", _, Some(_)) => (CREATED, comment().to_string()),
        _ => return None,
    })
}

// Every field `octocrab` requires of a check run.
fn check_run(id: u64) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "node_id": "CR_kwDOAAAAAA",
        "details_url": null,
        "head_sha": HEAD_SHA,
        "url": format!("https://api.github.com/repos/owner/repo/check-runs/{id}"),
        "html_url": null,
        "conclusion": null,
        "output": {
            "title": null,
            "summary": null,
            "text": null,
            "annotations_count": 0,
            "annotations_url": format!("https://api.github.com/repos/owner/repo/check-runs/{id}/annotations")
        },
        "started_at": null,
        "completed_at": null,
        "name": "Bencher Report",
        "pull_requests": []
    })
}

// Every field `octocrab` requires of an issue comment.
fn comment() -> serde_json::Value {
    let user = "https://api.github.com/users/github-actions";
    serde_json::json!({
        "id": 1,
        "node_id": "IC_kwDOAAAAAA",
        "url": "https://api.github.com/repos/owner/repo/issues/comments/1",
        "html_url": "https://github.com/owner/repo/pull/7#issuecomment-1",
        "body": "",
        "user": {
            "login": "github-actions[bot]",
            "id": 1,
            "node_id": "BOT_kgDOAAAAAA",
            "avatar_url": "https://avatars.githubusercontent.com/in/1",
            "gravatar_id": "",
            "url": user,
            "html_url": "https://github.com/apps/github-actions",
            "followers_url": format!("{user}/followers"),
            "following_url": format!("{user}/following"),
            "gists_url": format!("{user}/gists"),
            "starred_url": format!("{user}/starred"),
            "subscriptions_url": format!("{user}/subscriptions"),
            "organizations_url": format!("{user}/orgs"),
            "repos_url": format!("{user}/repos"),
            "events_url": format!("{user}/events"),
            "received_events_url": format!("{user}/received_events"),
            "type": "Bot",
            "site_admin": false
        },
        "created_at": TIME
    })
}

fn json_report() -> serde_json::Value {
    serde_json::json!({
        "uuid": UUID,
        "project": {
            "uuid": UUID,
            "organization": UUID,
            "name": "Project",
            "slug": "project",
            "visibility": "public",
            "bmf_version": 0,
            "created": TIME,
            "modified": TIME
        },
        "branch": {
            "uuid": UUID,
            "project": UUID,
            "name": "main",
            "slug": "main",
            "head": { "uuid": UUID, "version": { "number": 1 }, "created": TIME },
            "created": TIME,
            "modified": TIME
        },
        "testbed": {
            "uuid": UUID,
            "project": UUID,
            "name": "base",
            "slug": "base",
            "created": TIME,
            "modified": TIME
        },
        "start_time": TIME,
        "end_time": TIME,
        "adapter": "magic",
        "results": [],
        "job": JOB,
        "created": TIME
    })
}

/// A finished job, so a run that waits for it stops after one poll rather than hanging a test.
fn json_job(callback: Option<serde_json::Value>) -> serde_json::Value {
    let mut json_job = serde_json::json!({
        "uuid": JOB,
        "report": UUID,
        "status": "processed",
        "spec": {
            "uuid": UUID,
            "name": "Test Spec",
            "slug": "test-spec",
            "os": "linux",
            "architecture": "x86_64",
            "cpu": 2,
            "memory": 4096,
            "disk": 8192,
            "network": false,
            "created": TIME,
            "modified": TIME
        },
        "timeout": 60,
        "created": TIME,
        "modified": TIME
    });
    if let (Some(callback), Some(json_job)) = (callback, json_job.as_object_mut()) {
        json_job.insert("callback".to_owned(), callback);
    }
    json_job
}

#[test]
fn detached_callback_is_sent_whole_and_echoed_through_sanitize_json() {
    const BODY: &str = r#"{"outcome":"{{ job.status }}"}"#;
    let args = CALLBACK
        .iter()
        .chain(&["--callback-body", BODY])
        .copied()
        .collect::<Vec<_>>();
    let run = detach(JobRead::Callback("pending"), &args);
    run.assert_success();

    let posts = run.requests_to("POST", "/v0/run");
    let [post] = posts.as_slice() else {
        panic!("one run must be sent, not {}", posts.len());
    };
    let sent: serde_json::Value = serde_json::from_str(&post.body).unwrap();
    assert_eq!(sent["job"]["callback"]["url"], URL);
    assert_eq!(
        sent["job"]["callback"]["headers"],
        serde_json::json!({
            "authorization": "Bearer token-marker",
            "x-key": "key-marker",
        })
    );
    let body = serde_json::json!({ "outcome": "{{ job.status }}" });
    assert_eq!(sent["job"]["callback"]["body"], body);

    run.assert_echo(&callback(Some(body)));

    assert_eq!(run.job_reads(), 1);
    let stderr = run.stderr();
    assert!(stderr.contains(SUBMITTED), "{stderr}");
    assert!(!stderr.contains(SKIPPED), "{stderr}");
}

#[test]
fn skipped_callback_prints_the_notice() {
    let run = detach(JobRead::Callback("skipped"), &CALLBACK);
    run.assert_success();
    run.assert_echo(&callback(None));
    let lines = run.stderr_lines();
    let submitted = lines
        .iter()
        .position(|line| line.starts_with(SUBMITTED))
        .unwrap();
    assert_eq!(
        lines.get(submitted + 1).map(String::as_str),
        Some(SKIPPED),
        "{lines:#?}"
    );
    assert_eq!(
        lines.iter().filter(|line| line.as_str() == SKIPPED).count(),
        1
    );
    // Before the report, which consumes the check.
    assert!(
        run.position("GET", JOB_PATH).unwrap() < run.position("GET", CONSOLE_PATH).unwrap(),
        "the job must be read before the report is displayed"
    );

    let quiet = CALLBACK
        .iter()
        .chain(&["--quiet"])
        .copied()
        .collect::<Vec<_>>();
    let run = detach(JobRead::Callback("skipped"), &quiet);
    run.assert_success();
    assert!(!run.stderr().contains(SKIPPED), "{}", run.stderr());
}

#[test]
fn failed_job_read_prints_nothing_extra() {
    let run = detach(JobRead::NotFound, &CALLBACK);
    run.assert_success();
    assert_eq!(run.job_reads(), 1);
    assert_eq!(run.stderr_lines(), [submitted()]);
    assert_eq!(run.requests_to("POST", "/v0/run").len(), 1);
}

#[test]
fn job_read_uses_the_report_project() {
    // The project comes from the image, so the run has no `--project`.
    let args = [
        &ATTEMPTS[..],
        &["--image", "project:v1", "--detach"],
        &CALLBACK,
    ]
    .concat();
    let run = bencher_run(JobRead::Callback("skipped"), &args);
    run.assert_success();
    assert_eq!(run.job_reads(), 1);
    assert_eq!(run.stderr_lines(), [submitted(), SKIPPED.to_owned()]);
}

#[test]
fn detach_without_callback_reads_no_job() {
    let run = detach(JobRead::Callback("skipped"), &[]);
    run.assert_success();
    assert_eq!(run.job_reads(), 0);
    assert!(!run.stderr().contains(SKIPPED), "{}", run.stderr());
    let posts = run.requests_to("POST", "/v0/run");
    let [post] = posts.as_slice() else {
        panic!("one run must be sent, not {}", posts.len());
    };
    let sent: serde_json::Value = serde_json::from_str(&post.body).unwrap();
    assert!(sent["job"].get("callback").is_none(), "{sent}");
}

#[test]
fn invalid_callback_sends_nothing() {
    for (args, expected) in [
        (
            &["--callback-url", "http://receiver.example/path-marker"][..],
            "callback URL must use https",
        ),
        (
            &[
                "--callback-url",
                URL,
                "--callback-header",
                "Authorization Bearer token-marker",
            ][..],
            "`--callback-header` 1",
        ),
        (
            &[
                "--callback-url",
                URL,
                "--callback-header",
                "X Key: key-marker",
            ][..],
            "invalid name for callback header 1",
        ),
        (
            &[
                "--callback-url",
                URL,
                "--callback-body",
                r#"{"job":"{{ job.id }}"}"#,
            ][..],
            r#"callback body placeholder at "/job" names an unknown value"#,
        ),
    ] {
        let run = detach(JobRead::Callback("pending"), args);
        let stderr = run.stderr();
        assert_eq!(run.output.status.code(), Some(1), "{stderr}");
        assert!(stderr.contains(expected), "{stderr}");
        run.assert_no_marker();
        assert!(run.requests.is_empty(), "{args:?} sent a request");
    }
}

#[test]
fn usage_errors_print_no_secret() {
    for args in [
        &[
            "--image",
            "alpine:3.18",
            "--detach",
            "--callback-header",
            "Authorization: Bearer token-marker",
        ][..],
        &["--image", "alpine:3.18", "--callback-url", URL][..],
        &[
            "--job",
            JOB,
            "--callback-header",
            "Authorization: Bearer token-marker",
        ][..],
        &["--job", JOB, "--callback-url", URL][..],
    ] {
        let run = bencher_run(
            JobRead::Callback("pending"),
            &[&ATTEMPTS[..], &PROJECT, args].concat(),
        );
        assert_eq!(
            run.output.status.code(),
            Some(USAGE_ERROR),
            "{}",
            run.stderr()
        );
        run.assert_no_marker();
        assert!(run.requests.is_empty(), "{args:?} sent a request");
    }
}

/// The `client_payload` the server would send, rendered through the same code it uses.
#[expect(clippy::expect_used, reason = "test helper")]
fn rendered_payload(callback: &serde_json::Value) -> serde_json::Value {
    let callback: bencher_json::JsonNewCallback =
        serde_json::from_value(callback.clone()).expect("The composed callback must validate");
    let context = bencher_json::CallbackContext {
        project_uuid: UUID.parse().expect("project"),
        project_slug: "project".parse().expect("slug"),
        report_uuid: UUID.parse().expect("report"),
        job_uuid: JOB.parse().expect("job"),
        job_status: bencher_json::JobStatus::Processed,
    };
    let body = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime")
        .block_on(callback.render(&context, || async {
            Err::<bencher_json::JsonReport, _>("the composed body never sends the report")
        }))
        .expect("render");
    let body: serde_json::Value =
        serde_json::from_str(&body).expect("The rendered body must be JSON");
    assert_eq!(
        body.get("event_type"),
        Some(&serde_json::json!("bencher_run")),
        "{body}"
    );
    body.get("client_payload")
        .cloned()
        .expect("The body has no client_payload")
}

fn assert_token_only_in_the_run(run: &Run) {
    for request in &run.requests {
        let sent_in_the_run = request.method == "POST" && request.path == "/v0/run";
        for (name, value) in &request.headers {
            assert!(
                !value.contains(CALLBACK_TOKEN),
                "{name} of {} {}",
                request.method,
                request.path
            );
        }
        assert_eq!(
            request.body.contains(CALLBACK_TOKEN),
            sent_in_the_run,
            "{} {}",
            request.method,
            request.path
        );
    }
}

#[test]
fn detach_composes_the_repository_dispatch() {
    let args = [
        &DETACH[..],
        &["--ci-callback-token", CALLBACK_TOKEN, "--ci-id", "suite"],
    ]
    .concat();
    let run = github_run(
        JobRead::Callback("pending"),
        &pull_request(GitHubApi::Unreachable),
        &args,
    );
    run.assert_success();

    let api_url = run.github_api_url.clone().unwrap();
    let callback = run.sent_callback();
    assert_eq!(
        callback["url"],
        format!("{api_url}/repos/owner/repo/dispatches")
    );
    assert_eq!(
        callback["headers"],
        serde_json::json!({
            "accept": "application/vnd.github+json",
            "authorization": format!("Bearer {CALLBACK_TOKEN}"),
            "x-github-api-version": "2022-11-28",
        })
    );
    // The check could not start, so the payload names none.
    assert_eq!(
        rendered_payload(&callback),
        serde_json::json!({
            "job": JOB,
            "project": "project",
            "number": 7,
            "sha": HEAD_SHA,
            "ci_id": "suite",
        })
    );
    assert_token_only_in_the_run(&run);

    // The echo shows the composed request.
    run.assert_echo(&serde_json::from_value(callback).unwrap());
    // The one read after submit follows the composed callback.
    assert_eq!(run.job_reads(), 1);
    assert_eq!(run.step_summary, None);
}

#[test]
fn detach_on_a_push_composes_no_number() {
    let args = [&DETACH[..], &["--ci-callback-token", CALLBACK_TOKEN]].concat();
    let run = github_run(
        JobRead::Callback("pending"),
        &push(GitHubApi::Unreachable),
        &args,
    );
    run.assert_success();
    assert_eq!(
        rendered_payload(&run.sent_callback()),
        serde_json::json!({ "job": JOB, "project": "project", "sha": GITHUB_SHA })
    );
}

#[test]
fn detach_without_a_callback_is_refused() {
    let run = github_run(
        JobRead::Callback("pending"),
        &pull_request(GitHubApi::StandIn),
        &DETACH,
    );
    assert_eq!(
        run.output.status.code(),
        Some(USAGE_ERROR),
        "{}",
        run.stderr()
    );
    let stderr = run.stderr();
    assert!(
        stderr.contains("`--ci-callback-token`") && stderr.contains("`--callback-url`"),
        "{stderr}"
    );
    run.assert_no_marker();
    assert!(run.requests.is_empty());
}

#[test]
fn detach_starts_the_check_and_posts_nothing_else() {
    // An explicit `--callback-url` replaces the composed dispatch, even beside a token.
    let args = [
        &DETACH[..],
        &[
            "--callback-url",
            RECEIVER,
            "--ci-callback-token",
            CALLBACK_TOKEN,
        ],
    ]
    .concat();
    for state in ["pending", "delivered", "failed"] {
        let run = github_run(
            JobRead::Callback(state),
            &pull_request(GitHubApi::StandIn),
            &args,
        );
        run.assert_success();
        run.assert_no_marker();
        assert_eq!(run.sent_callback()["url"], RECEIVER);
        assert!(
            !run.requests_to("POST", "/v0/run")[0]
                .body
                .contains(CALLBACK_TOKEN)
        );
        // The check starts in progress on the pull request's head, and nothing completes or fails it.
        assert_eq!(run.github(), [("POST", CHECK_RUNS)], "{state}");
        let start = run.body("POST", CHECK_RUNS);
        assert_eq!(start["status"], "in_progress");
        assert_eq!(start["head_sha"], HEAD_SHA);
        assert_eq!(run.step_summary, None);
        assert_eq!(run.stderr_lines().last(), Some(&submitted()), "{state}");
    }
}

#[test]
fn unfired_callback_completes_the_check_as_neutral() {
    let check = format!("{CHECK_RUNS}/{NEW_CHECK}");
    let args = [&DETACH[..], &["--callback-url", RECEIVER]].concat();
    let run = github_run(
        JobRead::Callback("skipped"),
        &pull_request(GitHubApi::StandIn),
        &args,
    );
    run.assert_success();
    assert_eq!(
        run.github(),
        [("POST", CHECK_RUNS), ("PATCH", check.as_str())]
    );
    let complete = run.body("PATCH", &check);
    assert_eq!(complete["conclusion"], "neutral");
    assert_eq!(complete["name"], "Bencher Report (Project)");
    let summary = complete["output"]["summary"].as_str().unwrap();
    assert!(summary.contains(SKIPPED), "{summary}");
    assert_eq!(run.step_summary, None);
    assert!(
        run.stderr_lines().contains(&SKIPPED.to_owned()),
        "{}",
        run.stderr()
    );
}

#[test]
fn unread_callback_leaves_the_check_in_progress() {
    let args = [&DETACH[..], &["--callback-url", RECEIVER]].concat();
    for job_read in [JobRead::ServerError, JobRead::GatewayError] {
        let run = github_run(job_read, &pull_request(GitHubApi::StandIn), &args);
        run.assert_success();
        assert_eq!(run.github(), [("POST", CHECK_RUNS)]);
        let warnings = run
            .stderr_lines()
            .into_iter()
            .filter(|line| line.starts_with("Warning: failed to read the callback state"))
            .count();
        assert_eq!(warnings, 1, "{}", run.stderr());
    }
}

fn attach(github: &GitHub, args: &[&str]) -> Run {
    github_run(
        JobRead::Callback("delivered"),
        github,
        &[&["--job", JOB][..], args].concat(),
    )
}

/// The payload a detached run composes, with the given optional fields.
fn payload(fields: &serde_json::Value) -> serde_json::Value {
    let mut payload = serde_json::json!({ "job": JOB, "project": "project", "sha": HEAD_SHA });
    if let (Some(payload), Some(fields)) = (payload.as_object_mut(), fields.as_object()) {
        payload.extend(fields.clone());
    }
    payload
}

fn comment_tag(id: &str) -> String {
    format!(r#"<div id="bencher.dev/projects/project/id/{id}"></div>"#)
}

#[test]
fn attach_completes_the_dispatched_check() {
    let run = attach(
        &dispatch(&payload(
            &serde_json::json!({ "number": 7, "check": CHECK, "ci_id": "suite" }),
        )),
        &[],
    );
    run.assert_success();
    run.assert_no_marker();
    let check = format!("{CHECK_RUNS}/{CHECK}");
    // No new check, and none at `GITHUB_SHA`.
    assert_eq!(
        run.github(),
        [
            ("PATCH", check.as_str()),
            ("GET", "/repos/owner/repo/issues/7/comments"),
            ("POST", "/repos/owner/repo/issues/7/comments"),
        ]
    );
    let complete = run.body("PATCH", &check);
    assert_eq!(complete["conclusion"], "success");
    assert_eq!(complete["name"], "Bencher Report (suite)");
    let comment = run.body("POST", "/repos/owner/repo/issues/7/comments");
    assert!(
        comment["body"]
            .as_str()
            .unwrap()
            .ends_with(&comment_tag("suite")),
        "{comment}"
    );
    assert!(
        run.step_summary
            .is_some_and(|summary| summary.contains("Bencher Report"))
    );
    for request in &run.requests {
        assert!(!request.body.contains(GITHUB_SHA), "{}", request.path);
    }
}

#[test]
fn attach_creates_the_check_at_the_dispatched_sha() {
    let run = attach(
        &dispatch(&payload(&serde_json::json!({ "number": 7 }))),
        &[],
    );
    run.assert_success();
    let check = format!("{CHECK_RUNS}/{NEW_CHECK}");
    assert_eq!(
        run.github(),
        [
            ("POST", CHECK_RUNS),
            ("PATCH", check.as_str()),
            ("GET", "/repos/owner/repo/issues/7/comments"),
            ("POST", "/repos/owner/repo/issues/7/comments"),
        ]
    );
    assert_eq!(run.body("POST", CHECK_RUNS)["head_sha"], HEAD_SHA);
    assert_eq!(run.body("PATCH", &check)["conclusion"], "success");
}

#[test]
fn attach_without_a_number_posts_no_comment() {
    let run = attach(
        &dispatch(&payload(&serde_json::json!({ "check": CHECK }))),
        &[],
    );
    run.assert_success();
    let check = format!("{CHECK_RUNS}/{CHECK}");
    assert_eq!(run.github(), [("PATCH", check.as_str())]);
    assert_eq!(run.body("PATCH", &check)["conclusion"], "success");
    assert!(
        run.stdout().lines().any(|line| line
            == "The `repository_dispatch` payload has no pull request number and `--ci-number` was not set. Skipping PR comment."),
        "{}",
        run.stdout()
    );
}

#[test]
fn explicit_ci_number_and_ci_id_win_over_the_payload() {
    let run = attach(
        &dispatch(&payload(
            &serde_json::json!({ "number": 7, "check": CHECK, "ci_id": "suite" }),
        )),
        &["--ci-number", "9", "--ci-id", "other"],
    );
    run.assert_success();
    let check = format!("{CHECK_RUNS}/{CHECK}");
    assert_eq!(
        run.github(),
        [
            ("PATCH", check.as_str()),
            ("GET", "/repos/owner/repo/issues/9/comments"),
            ("POST", "/repos/owner/repo/issues/9/comments"),
        ]
    );
    assert_eq!(run.body("PATCH", &check)["name"], "Bencher Report (other)");
    let comment = run.body("POST", "/repos/owner/repo/issues/9/comments");
    assert!(
        comment["body"]
            .as_str()
            .unwrap()
            .ends_with(&comment_tag("other")),
        "{comment}"
    );
}

#[test]
fn malformed_payload_falls_back_with_a_warning() {
    for client_payload in [
        serde_json::json!({ "job": JOB, "sha": 7, "number": 7, "check": CHECK }),
        serde_json::json!("not an object"),
    ] {
        let run = attach(&dispatch(&client_payload), &[]);
        run.assert_success();
        let stderr = run.stderr();
        assert!(
            stderr.contains("Warning: failed to read the `repository_dispatch` payload"),
            "{stderr}"
        );
        // Today's attach: a new check at `GITHUB_SHA`, and no comment without `--ci-number`.
        let stdout = run.stdout();
        assert!(
            stdout.contains("Not running as a GitHub Action pull request event")
                && !stdout.contains("The `repository_dispatch` payload has no pull request number"),
            "{stdout}"
        );
        let check = format!("{CHECK_RUNS}/{NEW_CHECK}");
        assert_eq!(
            run.github(),
            [("POST", CHECK_RUNS), ("PATCH", check.as_str())]
        );
        assert_eq!(run.body("POST", CHECK_RUNS)["head_sha"], GITHUB_SHA);
    }
}

#[test]
fn attach_fails_the_dispatched_check_when_the_version_check_fails() {
    let run = bencher_run_with(
        StandIn {
            job_read: JobRead::Callback("delivered"),
            version: VersionRead::ServerError,
            check_runs: CheckRuns::Create,
        },
        Some(&dispatch(&payload(
            &serde_json::json!({ "number": 7, "check": CHECK }),
        ))),
        &[
            &ATTEMPTS[..],
            &PROJECT,
            &["--github-actions", GITHUB_TOKEN, "--job", JOB],
        ]
        .concat(),
    );
    assert_eq!(run.output.status.code(), Some(1), "{}", run.stderr());
    let check = format!("{CHECK_RUNS}/{CHECK}");
    assert_eq!(run.github(), [("PATCH", check.as_str())]);
    assert_eq!(run.body("PATCH", &check)["conclusion"], "failure");
    assert_eq!(run.job_reads(), 0);
}

#[test]
fn attach_on_another_event_reads_no_payload() {
    let run = attach(&pull_request(GitHubApi::StandIn), &[]);
    run.assert_success();
    assert!(
        !run.stderr().contains("repository_dispatch"),
        "{}",
        run.stderr()
    );
    let check = format!("{CHECK_RUNS}/{NEW_CHECK}");
    assert_eq!(
        run.github(),
        [
            ("POST", CHECK_RUNS),
            ("PATCH", check.as_str()),
            ("GET", "/repos/owner/repo/issues/7/comments"),
            ("POST", "/repos/owner/repo/issues/7/comments"),
        ]
    );
    assert_eq!(run.body("POST", CHECK_RUNS)["head_sha"], HEAD_SHA);
}

#[test]
fn callback_token_outside_github_actions_composes_nothing() {
    let run = detach(
        JobRead::Callback("pending"),
        &[
            "--github-actions",
            GITHUB_TOKEN,
            "--ci-callback-token",
            CALLBACK_TOKEN,
        ],
    );
    run.assert_success();
    run.assert_no_marker();
    assert!(
        run.stdout().contains("Not running as a GitHub Action"),
        "{}",
        run.stdout()
    );
    let sent = run.body("POST", "/v0/run");
    assert!(sent["job"].get("callback").is_none(), "{sent}");
    assert_eq!(run.job_reads(), 0);
}

#[test]
fn only_the_attach_reads_the_payload() {
    // A detached run that a `repository_dispatch` happens to trigger starts its own check.
    let args = [&DETACH[..], &["--callback-url", RECEIVER]].concat();
    let run = github_run(
        JobRead::Callback("pending"),
        &dispatch(&payload(
            &serde_json::json!({ "number": 7, "check": CHECK }),
        )),
        &args,
    );
    run.assert_success();
    assert_eq!(run.github(), [("POST", CHECK_RUNS)]);
    assert_eq!(run.body("POST", CHECK_RUNS)["head_sha"], GITHUB_SHA);
}

#[test]
fn attach_fails_the_dispatched_check_when_the_job_fails() {
    let check = format!("{CHECK_RUNS}/{CHECK}");
    for (status, conclusion, message) in [
        ("failed", "failure", "Remote job failed"),
        ("canceled", "cancelled", "Remote job was canceled"),
    ] {
        let run = github_run(
            JobRead::Status(status),
            &dispatch(&payload(
                &serde_json::json!({ "number": 7, "check": CHECK }),
            )),
            &["--job", JOB],
        );
        assert_eq!(
            run.output.status.code(),
            Some(1),
            "{status}: {}",
            run.stderr()
        );
        assert!(run.stderr().contains(message), "{}", run.stderr());
        // The report still posts: the check completes, then the comment.
        assert_eq!(
            run.github(),
            [
                ("PATCH", check.as_str()),
                ("GET", "/repos/owner/repo/issues/7/comments"),
                ("POST", "/repos/owner/repo/issues/7/comments"),
            ],
            "{status}"
        );
        let complete = run.body("PATCH", &check);
        assert_eq!(complete["conclusion"], conclusion, "{status}");
        assert_eq!(complete["name"], "Bencher Report (Project)", "{status}");
        assert!(run.stdout().contains("View report:"), "{}", run.stdout());
        assert!(run.step_summary.is_some(), "{status}");
    }
}

#[test]
fn attach_creates_the_check_at_the_dispatched_sha_when_the_start_fails() {
    for (job_read, conclusion, code) in [
        (JobRead::Status("processed"), "success", 0),
        (JobRead::Status("failed"), "failure", 1),
    ] {
        let run = github_run_with(
            StandIn {
                job_read,
                version: VersionRead::Current,
                check_runs: CheckRuns::Unprocessable,
            },
            &dispatch(&payload(&serde_json::json!({ "number": 7 }))),
            &["--job", JOB],
        );
        assert_eq!(run.output.status.code(), Some(code), "{}", run.stderr());
        let creates = run.bodies("POST", CHECK_RUNS);
        assert_eq!(creates.len(), 2, "{creates:?}");
        for create in &creates {
            assert_eq!(create["head_sha"], HEAD_SHA, "{create}");
            assert!(!create.to_string().contains(GITHUB_SHA), "{create}");
        }
        assert_eq!(creates[0]["status"], "in_progress");
        assert!(creates[0].get("conclusion").is_none(), "{}", creates[0]);
        assert_eq!(creates[1]["conclusion"], conclusion);
    }
}

#[test]
fn attached_run_fails_its_check_when_the_job_fails() {
    let run = github_run(
        JobRead::Status("failed"),
        &pull_request(GitHubApi::StandIn),
        &["--image", "alpine:3.18", "--job-poll-interval", "1"],
    );
    assert_eq!(run.output.status.code(), Some(1), "{}", run.stderr());
    let check = format!("{CHECK_RUNS}/{NEW_CHECK}");
    assert_eq!(
        run.github().get(..2),
        Some(&[("POST", CHECK_RUNS), ("PATCH", check.as_str())][..]),
        "{:?}",
        run.github()
    );
    assert_eq!(run.body("PATCH", &check)["conclusion"], "failure");
}

#[test]
fn composition_failure_fails_the_check_and_submits_nothing() {
    // The stand-in's GitHub API is `http`, so the composed dispatch URL is refused.
    let run = github_run(
        JobRead::Callback("pending"),
        &pull_request(GitHubApi::StandIn),
        &[&DETACH[..], &["--ci-callback-token", CALLBACK_TOKEN]].concat(),
    );
    assert_eq!(run.output.status.code(), Some(1), "{}", run.stderr());
    let stderr = run.stderr();
    assert!(
        stderr.contains("Failed to compose the GitHub `repository_dispatch` callback")
            && stderr.contains("https"),
        "{stderr}"
    );
    assert!(run.requests_to("POST", "/v0/run").is_empty());
    assert_eq!(run.job_reads(), 0);
    let check = format!("{CHECK_RUNS}/{NEW_CHECK}");
    assert_eq!(
        run.github(),
        [("POST", CHECK_RUNS), ("PATCH", check.as_str())]
    );
    assert_eq!(run.body("PATCH", &check)["conclusion"], "failure");
    run.assert_no_marker();
}
