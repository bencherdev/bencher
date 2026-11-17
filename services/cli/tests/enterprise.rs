#![expect(
    unused_crate_dependencies,
    reason = "integration test of the `bencher` binary"
)]
#![cfg(feature = "plus")]
#![expect(
    clippy::tests_outside_test_module,
    reason = "integration test of the `bencher` binary"
)]
//! The CLI against a stand-in API server that refuses what an Enterprise plan does not allow.

use std::{
    io::{BufRead as _, BufReader, Read as _, Write as _},
    net::{SocketAddr, TcpListener, TcpStream},
    process::Command,
    thread,
    time::Duration,
};

const TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.c2lnbmF0dXJl";
const VERSION_PATH: &str = "/v0/server/version";
const ORGANIZATION_PATH: &str = "/v0/organizations/pompeii-llc";
const PLAN_PATH: &str = "/v0/organizations/pompeii-llc/plan";
const MESSAGE: &str = "Refusal marker: contact enterprise@bencher.dev";
const STOP: &str = "STOP";

/// Answer every request until a stop: the API version, and a refusal with [`MESSAGE`] for
/// anything else. Returns the method and path of each request it answered.
fn serve(listener: &TcpListener) -> Vec<(String, String)> {
    let mut requests = Vec::new();
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        let Some((method, path)) = read_request(&stream) else {
            continue;
        };
        if method == STOP {
            break;
        }
        let (status, body) = if path == VERSION_PATH {
            (
                "200 OK",
                serde_json::json!({ "version": env!("CARGO_PKG_VERSION") }),
            )
        } else {
            (
                "403 Forbidden",
                serde_json::json!({ "request_id": "request", "message": MESSAGE }),
            )
        };
        let body = body.to_string();
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        if stream.write_all(response.as_bytes()).is_ok() {
            requests.push((method, path));
        }
    }
    requests
}

fn read_request(stream: &TcpStream) -> Option<(String, String)> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut request_line = line.split_whitespace();
    let method = request_line.next()?.to_owned();
    let target = request_line.next().unwrap_or_default();
    let path = format!("/{}", target.trim_start_matches('/'));
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
    Some((method, path))
}

#[expect(clippy::expect_used, reason = "test helper")]
fn stop(addr: SocketAddr) {
    TcpStream::connect(addr)
        .and_then(|mut stream| stream.write_all(format!("{STOP}\r\n\r\n").as_bytes()))
        .expect("Failed to stop the stand-in server");
}

/// Run `bencher` with `args` against the stand-in, and return its output and the method and path
/// of each request the stand-in answered.
#[expect(clippy::expect_used, reason = "test helper")]
fn bencher(args: &[&str]) -> (std::process::Output, Vec<(String, String)>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind a free port");
    let addr = listener
        .local_addr()
        .expect("Failed to read the bound address");
    thread::scope(|scope| {
        let server = scope.spawn(|| serve(&listener));
        let output = Command::new(env!("CARGO_BIN_EXE_bencher"))
            .env_clear()
            .args(args)
            .args([
                "--host",
                &format!("http://{addr}"),
                "--token",
                TOKEN,
                "--attempts",
                "1",
            ])
            .output()
            .expect("Failed to run `bencher`");
        stop(addr);
        let requests = server.join().expect("The stand-in server panicked");
        (output, requests)
    })
}

// Fails if `bencher plan update` or `bencher organization delete` does not send its request, or
// does not print the API's message when the API refuses it, as it does to cancel or resume an
// Enterprise plan or to delete an organization that holds one.
#[test]
fn enterprise_refusals_print_the_api_message() {
    for (args, request) in [
        (
            ["plan", "update", "pompeii-llc", "--cancel"].as_slice(),
            ("PATCH", PLAN_PATH),
        ),
        (
            ["plan", "update", "pompeii-llc", "--resume"].as_slice(),
            ("PATCH", PLAN_PATH),
        ),
        (
            ["organization", "delete", "pompeii-llc"].as_slice(),
            ("DELETE", ORGANIZATION_PATH),
        ),
    ] {
        let (output, requests) = bencher(args);

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{args:?}: {stderr}");
        assert!(stderr.contains(MESSAGE), "{args:?}: {stderr}");
        assert_eq!(
            requests.last(),
            Some(&(request.0.to_owned(), request.1.to_owned())),
            "{args:?}"
        );
    }
}
