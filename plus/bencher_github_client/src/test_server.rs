use std::{collections::HashMap, time::Duration};

use rustls::crypto::aws_lc_rs;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use url::Url;

/// A loopback server that answers each request with the next scripted status and JSON body.
pub struct Server {
    base: Url,
    task: JoinHandle<Vec<Received>>,
}

/// One request as the server read it off the wire.
pub struct Received {
    head: String,
    body: Vec<u8>,
}

impl Server {
    pub async fn start(responses: Vec<(u16, String)>) -> Self {
        install_crypto_provider();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base: Url = format!("http://{}/github", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        // A redirect points back at this server, so a client that follows it sends a request
        // that nothing answers.
        let location = format!("location: {base}/redirected\r\n");
        let task = tokio::spawn(async move {
            let mut received = Vec::new();
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                received.push(read_request(&mut stream).await);
                let location = if (300..400).contains(&status) {
                    location.as_str()
                } else {
                    ""
                };
                let response = format!(
                    "HTTP/1.1 {status} Status\r\ncontent-type: application/json\r\ncontent-length: {}\r\n{location}connection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
                stream.shutdown().await.unwrap();
            }
            received
        });
        Self { base, task }
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
    let _provider = aws_lc_rs::default_provider().install_default();
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
