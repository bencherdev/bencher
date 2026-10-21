//! A stand-in for GitHub, for this crate's tests and, behind `test-base-url`, for the API's.
#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "test support, where any error fails the test"
)]

use std::{collections::HashMap, sync::Arc, time::Duration};

use aws_lc_rs::{
    encoding::{AsDer as _, Pkcs8V1Der},
    rsa::{KeyPair, KeySize},
    signature::KeyPair as _,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use bencher_valid::Secret;
use rustls::crypto::aws_lc_rs as rustls_aws_lc_rs;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::Notify,
    task::JoinHandle,
};
use url::Url;

use crate::api::TIMEOUT;

/// A loopback server that answers each request with the next scripted status and JSON body.
pub struct Server {
    base: Url,
    task: JoinHandle<Vec<Received>>,
    held: Arc<Notify>,
    release: Arc<Notify>,
}

/// One request as the server read it off the wire.
pub struct Received {
    head: String,
    body: Vec<u8>,
}

/// A scripted answer: its status, its headers, and its JSON body.
pub type Answer = (u16, Vec<(&'static str, String)>, String);

/// A scripted status that reads the request and answers nothing until the client has timed out.
pub const NO_ANSWER: u16 = 0;
/// A scripted header, never sent, that holds its answer from the moment its request arrives until
/// the test calls `release`.
pub const HOLD: &str = "x-test-hold";

impl Server {
    pub async fn start(responses: Vec<(u16, String)>) -> Self {
        Self::start_with_headers(
            responses
                .into_iter()
                .map(|(status, body)| (status, Vec::new(), body))
                .collect(),
        )
        .await
    }

    /// Each response with these headers too. A redirect without a `location` points back at this
    /// server, so a client that follows it sends a request that nothing answers.
    pub async fn start_with_headers(responses: Vec<Answer>) -> Self {
        install_crypto_provider();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base: Url = format!("http://{}/github", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        let redirected = format!("{base}/redirected");
        let held = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let (task_held, task_release) = (Arc::clone(&held), Arc::clone(&release));
        let task = tokio::spawn(async move {
            let mut received = Vec::new();
            for (status, mut headers, body) in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                received.push(read_request(&mut stream).await);
                if headers.iter().any(|(name, _)| *name == HOLD) {
                    headers.retain(|(name, _)| *name != HOLD);
                    task_held.notify_one();
                    task_release.notified().await;
                }
                if status == NO_ANSWER {
                    tokio::time::sleep(TIMEOUT + Duration::from_secs(1)).await;
                    continue;
                }
                if (300..400).contains(&status)
                    && !headers.iter().any(|(name, _)| *name == "location")
                {
                    headers.push(("location", redirected.clone()));
                }
                let headers = headers
                    .iter()
                    .fold(String::new(), |mut lines, (name, value)| {
                        lines.push_str(name);
                        lines.push_str(": ");
                        lines.push_str(value);
                        lines.push_str("\r\n");
                        lines
                    });
                let response = format!(
                    "HTTP/1.1 {status} Status\r\ncontent-type: application/json\r\ncontent-length: {}\r\n{headers}connection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
                stream.shutdown().await.unwrap();
            }
            received
        });
        Self {
            base,
            task,
            held,
            release,
        }
    }

    /// Wait until a held answer's request has arrived.
    pub async fn until_held(&self) {
        tokio::time::timeout(Duration::from_secs(10), self.held.notified())
            .await
            .expect("no request reached a held answer");
    }

    /// Send the held answer.
    pub fn release(&self) {
        self.release.notify_one();
    }

    /// The base ends in a path with no trailing slash, so a URL built with `Url::join` misses it.
    pub fn base(&self) -> Url {
        self.base.clone()
    }

    /// Every request, once the server has answered all of its scripted responses.
    pub async fn received(self) -> Vec<Received> {
        tokio::time::timeout(Duration::from_secs(10), self.task)
            .await
            .expect("the client sent fewer requests than the server scripted")
            .unwrap()
    }
}

impl Received {
    pub fn request_line(&self) -> &str {
        self.head.lines().next().unwrap()
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.head.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name).then(|| value.trim())
        })
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap()
    }

    pub fn form(&self) -> HashMap<String, String> {
        url::form_urlencoded::parse(&self.body)
            .into_owned()
            .collect()
    }
}

pub fn install_crypto_provider() {
    // Another test in this process may have installed it already.
    let _provider = rustls_aws_lc_rs::default_provider().install_default();
}

/// A fresh RSA key for a GitHub App: the private key as a PKCS#8 PEM, and the public key as
/// PKCS#1 DER.
pub fn app_key() -> (Secret, Vec<u8>) {
    let key_pair = KeyPair::generate(KeySize::Rsa2048).unwrap();
    let private: Pkcs8V1Der<'static> = key_pair.as_der().unwrap();
    let pem = format!(
        "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
        STANDARD.encode(private.as_ref())
    );
    (
        pem.parse().unwrap(),
        key_pair.public_key().as_ref().to_vec(),
    )
}

async fn read_request(stream: &mut TcpStream) -> Received {
    let mut buffer = Vec::new();
    let mut chunk = [0; 4096];
    let head_len = loop {
        if let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
        let read = stream.read(&mut chunk).await.unwrap();
        assert_ne!(read, 0, "the client closed before the request head ended");
        buffer.extend_from_slice(&chunk[..read]);
    };
    let body = buffer.split_off(head_len);
    let mut received = Received {
        head: String::from_utf8(buffer).unwrap(),
        body,
    };
    let length: usize = received
        .header("content-length")
        .map_or(0, |length| length.parse().unwrap());
    while received.body.len() < length {
        let read = stream.read(&mut chunk).await.unwrap();
        assert_ne!(read, 0, "the client closed before the request body ended");
        received.body.extend_from_slice(&chunk[..read]);
    }
    received
}
