use std::{
    error::Error,
    fmt, io, iter,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use http::{HeaderMap, StatusCode};
use reqwest::{
    dns::{Addrs, Name, Resolve, Resolving},
    redirect,
};
use url::{Host, Url};

mod policy;

pub use policy::AddressClass;

/// The whole attempt, from the host lookup to the response head.
const TIMEOUT: Duration = Duration::from_secs(10);
/// The lookup, the TCP connect, and the TLS handshake, with the connect split across the host's addresses.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Sends one attempt of a callback.
#[async_trait]
pub trait CallbackSender: Send + Sync {
    async fn send(&self, request: &CallbackRequest) -> CallbackAttempt;
}

/// A callback ready to send: its URL, its final headers, and its rendered JSON body.
#[derive(Clone)]
pub struct CallbackRequest {
    pub url: Url,
    pub headers: HeaderMap,
    pub body: String,
}

/// What one attempt came to.
#[derive(Debug)]
pub enum CallbackAttempt {
    /// The receiver answered with a 2xx status.
    Delivered(StatusCode),
    /// The receiver answered with any other status from 100 to 999, a redirect included.
    Refused(StatusCode),
    /// The receiver did not answer in time.
    TimedOut,
    /// The attempt failed before the receiver answered, a failed host lookup included.
    Connection(CallbackConnectionError),
    /// The address policy refused the destination, so nothing was sent.
    Blocked(CallbackBlock),
}

/// Why the address policy refused a destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CallbackBlock {
    #[error("Callback URL must use https")]
    NotHttps,
    #[error("Callback address ({address}) is not public: {class}")]
    Address {
        address: IpAddr,
        class: AddressClass,
    },
}

/// Why an attempt failed before the receiver answered.
#[derive(Debug, thiserror::Error)]
pub enum CallbackConnectionError {
    #[error(transparent)]
    Send(reqwest::Error),
    /// The HTTP client failed to build at boot, so no callback is ever sent.
    #[error("Callback client is unavailable")]
    Unavailable(#[source] Arc<reqwest::Error>),
}

/// The production sender: `https` to public addresses only, one request per attempt.
pub struct CallbackClient {
    /// The HTTP client, or why it failed to build.
    http: Result<reqwest::Client, Arc<reqwest::Error>>,
    policy: Policy,
}

impl fmt::Debug for CallbackRequest {
    // The body can hold a private project's results, so like `Secret`, only a debug build prints it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let body = if cfg!(debug_assertions) {
            BodyDebug::Full
        } else {
            BodyDebug::Length
        };
        fmt::Debug::fmt(
            &RequestDebug {
                request: self,
                body,
            },
            f,
        )
    }
}

/// How a request's `Debug` prints its body.
#[derive(Clone, Copy)]
enum BodyDebug {
    Full,
    Length,
}

struct RequestDebug<'a> {
    request: &'a CallbackRequest,
    body: BodyDebug,
}

impl fmt::Debug for RequestDebug<'_> {
    // The URL path, query, and userinfo and every header value can hold a secret.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            request: CallbackRequest { url, headers, body },
            body: body_debug,
        } = self;
        let mut debug = f.debug_struct("CallbackRequest");
        debug
            .field("url", &url.origin().ascii_serialization())
            .field("headers", &headers.keys().collect::<Vec<_>>());
        match body_debug {
            BodyDebug::Full => debug.field("body", body),
            BodyDebug::Length => debug.field("body_len", &body.len()),
        };
        debug.finish()
    }
}

impl CallbackAttempt {
    fn answered(status: StatusCode) -> Self {
        if status.is_success() {
            Self::Delivered(status)
        } else {
            Self::Refused(status)
        }
    }

    fn failed(error: reqwest::Error) -> Self {
        // reqwest keeps the resolver's error in the source chain, where it also finds its own timeouts.
        let block = iter::successors(error.source(), |&cause| cause.source())
            .find_map(|cause| cause.downcast_ref::<CallbackBlock>())
            .copied();
        if let Some(block) = block {
            Self::Blocked(block)
        } else if error.is_timeout() {
            Self::TimedOut
        } else {
            Self::Connection(CallbackConnectionError::Send(error.without_url()))
        }
    }
}

