#![expect(
    unused_crate_dependencies,
    reason = "integration test of the `bencher` binary"
)]
#![cfg(feature = "plus")]
#![expect(
    clippy::tests_outside_test_module,
    reason = "integration test of the `bencher` binary"
)]
//! `bencher run` against a stand-in API server that answers a detached run and records its requests.

use std::{
    io::{BufRead as _, BufReader, Read as _, Write as _},
    net::{SocketAddr, TcpListener, TcpStream},
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
const CONSOLE_PATH: &str = "/v0/server/config/console";
const VERSION_PATH: &str = "/v0/server/version";
// One attempt per request, unless a test is about retries.
const ATTEMPTS: [&str; 2] = ["--attempts", "1"];
const PROJECT: [&str; 2] = ["--project", "project"];
const DETACH: [&str; 3] = ["--image", "alpine:3.18", "--detach"];
const STOP: &str = "STOP";
// Clap's exit status for a usage error.
const USAGE_ERROR: i32 = 2;

/// How the stand-in answers a read of the job.
#[derive(Clone, Copy)]
enum JobRead {
    Callback(&'static str),
    NotFound,
}

struct Request {
    method: String,
    path: String,
    body: String,
}

struct Run {
    output: Output,
    requests: Vec<Request>,
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
            for marker in MARKERS {
                assert!(!printed.contains(marker), "{marker} in {printed}");
            }
        }
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
#[expect(clippy::expect_used, reason = "test helper")]
fn bencher_run(job_read: JobRead, args: &[&str]) -> Run {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind a free port");
    let addr = listener
        .local_addr()
        .expect("Failed to read the bound address");
    thread::scope(|scope| {
        let server = scope.spawn(|| serve(&listener, job_read));
        let output = Command::new(env!("CARGO_BIN_EXE_bencher"))
            .env_clear()
            .args(["run", "--host", &format!("http://{addr}")])
            .args(args)
            .output();
        stop(addr);
        let requests = server.join().expect("The stand-in server panicked");
        Run {
            output: output.expect("Failed to run `bencher`"),
            requests,
        }
    })
}

/// A detached run of `--project project`, one attempt per request.
fn detach(job_read: JobRead, args: &[&str]) -> Run {
    bencher_run(job_read, &[&ATTEMPTS[..], &PROJECT, &DETACH, args].concat())
}

fn submitted() -> String {
    format!("{SUBMITTED}: {JOB}")
}

/// Answer each request in turn until the test sends `STOP`.
fn serve(listener: &TcpListener, job_read: JobRead) -> Vec<Request> {
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
        let (status, body) = respond(&request, job_read);
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
    let path = format!(
        "/{}",
        request_line
            .next()
            .unwrap_or_default()
            .trim_start_matches('/')
    );
    let mut content_length = 0;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().ok()?;
        }
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).ok()?;
    Some(Request {
        method,
        path,
        body: String::from_utf8(body).ok()?,
    })
}

#[expect(clippy::expect_used, reason = "test helper")]
fn stop(addr: SocketAddr) {
    TcpStream::connect(addr)
        .and_then(|mut stream| stream.write_all(format!("{STOP}\r\n\r\n").as_bytes()))
        .expect("Failed to stop the stand-in server");
}

fn respond(request: &Request, job_read: JobRead) -> (&'static str, String) {
    const OK: &str = "200 OK";
    const CREATED: &str = "201 Created";
    const NOT_FOUND: &str = "404 Not Found";
    // The API's error shape, so the client sees an error response rather than an odd payload.
    let error = |status, message| {
        (
            status,
            serde_json::json!({ "request_id": "request", "message": message }).to_string(),
        )
    };
    let not_found = || error(NOT_FOUND, "Not Found");
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", VERSION_PATH) => (
            OK,
            serde_json::json!({ "version": env!("CARGO_PKG_VERSION") }).to_string(),
        ),
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
            JobRead::NotFound => not_found(),
        },
        _ => not_found(),
    }
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