impl CallbackConnectionError {
    #[must_use]
    pub fn without_url(self) -> Self {
        match self {
            Self::Send(error) => Self::Send(error.without_url()),
            Self::Unavailable(error) => Self::Unavailable(error),
        }
    }
}

/// An error and every cause under it, which tell a missing name from a refused connection.
pub fn error_chain(error: &dyn Error) -> String {
    iter::successors(error.source(), |&cause| cause.source()).fold(
        error.to_string(),
        |mut chain, cause| {
            chain.push_str(": ");
            chain.push_str(&cause.to_string());
            chain
        },
    )
}

impl CallbackClient {
    /// Builds the HTTP client now, at boot, which panics if the process has no rustls crypto provider installed.
    /// A client that fails to build, as on a host without CA certificates, makes every send a connection error.
    pub fn new() -> Self {
        Self::with_policy(Policy::Public, Arc::new(SystemLookup))
    }

    fn with_policy(policy: Policy, lookup: Arc<dyn Lookup>) -> Self {
        let http = reqwest::Client::builder()
            .dns_resolver(VettingResolver { lookup, policy })
            .no_proxy()
            .redirect(redirect::Policy::none())
            .retry(reqwest::retry::never())
            .http1_only()
            .pool_max_idle_per_host(0)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(TIMEOUT)
            .build()
            .map_err(|error| Arc::new(error.without_url()));
        Self { http, policy }
    }

    /// Why the HTTP client failed to build, if it did.
    pub fn build_error(&self) -> Option<&reqwest::Error> {
        self.http.as_ref().err().map(AsRef::as_ref)
    }
}

impl Default for CallbackClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CallbackSender for CallbackClient {
    async fn send(&self, request: &CallbackRequest) -> CallbackAttempt {
        let CallbackRequest { url, headers, body } = request;
        if let Err(block) = self.policy.vet_url(url) {
            return CallbackAttempt::Blocked(block);
        }
        let http = match &self.http {
            Ok(http) => http,
            Err(error) => {
                return CallbackAttempt::Connection(CallbackConnectionError::Unavailable(
                    Arc::clone(error),
                ));
            },
        };
        let response = http
            .post(url.clone())
            .headers(headers.clone())
            .body(body.clone())
            .send()
            .await;
        match response {
            // The body is never read: dropping the response closes the connection.
            Ok(response) => CallbackAttempt::answered(response.status()),
            Err(error) => CallbackAttempt::failed(error),
        }
    }
}

/// Where a callback may go.
#[derive(Clone, Copy)]
enum Policy {
    /// `https` to public addresses.
    Public,
    /// Also plain `http` and loopback addresses, so a test can deliver to a local listener.
    #[cfg(test)]
    Loopback,
}

impl Policy {
    fn vet_url(self, url: &Url) -> Result<(), CallbackBlock> {
        let scheme_allowed = match self {
            Self::Public => url.scheme() == "https",
            #[cfg(test)]
            Self::Loopback => matches!(url.scheme(), "https" | "http"),
        };
        if !scheme_allowed {
            return Err(CallbackBlock::NotHttps);
        }
        match url.host() {
            // The connector dials an IP literal without a lookup, so it is vetted here.
            Some(Host::Ipv4(address)) => self.vet(IpAddr::V4(address)),
            Some(Host::Ipv6(address)) => self.vet(IpAddr::V6(address)),
            // A name is vetted as it resolves, and a URL without a host never connects.
            Some(Host::Domain(_)) | None => Ok(()),
        }
    }

    fn vet(self, address: IpAddr) -> Result<(), CallbackBlock> {
        let refused = match self {
            Self::Public => AddressClass::of(address),
            #[cfg(test)]
            Self::Loopback => {
                AddressClass::of(address).filter(|class| *class != AddressClass::Loopback)
            },
        };
        refused.map_or(Ok(()), |class| {
            Err(CallbackBlock::Address { address, class })
        })
    }
}

/// Looks up the addresses of a host name.
#[async_trait]
trait Lookup: Send + Sync {
    async fn lookup(&self, host: &str) -> io::Result<Vec<IpAddr>>;
}

struct SystemLookup;

#[async_trait]
impl Lookup for SystemLookup {
    async fn lookup(&self, host: &str) -> io::Result<Vec<IpAddr>> {
        let addresses = tokio::net::lookup_host((host, 0)).await?;
        Ok(addresses.map(|address| address.ip()).collect())
    }
}

/// The client's only resolver. It looks a name up once, vets every address in the answer,
/// and hands the connector exactly those addresses, so the address vetted is the address dialed.
struct VettingResolver {
    lookup: Arc<dyn Lookup>,
    policy: Policy,
}

impl Resolve for VettingResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let lookup = Arc::clone(&self.lookup);
        let policy = self.policy;
        Box::pin(async move {
            let addresses = lookup.lookup(name.as_str()).await?;
            addresses
                .iter()
                .try_for_each(|address| policy.vet(*address))?;
            // Port 0 takes the URL's port, or the scheme's default.
            let addrs: Addrs = Box::new(
                addresses
                    .into_iter()
                    .map(|address| SocketAddr::new(address, 0)),
            );
            Ok(addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        io, iter,
        net::{IpAddr, Ipv4Addr, SocketAddr},
        sync::{Arc, Mutex},
        time::Duration,
    };

    use async_trait::async_trait;
    use http::{HeaderMap, HeaderValue, StatusCode, header};
    use reqwest::dns::Resolve as _;
    use tokio::{
        io::{AsyncReadExt as _, AsyncWriteExt as _},
        net::{TcpListener, TcpStream},
        sync::oneshot,
        task::JoinHandle,
    };
    use tokio_rustls::rustls::crypto::aws_lc_rs;

    use super::{
        AddressClass, BodyDebug, CallbackAttempt, CallbackBlock, CallbackClient,
        CallbackConnectionError, CallbackRequest, CallbackSender, Lookup, Policy, RequestDebug,
        SystemLookup, VettingResolver, error_chain,
    };

    /// Answers each lookup with the next scripted answer, and records every name it is asked.
    struct ScriptedLookup {
        answers: Mutex<VecDeque<Vec<IpAddr>>>,
        names: Mutex<Vec<String>>,
    }

    impl ScriptedLookup {
        fn new(answers: Vec<Vec<IpAddr>>) -> Arc<Self> {
            Arc::new(Self {
                answers: Mutex::new(answers.into()),
                names: Mutex::new(Vec::new()),
            })
        }

        fn names(&self) -> Vec<String> {
            self.names.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Lookup for ScriptedLookup {
        async fn lookup(&self, host: &str) -> io::Result<Vec<IpAddr>> {
            self.names.lock().unwrap().push(host.to_owned());
            self.answers
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        }
    }

    fn client(policy: Policy, lookup: Arc<ScriptedLookup>) -> CallbackClient {
        install_crypto_provider();
        CallbackClient::with_policy(policy, lookup)
    }

    fn install_crypto_provider() {
        // Another test in this process may have installed it already.
        let _provider = aws_lc_rs::default_provider().install_default();
    }

    fn request(url: &str) -> CallbackRequest {
        CallbackRequest {
            url: url.parse().unwrap(),
            headers: HeaderMap::new(),
            body: "{}".to_owned(),
        }
    }

    fn ip(address: &str) -> IpAddr {
        address.parse().unwrap()
    }

    fn answer(status: u16) -> String {
        format!("HTTP/1.1 {status} Status\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
    }

    /// One request as the receiver read it off the wire.
    struct Received {
        head: String,
        body: Vec<u8>,
    }

    impl Received {
        fn request_line(&self) -> &str {
            self.head.lines().next().unwrap()
        }

        fn header(&self, name: &str) -> Option<&str> {
            self.head.lines().skip(1).find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case(name).then(|| value.trim())
            })
        }
    }

    async fn read_request(stream: &mut TcpStream) -> Received {
        let mut buffer = Vec::new();
        let mut chunk = [0; 4096];
        let head_len = loop {
            if let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                break end + 4;
            }
            let read = stream.read(&mut chunk).await.unwrap();
            assert_ne!(read, 0, "the sender closed before the request head ended");
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
            assert_ne!(read, 0, "the sender closed before the request body ended");
            received.body.extend_from_slice(&chunk[..read]);
        }
        received
    }

    /// Reads until the sender hangs up, and returns anything it sent first.
    async fn until_hung_up(mut stream: TcpStream) -> Vec<u8> {
        let mut rest = Vec::new();
        if let Err(error) = stream.read_to_end(&mut rest).await {
            // A sender that hangs up with part of the response unread resets the connection.
            assert_eq!(error.kind(), io::ErrorKind::ConnectionReset, "{error}");
        }
        rest
    }

    async fn listener() -> (TcpListener, SocketAddr) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        (listener, address)
    }

    /// A receiver on loopback that reads one request and answers it with `response`.
    async fn receiver(response: String) -> (SocketAddr, JoinHandle<Received>) {
        let (listener, address) = listener().await;
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let received = read_request(&mut stream).await;
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.shutdown().await.unwrap();
            received
        });
        (address, task)
    }

    #[tokio::test]
    async fn sends_the_request_as_given() {
        let (address, server) = receiver(answer(204)).await;
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static("bencher/1.2.3"),
        );
        headers.insert("x-token", HeaderValue::from_static("token-marker"));
        let request = CallbackRequest {
            url: format!("http://{address}/hook/path?query=value")
                .parse()
                .unwrap(),
            headers,
            body: r#"{"job":{"uuid":"00000000-0000-4000-8000-000000000000"}}"#.to_owned(),
        };
        let lookup = ScriptedLookup::new(Vec::new());
        let client = client(Policy::Loopback, Arc::clone(&lookup));

        let attempt = client.send(&request).await;

        assert!(
            matches!(attempt, CallbackAttempt::Delivered(StatusCode::NO_CONTENT)),
            "{attempt:?}"
        );
        let received = server.await.unwrap();
        assert_eq!(
            received.request_line(),
            "POST /hook/path?query=value HTTP/1.1"
        );
        assert_eq!(received.header("content-type"), Some("application/json"));
        assert_eq!(received.header("user-agent"), Some("bencher/1.2.3"));
        assert_eq!(received.header("x-token"), Some("token-marker"));
        assert_eq!(received.header("host"), Some(address.to_string().as_str()));
        assert_eq!(received.body, request.body.as_bytes());
        assert!(
            lookup.names().is_empty(),
            "an IP literal is never looked up"
        );
    }

    #[tokio::test]
    async fn sends_userinfo_as_basic_auth() {
        let (address, server) = receiver(answer(200)).await;
        let request = request(&format!("http://user:secret@{address}/hook"));

        let attempt = client(Policy::Loopback, ScriptedLookup::new(Vec::new()))
            .send(&request)
            .await;

        assert!(
            matches!(attempt, CallbackAttempt::Delivered(StatusCode::OK)),
            "{attempt:?}"
        );
        let received = server.await.unwrap();
        assert_eq!(received.request_line(), "POST /hook HTTP/1.1");
        // base64 of `user:secret`
        assert_eq!(
            received.header("authorization"),
            Some("Basic dXNlcjpzZWNyZXQ=")
        );
    }

    #[tokio::test]
    async fn a_2xx_is_delivered_and_any_other_status_is_refused() {
        for (status, delivered) in [
            (200, true),
            (201, true),
            (204, true),
            (299, true),
            (301, false),
            (304, false),
            (400, false),
            (404, false),
            (429, false),
            (500, false),
            (503, false),
        ] {
            let (address, server) = receiver(answer(status)).await;

            let attempt = client(Policy::Loopback, ScriptedLookup::new(Vec::new()))
                .send(&request(&format!("http://{address}/")))
                .await;

            let expected = StatusCode::from_u16(status).unwrap();
            if delivered {
                assert!(
                    matches!(attempt, CallbackAttempt::Delivered(code) if code == expected),
                    "{status}: {attempt:?}"
                );
            } else {
                assert!(
                    matches!(attempt, CallbackAttempt::Refused(code) if code == expected),
                    "{status}: {attempt:?}"
                );
            }
            server.await.unwrap();
        }
    }

    // Following the redirect would open a second connection that nothing accepts, and wait out
    // the timeout.
    #[tokio::test]
    async fn does_not_follow_a_redirect() {
        let (listener, address) = listener().await;
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let received = read_request(&mut stream).await;
            let response = format!(
                "HTTP/1.1 302 Found\r\nlocation: http://{address}/moved\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.shutdown().await.unwrap();
            // The listener stays open, so a followed redirect would connect.
            (received, listener)
        });

        let attempt = client(Policy::Loopback, ScriptedLookup::new(Vec::new()))
            .send(&request(&format!("http://{address}/hook")))
            .await;

        assert!(
            matches!(attempt, CallbackAttempt::Refused(StatusCode::FOUND)),
            "{attempt:?}"
        );
        let (received, _listener) = server.await.unwrap();
        assert_eq!(received.request_line(), "POST /hook HTTP/1.1");
    }

    // The body is far larger than the socket buffers, so the receiver can finish writing it only
    // if the sender drains it.
    #[tokio::test]
    async fn does_not_read_the_response_body() {
        const CHUNK: usize = 64 * 1024;
        const CHUNKS: usize = 1024;
        let (listener, address) = listener().await;
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_request(&mut stream).await;
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n",
                CHUNK * CHUNKS
            );
            stream.write_all(head.as_bytes()).await.unwrap();
            let chunk = vec![0; CHUNK];
            for _ in 0..CHUNKS {
                if stream.write_all(&chunk).await.is_err() {
                    // The sender hung up with the body unread.
                    return false;
                }
            }
            true
        });

        let attempt = client(Policy::Loopback, ScriptedLookup::new(Vec::new()))
            .send(&request(&format!("http://{address}/")))
            .await;

        assert!(
            matches!(attempt, CallbackAttempt::Delivered(StatusCode::OK)),
            "{attempt:?}"
        );
        assert!(!server.await.unwrap(), "the sender drained the body");
    }

    #[tokio::test]
    async fn times_out_a_receiver_that_never_answers() {
        let (listener, address) = listener().await;
        let (received_tx, received_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_request(&mut stream).await;
            received_tx.send(()).unwrap();
            // Hold the connection open, unanswered, until the sender gives up.
            until_hung_up(stream).await
        });
        let sender: Arc<dyn CallbackSender> =
            Arc::new(client(Policy::Loopback, ScriptedLookup::new(Vec::new())));
        let request = request(&format!("http://{address}/"));

        let start = tokio::time::Instant::now();
        let attempt = tokio::spawn(async move { sender.send(&request).await });
        // The request reaches the receiver in real time, then the clock stops.
        received_rx.await.unwrap();
        tokio::time::pause();
        let before_pause = start.elapsed();
        let almost = Duration::from_secs(10)
            .checked_sub(before_pause + Duration::from_millis(1))
            .unwrap();
        tokio::time::advance(almost).await;
        tokio::task::yield_now().await;
        assert!(!attempt.is_finished(), "the attempt waits out its timeout");

        // Nothing else is pending, so the paused clock jumps to the attempt's deadline.
        let attempt = tokio::time::timeout(Duration::from_secs(10), attempt)
            .await
            .expect("the attempt has a timeout")
            .unwrap();

        assert!(matches!(attempt, CallbackAttempt::TimedOut), "{attempt:?}");
        let elapsed = start.elapsed();
        // The 2 ms cover the timer's 1 ms granularity.
        assert!(
            elapsed >= Duration::from_secs(10)
                && elapsed <= before_pause + Duration::from_secs(10) + Duration::from_millis(2),
            "timed out after {elapsed:?}"
        );
        assert!(server.await.unwrap().is_empty());
    }

    // A real `reqwest` error with a cause, which stands in for a client that failed to build.
    fn build_error() -> reqwest::Error {
        reqwest::Request::try_from(http::Request::builder().uri("/relative").body("").unwrap())
            .unwrap_err()
    }

    #[tokio::test]
    async fn an_unavailable_client_sends_nothing() {
        let unavailable = |policy| CallbackClient {
            http: Err(Arc::new(build_error())),
            policy,
        };
        let build_chain = error_chain(&build_error());
        let (listener, address) = listener().await;

        let client = unavailable(Policy::Loopback);
        assert_eq!(
            client.build_error().map(|error| error_chain(error)),
            Some(build_chain.clone())
        );
        for _ in 0..2 {
            let attempt = client
                .send(&request(&format!("http://{address}/hook")))
                .await;
            let CallbackAttempt::Connection(error @ CallbackConnectionError::Unavailable(_)) =
                attempt
            else {
                panic!("{attempt:?}");
            };
            assert_eq!(
                error_chain(&error),
                format!("Callback client is unavailable: {build_chain}")
            );
        }
        // A dial would already wait in the listener's queue.
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err(),
            "an unavailable client never dials"
        );

        // The scheme and address refusals need no client, so they come first.
        let client = unavailable(Policy::Public);
        assert!(
            matches!(
                client.send(&request("http://callback.example/hook")).await,
                CallbackAttempt::Blocked(CallbackBlock::NotHttps)
            ),
            "http is refused first"
        );
        assert!(
            matches!(
                client.send(&request("https://10.0.0.1/hook")).await,
                CallbackAttempt::Blocked(CallbackBlock::Address {
                    class: AddressClass::Private,
                    ..
                })
            ),
            "a private literal is refused first"
        );
    }

    #[tokio::test]
    async fn refuses_http() {
        install_crypto_provider();
        let production = CallbackClient::new();
        let lookup = ScriptedLookup::new(vec![vec![ip("93.184.216.34")]]);
        let scripted = client(Policy::Public, Arc::clone(&lookup));

        for sender in [&production, &scripted] {
            let attempt = sender.send(&request("http://callback.example/hook")).await;

            assert!(
                matches!(attempt, CallbackAttempt::Blocked(CallbackBlock::NotHttps)),
                "{attempt:?}"
            );
        }
        assert!(
            lookup.names().is_empty(),
            "a refused URL is never looked up"
        );
    }

    #[tokio::test]
    async fn refuses_a_private_ip_literal_without_a_lookup() {
        for (url, address, class) in [
            ("https://10.0.0.1/hook", "10.0.0.1", AddressClass::Private),
            (
                "https://127.0.0.1:8443/",
                "127.0.0.1",
                AddressClass::Loopback,
            ),
            (
                "https://169.254.169.254/latest/meta-data/",
                "169.254.169.254",
                AddressClass::LinkLocal,
            ),
            ("https://[::1]/", "::1", AddressClass::Loopback),
            (
                "https://[::ffff:127.0.0.1]/",
                "::ffff:127.0.0.1",
                AddressClass::Loopback,
            ),
            (
                "https://[fd00:ec2::254]/",
                "fd00:ec2::254",
                AddressClass::UniqueLocal,
            ),
            (
                "https://[64:ff9b::a9fe:a9fe]/",
                "64:ff9b::a9fe:a9fe",
                AddressClass::LinkLocal,
            ),
            // The URL parser reads each of these as an IPv4 address.
            ("https://2130706433/", "127.0.0.1", AddressClass::Loopback),
            ("https://0x7f.1/", "127.0.0.1", AddressClass::Loopback),
            ("https://0/", "0.0.0.0", AddressClass::Unspecified),
            ("https://0177.0.0.1/", "127.0.0.1", AddressClass::Loopback),
            ("https://[::]/", "::", AddressClass::Unspecified),
            (
                "https://[2002:7f00:1::]/",
                "2002:7f00:1::",
                AddressClass::Loopback,
            ),
        ] {
            let lookup = ScriptedLookup::new(vec![vec![ip("93.184.216.34")]]);
            let expected = CallbackBlock::Address {
                address: ip(address),
                class,
            };

            install_crypto_provider();
            let production = CallbackClient::new();
            let scripted = client(Policy::Public, Arc::clone(&lookup));

            for sender in [&production, &scripted] {
                let attempt = sender.send(&request(url)).await;

                assert!(
                    matches!(attempt, CallbackAttempt::Blocked(block) if block == expected),
                    "{url}: {attempt:?}"
                );
            }
            assert!(lookup.names().is_empty(), "{url} is never looked up");
        }
    }

    #[tokio::test]
    async fn refuses_a_name_when_any_address_is_not_public() {
        for (answer, address, class) in [
            (
                vec![ip("93.184.216.34"), ip("10.0.0.1")],
                "10.0.0.1",
                AddressClass::Private,
            ),
            (
                vec![ip("10.0.0.1"), ip("93.184.216.34")],
                "10.0.0.1",
                AddressClass::Private,
            ),
            (
                vec![ip("2606:4700:4700::1111"), ip("169.254.169.254")],
                "169.254.169.254",
                AddressClass::LinkLocal,
            ),
            (vec![ip("::1")], "::1", AddressClass::Loopback),
            (
                vec![ip("::ffff:192.168.0.1")],
                "::ffff:192.168.0.1",
                AddressClass::Private,
            ),
        ] {
            let lookup = ScriptedLookup::new(vec![answer]);

            let attempt = client(Policy::Public, Arc::clone(&lookup))
                .send(&request("https://callback.example/hook"))
                .await;

            let expected = CallbackBlock::Address {
                address: ip(address),
                class,
            };
            assert!(
                matches!(attempt, CallbackAttempt::Blocked(block) if block == expected),
                "{attempt:?}"
            );
            assert_eq!(lookup.names(), ["callback.example"]);
        }
    }

    // A rebinding answer on a second lookup must never reach the connector.
    #[tokio::test]
    async fn dials_the_address_it_vetted() {
        let (address, server) = receiver(answer(200)).await;
        let lookup = ScriptedLookup::new(vec![
            vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            vec![ip("10.0.0.1")],
        ]);

        let attempt = client(Policy::Loopback, Arc::clone(&lookup))
            .send(&request(&format!(
                "http://callback.example:{}/hook",
                address.port()
            )))
            .await;

        assert!(
            matches!(attempt, CallbackAttempt::Delivered(StatusCode::OK)),
            "{attempt:?}"
        );
        assert_eq!(lookup.names(), ["callback.example"]);
        let received = server.await.unwrap();
        assert_eq!(
            received.header("host"),
            Some(format!("callback.example:{}", address.port()).as_str())
        );
    }

    // The receiver keeps the first connection open, so a pooled connection would carry the
    // second attempt past the lookup and wait out the timeout.
    #[tokio::test]
    async fn every_attempt_looks_up_and_vets_again() {
        let (listener, address) = listener().await;
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_request(&mut stream).await;
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n")
                .await
                .unwrap();
            // The sender closes the connection instead of keeping it for the next attempt.
            until_hung_up(stream).await
        });
        let lookup = ScriptedLookup::new(vec![
            vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            vec![ip("10.0.0.1")],
        ]);
        let client = client(Policy::Loopback, Arc::clone(&lookup));
        let request = request(&format!("http://callback.example:{}/hook", address.port()));

        let first = client.send(&request).await;
        let second = client.send(&request).await;

        assert!(
            matches!(first, CallbackAttempt::Delivered(StatusCode::OK)),
            "{first:?}"
        );
        let expected = CallbackBlock::Address {
            address: ip("10.0.0.1"),
            class: AddressClass::Private,
        };
        assert!(
            matches!(second, CallbackAttempt::Blocked(block) if block == expected),
            "{second:?}"
        );
        assert_eq!(lookup.names(), ["callback.example", "callback.example"]);
        assert!(server.await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_failed_or_empty_lookup_is_a_connection_error() {
        // No scripted answer fails the lookup; an empty one leaves nothing to dial.
        for answers in [Vec::new(), vec![Vec::new()]] {
            let lookup = ScriptedLookup::new(answers);

            let attempt = client(Policy::Public, Arc::clone(&lookup))
                .send(&request("https://callback.example/hook"))
                .await;

            assert!(
                matches!(attempt, CallbackAttempt::Connection(_)),
                "{attempt:?}"
            );
            assert_eq!(lookup.names(), ["callback.example"]);
        }
    }

    // Port 0 tells the connector to use the URL's port, or the scheme's default.
    #[tokio::test]
    async fn the_resolver_hands_over_exactly_the_vetted_addresses() {
        let answer = vec![ip("93.184.216.34"), ip("2606:4700:4700::1111")];
        let resolver = VettingResolver {
            lookup: ScriptedLookup::new(vec![answer.clone()]),
            policy: Policy::Public,
        };

        let addresses: Vec<SocketAddr> = resolver
            .resolve("callback.example".parse().unwrap())
            .await
            .unwrap()
            .collect();

        let expected: Vec<SocketAddr> = answer
            .into_iter()
            .map(|address| SocketAddr::new(address, 0))
            .collect();
        assert_eq!(addresses, expected);
    }

    #[tokio::test]
    async fn the_system_lookup_resolves_a_name() {
        let addresses = SystemLookup.lookup("localhost").await.unwrap();

        assert!(!addresses.is_empty(), "localhost has an address");
        assert!(addresses.iter().all(IpAddr::is_loopback), "{addresses:?}");
    }

    #[test]
    fn uses_no_proxy() {
        install_crypto_provider();
        // A client with reqwest's defaults shows its proxies, so this test cannot pass vacuously.
        let defaults = reqwest::Client::builder().build().unwrap();
        assert!(format!("{defaults:?}").contains("proxies"), "{defaults:?}");

        let client = client(Policy::Public, ScriptedLookup::new(Vec::new()));
        let http = client.http.as_ref().unwrap();

        assert!(!format!("{http:?}").contains("proxies"), "{http:?}");
    }

    #[tokio::test]
    async fn a_connection_error_holds_no_secret_from_the_url() {
        let (listener, address) = listener().await;
        drop(listener);

        let attempt = client(Policy::Loopback, ScriptedLookup::new(Vec::new()))
            .send(&request(&format!(
                "http://user-marker:password-marker@{address}/path-marker?query-marker=value-marker#fragment-marker"
            )))
            .await;

        let CallbackAttempt::Connection(error) = attempt else {
            panic!("{attempt:?}");
        };
        let error: &(dyn std::error::Error + 'static) = &error;
        let printed = iter::successors(Some(error), |&cause| cause.source())
            .map(|cause| format!("{cause} {cause:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        for marker in [
            "user-marker",
            "password-marker",
            "path-marker",
            "query-marker",
            "value-marker",
            "fragment-marker",
        ] {
            assert!(!printed.contains(marker), "{marker} in {printed}");
        }
    }

    #[test]
    fn request_debug_holds_no_secret() {
        const URL_AND_HEADER_MARKERS: [&str; 6] = [
            "user-marker",
            "password-marker",
            "path-marker",
            "query-marker",
            "fragment-marker",
            "token-marker",
        ];
        let mut headers = HeaderMap::new();
        headers.insert("x-token", HeaderValue::from_static("token-marker"));
        let request = CallbackRequest {
            url: "https://user-marker:password-marker@callback.example:8443/path-marker?query-marker#fragment-marker"
                .parse()
                .unwrap(),
            headers,
            body: r#"{"job":"body-marker"}"#.to_owned(),
        };
        let printed = |body| {
            format!(
                "{:?}",
                RequestDebug {
                    request: &request,
                    body
                }
            )
        };

        let release = printed(BodyDebug::Length);
        let debug = printed(BodyDebug::Full);
        for printed in [&release, &debug] {
            for marker in URL_AND_HEADER_MARKERS {
                assert!(!printed.contains(marker), "{marker} in {printed}");
            }
            assert!(
                printed.contains("https://callback.example:8443"),
                "{printed}"
            );
            assert!(printed.contains("x-token"), "{printed}");
        }
        assert!(!release.contains("body-marker"), "{release}");
        assert!(release.contains("body_len: 21"), "{release}");
        assert!(debug.contains("body-marker"), "{debug}");

        // The build decides which of the two `Debug` prints.
        let expected = if cfg!(debug_assertions) {
            debug
        } else {
            release
        };
        assert_eq!(format!("{request:?}"), expected);
    }
}
