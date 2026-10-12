use std::{
    error::Error as _,
    io, iter,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use http::{
    HeaderMap, HeaderValue, StatusCode,
    header::{ACCEPT, AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, USER_AGENT},
};
use reqwest::{
    dns::{Addrs, Name, Resolve, Resolving},
    redirect,
};
use url::{Host, Url};

use crate::AddressClass;

const APPLICATION_JSON: &str = "application/json";
const BENCHER_USER_AGENT: &str = concat!("Bencher/", env!("CARGO_PKG_VERSION"));

/// An HTTP client that reaches only what its policy allows: `https` to public addresses. It never
/// uses a proxy, follows a redirect, retries, or reuses a connection.
pub struct EgressClient {
    /// The HTTP client, or why it failed to build.
    http: Result<reqwest::Client, Arc<reqwest::Error>>,
    policy: EgressPolicy,
}

/// Why a request got no response.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EgressError {
    /// The policy refused the destination, so nothing was sent.
    #[error(transparent)]
    Blocked(EgressBlock),
    #[error("Request timed out")]
    TimedOut,
    /// The request failed before its response ended, a failed host lookup or a client that failed
    /// to build included. It never carries the URL.
    #[error(transparent)]
    Connection(Arc<reqwest::Error>),
}

impl EgressClient {
    /// Builds the HTTP client now, which panics if the process has no rustls crypto provider installed.
    /// A client that fails to build, as on a host without CA certificates, fails every request with a
    /// connection error.
    pub fn new(timeouts: EgressTimeouts) -> Self {
        Self::build(EgressPolicy::Public, timeouts, Arc::new(SystemLookup))
    }

    #[cfg(any(test, feature = "test-loopback"))]
    pub fn with_lookup(
        policy: EgressPolicy,
        timeouts: EgressTimeouts,
        lookup: Arc<dyn Lookup>,
    ) -> Self {
        Self::build(policy, timeouts, lookup)
    }

    /// A client whose HTTP client failed to build.
    #[cfg(any(test, feature = "test-loopback"))]
    pub fn unavailable(policy: EgressPolicy, error: reqwest::Error) -> Self {
        Self {
            http: Err(Arc::new(error)),
            policy,
        }
    }

    fn build(policy: EgressPolicy, timeouts: EgressTimeouts, lookup: Arc<dyn Lookup>) -> Self {
        let EgressTimeouts { connect, total } = timeouts;
        let http = reqwest::Client::builder()
            .dns_resolver(VettingResolver { lookup, policy })
            .no_proxy()
            // The body cap counts the bytes as sent, whatever features other crates enable.
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .redirect(redirect::Policy::none())
            .retry(reqwest::retry::never())
            .http1_only()
            .pool_max_idle_per_host(0)
            .connect_timeout(connect)
            .timeout(total)
            .build()
            .map_err(|error| Arc::new(error.without_url()));
        Self { http, policy }
    }

    /// Why the HTTP client failed to build, if it did.
    pub fn build_error(&self) -> Option<&reqwest::Error> {
        self.http.as_ref().err().map(AsRef::as_ref)
    }

    /// Sends a `POST` and hands back the response with its body unread.
    pub async fn post(
        &self,
        url: &Url,
        headers: HeaderMap,
        body: String,
    ) -> Result<reqwest::Response, EgressError> {
        self.client_for(url)?
            .post(url.clone())
            .headers(headers)
            .body(body)
            .send()
            .await
            .map_err(EgressError::from)
    }

    /// Fetches a JSON document of at most `cap` bytes, without credentials. Anything but a `200 OK`
    /// with an `application/json` body is refused.
    pub async fn get_json(&self, url: &Url, cap: usize) -> Result<JsonDocument, GetJsonError> {
        let http = self.client_for(url)?;
        let mut request = http
            .get(url.clone())
            .header(ACCEPT, HeaderValue::from_static(APPLICATION_JSON))
            .header(USER_AGENT, HeaderValue::from_static(BENCHER_USER_AGENT))
            .build()
            .map_err(EgressError::from)?;
        // The client sends a URL's userinfo as basic auth.
        request.headers_mut().remove(AUTHORIZATION);
        let mut response = http.execute(request).await.map_err(EgressError::from)?;
        if response.status() != StatusCode::OK {
            return Err(GetJsonError::Status);
        }
        if !is_json(response.headers()) {
            return Err(GetJsonError::ContentType);
        }
        let max_age = cache_max_age(response.headers());
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(EgressError::from)? {
            if body.len() + chunk.len() > cap {
                return Err(GetJsonError::TooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(JsonDocument { body, max_age })
    }

    /// The HTTP client, once the policy allows the URL. A refusal needs no client, so it comes first.
    fn client_for(&self, url: &Url) -> Result<&reqwest::Client, EgressError> {
        self.policy.vet_url(url).map_err(EgressError::Blocked)?;
        self.http
            .as_ref()
            .map_err(|error| EgressError::Connection(Arc::clone(error)))
    }
}

impl From<reqwest::Error> for EgressError {
    fn from(error: reqwest::Error) -> Self {
        // reqwest keeps the resolver's error in the source chain, where it also finds its own timeouts.
        let block = iter::successors(error.source(), |&cause| cause.source())
            .find_map(|cause| cause.downcast_ref::<EgressBlock>())
            .copied();
        if let Some(block) = block {
            Self::Blocked(block)
        } else if error.is_timeout() {
            Self::TimedOut
        } else {
            Self::Connection(Arc::new(error.without_url()))
        }
    }
}

/// How long a request may take.
#[derive(Debug, Clone, Copy)]
pub struct EgressTimeouts {
    /// The lookup, the TCP connect, and the TLS handshake, with the connect split across the host's
    /// addresses.
    pub connect: Duration,
    /// The whole request, from the lookup to the end of the response body.
    pub total: Duration,
}

/// What `get_json` fetched. Nothing else from the response head is kept, since its size has no cap.
#[derive(Debug)]
pub struct JsonDocument {
    pub body: Vec<u8>,
    /// The `max-age` of `Cache-Control`, for a caller that caches the document.
    pub max_age: Option<Duration>,
}

/// Why `get_json` fetched nothing. No variant carries the response status or body.
#[derive(Debug, Clone, thiserror::Error)]
pub enum GetJsonError {
    #[error(transparent)]
    Egress(#[from] EgressError),
    #[error("Response status is not 200 OK")]
    Status,
    #[error("Response content type is not application/json")]
    ContentType,
    #[error("Response body is too large")]
    TooLarge,
}

fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case(APPLICATION_JSON))
}

/// The first `max-age` directive across every `Cache-Control` field line (RFC 9111). A delta too
/// large to hold saturates.
fn cache_max_age(headers: &HeaderMap) -> Option<Duration> {
    let delta = headers
        .get_all(CACHE_CONTROL)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .find_map(|directive| {
            let (name, argument) = directive.split_once('=')?;
            name.trim()
                .eq_ignore_ascii_case("max-age")
                .then(|| argument.trim())
        })?;
    // Recipients accept the quoted-string form too.
    let delta = delta
        .strip_prefix('"')
        .and_then(|delta| delta.strip_suffix('"'))
        .unwrap_or(delta);
    (!delta.is_empty() && delta.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| Duration::from_secs(delta.parse().unwrap_or(u64::MAX)))
}

/// Where a request may go.
#[derive(Clone, Copy)]
pub enum EgressPolicy {
    /// `https` to public addresses.
    Public,
    /// Also plain `http` and loopback addresses, so a test can reach a local listener.
    #[cfg(any(test, feature = "test-loopback"))]
    Loopback,
}

/// Why the policy refused a destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EgressBlock {
    #[error("URL must use https")]
    NotHttps,
    #[error("Address ({address}) is not public: {class}")]
    Address {
        address: IpAddr,
        class: AddressClass,
    },
}

impl EgressPolicy {
    fn vet_url(self, url: &Url) -> Result<(), EgressBlock> {
        let scheme_allowed = match self {
            Self::Public => url.scheme() == "https",
            #[cfg(any(test, feature = "test-loopback"))]
            Self::Loopback => matches!(url.scheme(), "https" | "http"),
        };
        if !scheme_allowed {
            return Err(EgressBlock::NotHttps);
        }
        match url.host() {
            // The connector dials an IP literal without a lookup, so it is vetted here.
            Some(Host::Ipv4(address)) => self.vet(IpAddr::V4(address)),
            Some(Host::Ipv6(address)) => self.vet(IpAddr::V6(address)),
            // A name is vetted as it resolves, and a URL without a host never connects.
            Some(Host::Domain(_)) | None => Ok(()),
        }
    }

    fn vet(self, address: IpAddr) -> Result<(), EgressBlock> {
        let refused = match self {
            Self::Public => AddressClass::of(address),
            #[cfg(any(test, feature = "test-loopback"))]
            Self::Loopback => {
                AddressClass::of(address).filter(|class| *class != AddressClass::Loopback)
            },
        };
        refused.map_or(Ok(()), |class| Err(EgressBlock::Address { address, class }))
    }
}

/// Looks up the addresses of a host name.
#[async_trait]
pub trait Lookup: Send + Sync {
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
    policy: EgressPolicy,
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
        error::Error,
        future, io, iter,
        net::{IpAddr, Ipv4Addr, SocketAddr},
        sync::{Arc, Mutex},
        time::Duration,
    };

    use async_trait::async_trait;
    use http::{HeaderMap, HeaderValue, header::CACHE_CONTROL};
    use reqwest::dns::Resolve as _;
    use tokio::{
        io::{AsyncReadExt as _, AsyncWriteExt as _},
        net::{TcpListener, TcpStream},
        sync::oneshot,
        task::JoinHandle,
        time::Instant,
    };
    use tokio_rustls::rustls::crypto::aws_lc_rs;
    use url::Url;

    use super::{
        AddressClass, EgressBlock, EgressClient, EgressError, EgressPolicy, EgressTimeouts,
        GetJsonError, Lookup, SystemLookup, VettingResolver, cache_max_age,
    };

    const TIMEOUTS: EgressTimeouts = EgressTimeouts {
        connect: Duration::from_secs(3),
        total: Duration::from_secs(5),
    };
    const CAP: usize = 5 << 10;

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

    /// Never answers.
    struct HangingLookup;

    #[async_trait]
    impl Lookup for HangingLookup {
        async fn lookup(&self, _host: &str) -> io::Result<Vec<IpAddr>> {
            future::pending().await
        }
    }

    fn client<L: Lookup + 'static>(policy: EgressPolicy, lookup: Arc<L>) -> EgressClient {
        install_crypto_provider();
        EgressClient::with_lookup(policy, TIMEOUTS, lookup)
    }

    /// A client for a server on loopback, reached by its IP literal.
    fn loopback() -> EgressClient {
        client(EgressPolicy::Loopback, ScriptedLookup::new(Vec::new()))
    }

    fn install_crypto_provider() {
        // Another test in this process may have installed it already.
        let _provider = aws_lc_rs::default_provider().install_default();
    }

    fn url(url: &str) -> Url {
        url.parse().unwrap()
    }

    fn ip(address: &str) -> IpAddr {
        address.parse().unwrap()
    }

    fn response(status: u16, content_type: &str, body: &[u8]) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {status} Status\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        response
    }

    fn json(body: &[u8]) -> Vec<u8> {
        response(200, "application/json", body)
    }

    /// A `200` with no declared length, so only reading the body to the hang up tells its size.
    fn close_delimited(body: &[u8]) -> Vec<u8> {
        let mut response =
            b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\nconnection: close\r\n\r\n"
                .to_vec();
        response.extend_from_slice(body);
        response
    }

    /// One request head as the server read it off the wire.
    struct Received(String);

    impl Received {
        fn request_line(&self) -> &str {
            self.0.lines().next().unwrap()
        }

        fn header(&self, name: &str) -> Option<&str> {
            self.0.lines().skip(1).find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case(name).then(|| value.trim())
            })
        }
    }

    async fn read_head(stream: &mut TcpStream) -> Received {
        let mut head = Vec::new();
        let mut chunk = [0; 4096];
        while !head.ends_with(b"\r\n\r\n") {
            let read = stream.read(&mut chunk).await.unwrap();
            assert_ne!(read, 0, "the client closed before the request head ended");
            head.extend_from_slice(&chunk[..read]);
        }
        Received(String::from_utf8(head).unwrap())
    }

    /// Reads until the client hangs up, and returns anything it sent first.
    async fn until_hung_up(mut stream: TcpStream) -> Vec<u8> {
        let mut rest = Vec::new();
        if let Err(error) = stream.read_to_end(&mut rest).await {
            // A client that hangs up with part of the response unread resets the connection.
            assert_eq!(error.kind(), io::ErrorKind::ConnectionReset, "{error}");
        }
        rest
    }

    async fn listener() -> (TcpListener, SocketAddr) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        (listener, address)
    }

    /// A server on loopback that reads one request, answers it with `response`, and hangs up.
    async fn server(response: Vec<u8>) -> (SocketAddr, JoinHandle<Received>) {
        let (listener, address) = listener().await;
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let received = read_head(&mut stream).await;
            stream.write_all(&response).await.unwrap();
            received
        });
        (address, task)
    }

    #[tokio::test]
    async fn get_fetches_a_json_document() {
        const DOCUMENT: &[u8] = br#"{"client_id":"https://client.example/oauth/client.json"}"#;
        let mut answer = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncache-control: max-age=300\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            DOCUMENT.len()
        )
        .into_bytes();
        answer.extend_from_slice(DOCUMENT);
        let (address, server) = server(answer).await;
        let lookup = ScriptedLookup::new(vec![vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]]);
        let port = address.port();

        let document = client(EgressPolicy::Loopback, Arc::clone(&lookup))
            .get_json(
                &url(&format!(
                    "http://client.example:{port}/oauth/client.json?query=value"
                )),
                CAP,
            )
            .await
            .unwrap();

        assert_eq!(document.body, DOCUMENT);
        assert_eq!(document.max_age, Some(Duration::from_secs(300)));
        let received = server.await.unwrap();
        assert_eq!(
            received.request_line(),
            "GET /oauth/client.json?query=value HTTP/1.1"
        );
        assert_eq!(
            received.header("host"),
            Some(format!("client.example:{port}").as_str())
        );
        assert_eq!(received.header("accept"), Some("application/json"));
        assert_eq!(
            received.header("user-agent"),
            Some(concat!("Bencher/", env!("CARGO_PKG_VERSION")))
        );
        assert_eq!(lookup.names(), ["client.example"]);
    }

    #[test]
    fn max_age_is_the_first_max_age_directive_across_every_cache_control_line() {
        for (lines, expected) in [
            (&[][..], None),
            (&["max-age=300"][..], Some(300)),
            (&["public, Max-Age=300"][..], Some(300)),
            (&["s-maxage=60, max-age=300"][..], Some(300)),
            (&["public", "max-age=600"][..], Some(600)),
            (&["max-age=300", "max-age=600"][..], Some(300)),
            (&["max-age=\"300\""][..], Some(300)),
            (&["max-age=99999999999999999999999"][..], Some(u64::MAX)),
            (&["max-age=abc"][..], None),
            (&["max-age="][..], None),
        ] {
            let mut headers = HeaderMap::new();
            for line in lines {
                headers.append(CACHE_CONTROL, HeaderValue::from_static(line));
            }

            assert_eq!(
                cache_max_age(&headers),
                expected.map(Duration::from_secs),
                "{lines:?}"
            );
        }
    }

    #[tokio::test]
    async fn get_refuses_a_private_address() {
        for (answer, address, class) in [
            (vec![ip("10.0.0.1")], "10.0.0.1", AddressClass::Private),
            (
                vec![ip("93.184.216.34"), ip("192.168.0.1")],
                "192.168.0.1",
                AddressClass::Private,
            ),
            (vec![ip("127.0.0.1")], "127.0.0.1", AddressClass::Loopback),
            (vec![ip("::1")], "::1", AddressClass::Loopback),
            (
                vec![ip("169.254.169.254")],
                "169.254.169.254",
                AddressClass::LinkLocal,
            ),
            (
                vec![ip("fd00:ec2::254")],
                "fd00:ec2::254",
                AddressClass::UniqueLocal,
            ),
            (vec![ip("100.64.0.1")], "100.64.0.1", AddressClass::Shared),
            (vec![ip("240.0.0.1")], "240.0.0.1", AddressClass::Reserved),
        ] {
            let lookup = ScriptedLookup::new(vec![answer]);

            let result = client(EgressPolicy::Public, Arc::clone(&lookup))
                .get_json(&url("https://client.example/oauth/client.json"), CAP)
                .await;

            let expected = EgressBlock::Address {
                address: ip(address),
                class,
            };
            assert!(
                matches!(result, Err(GetJsonError::Egress(EgressError::Blocked(block))) if block == expected),
                "{address}: {result:?}"
            );
            assert_eq!(lookup.names(), ["client.example"]);
        }

        install_crypto_provider();
        let production = EgressClient::new(TIMEOUTS);
        for (literal, address, class) in [
            (
                "https://169.254.169.254/latest/meta-data/",
                "169.254.169.254",
                AddressClass::LinkLocal,
            ),
            ("https://10.0.0.1/", "10.0.0.1", AddressClass::Private),
            (
                "https://[::ffff:127.0.0.1]/",
                "::ffff:127.0.0.1",
                AddressClass::Loopback,
            ),
            ("https://100.64.0.1/", "100.64.0.1", AddressClass::Shared),
        ] {
            let result = production.get_json(&url(literal), CAP).await;

            let expected = EgressBlock::Address {
                address: ip(address),
                class,
            };
            assert!(
                matches!(result, Err(GetJsonError::Egress(EgressError::Blocked(block))) if block == expected),
                "{literal}: {result:?}"
            );
        }
    }

    // A followed redirect would find a document at its target.
    #[tokio::test]
    async fn get_does_not_follow_a_redirect() {
        let (listener, address) = listener().await;
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            let (mut stream, _) = listener.accept().await.unwrap();
            requests.push(read_head(&mut stream).await.request_line().to_owned());
            let redirect = format!(
                "HTTP/1.1 302 Found\r\nlocation: http://{address}/moved.json\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            );
            stream.write_all(redirect.as_bytes()).await.unwrap();
            drop(stream);
            if let Ok(accepted) =
                tokio::time::timeout(Duration::from_millis(100), listener.accept()).await
            {
                let (mut stream, _) = accepted.unwrap();
                requests.push(read_head(&mut stream).await.request_line().to_owned());
                stream.write_all(&json(b"{}")).await.unwrap();
            }
            requests
        });

        let result = loopback()
            .get_json(&url(&format!("http://{address}/client.json")), CAP)
            .await;

        assert!(matches!(result, Err(GetJsonError::Status)), "{result:?}");
        assert_eq!(server.await.unwrap(), ["GET /client.json HTTP/1.1"]);
    }

    #[tokio::test]
    async fn get_stops_at_the_body_cap() {
        let over = vec![b' '; 6 << 10];
        let at = vec![b' '; CAP];
        for (answer, fits) in [
            (json(&over), false),
            (close_delimited(&over), false),
            (close_delimited(&at), true),
        ] {
            let (address, server) = server(answer).await;

            let result = loopback()
                .get_json(&url(&format!("http://{address}/client.json")), CAP)
                .await;

            if fits {
                assert_eq!(result.unwrap().body, at);
            } else {
                assert!(matches!(result, Err(GetJsonError::TooLarge)), "{result:?}");
            }
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn get_refuses_http() {
        install_crypto_provider();
        let production = EgressClient::new(TIMEOUTS);
        // A documentation address, so a lookup that ran would end in another refusal.
        let lookup = ScriptedLookup::new(vec![vec![ip("192.0.2.1")]]);
        let scripted = client(EgressPolicy::Public, Arc::clone(&lookup));

        for client in [&production, &scripted] {
            let result = client
                .get_json(&url("http://client.example/oauth/client.json"), CAP)
                .await;

            assert!(
                matches!(
                    result,
                    Err(GetJsonError::Egress(EgressError::Blocked(
                        EgressBlock::NotHttps
                    )))
                ),
                "{result:?}"
            );
        }
        assert!(
            lookup.names().is_empty(),
            "a refused URL is never looked up"
        );
    }

    #[tokio::test]
    async fn get_accepts_only_200() {
        for status in [201, 203, 204, 206, 299, 301, 304, 400, 404, 500, 503] {
            let (address, server) = server(response(status, "application/json", b"{}")).await;

            let result = loopback()
                .get_json(&url(&format!("http://{address}/client.json")), CAP)
                .await;

            assert!(
                matches!(result, Err(GetJsonError::Status)),
                "{status}: {result:?}"
            );
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn get_requires_a_json_media_type() {
        for (content_type, accepted) in [
            (Some("application/json"), true),
            (Some("application/json; charset=utf-8"), true),
            (Some("Application/JSON;charset=UTF-8"), true),
            (Some("application/json ; charset=utf-8"), true),
            (None, false),
            (Some(""), false),
            (Some("text/json"), false),
            (Some("text/html; charset=utf-8"), false),
            (Some("application/jsonp"), false),
            (Some("application/json-seq"), false),
            (Some("application/problem+json"), false),
            (Some("application/x-json"), false),
        ] {
            let header =
                content_type.map_or_else(String::new, |value| format!("content-type: {value}\r\n"));
            let (address, server) = server(
                format!(
                    "HTTP/1.1 200 OK\r\n{header}content-length: 2\r\nconnection: close\r\n\r\n{{}}"
                )
                .into_bytes(),
            )
            .await;

            let result = loopback()
                .get_json(&url(&format!("http://{address}/client.json")), CAP)
                .await;

            if accepted {
                assert_eq!(result.unwrap().body, b"{}", "{content_type:?}");
            } else {
                assert!(
                    matches!(result, Err(GetJsonError::ContentType)),
                    "{content_type:?}: {result:?}"
                );
            }
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn get_errors_never_reflect_the_response() {
        const MARKER: &str = "BODY-MARKER-7c1e";
        let over = format!("{MARKER}{}", " ".repeat(6 << 10));
        for answer in [
            response(418, "application/json", MARKER.as_bytes()),
            response(200, "text/html", MARKER.as_bytes()),
            response(200, "application/json", over.as_bytes()),
        ] {
            let (address, server) = server(answer).await;

            let error = loopback()
                .get_json(&url(&format!("http://{address}/client.json")), CAP)
                .await
                .unwrap_err();

            let error: &(dyn Error + 'static) = &error;
            let printed = iter::successors(Some(error), |&cause| cause.source())
                .map(|cause| format!("{cause} {cause:?}"))
                .collect::<Vec<_>>()
                .join("\n");
            for reflected in [MARKER, "418"] {
                assert!(!printed.contains(reflected), "{reflected} in {printed}");
            }
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn get_sends_no_credentials() {
        let (address, server) = server(json(b"{}")).await;

        loopback()
            .get_json(
                &url(&format!(
                    "http://user-marker:password-marker@{address}/client.json"
                )),
                CAP,
            )
            .await
            .unwrap();

        let received = server.await.unwrap();
        assert_eq!(received.request_line(), "GET /client.json HTTP/1.1");
        assert_eq!(received.header("authorization"), None);
        assert!(!received.0.contains("marker"), "{}", received.0);
    }

    #[tokio::test]
    async fn get_times_out_a_stalled_body() {
        let (listener, address) = listener().await;
        let (answered_tx, answered_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_head(&mut stream).await;
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 64\r\n\r\n{",
                )
                .await
                .unwrap();
            answered_tx.send(()).unwrap();
            // Hold the rest of the body back until the client gives up.
            until_hung_up(stream).await
        });
        let client = loopback();
        let url = url(&format!("http://{address}/client.json"));

        let start = Instant::now();
        let fetch = tokio::spawn(async move { client.get_json(&url, CAP).await });
        // The head and the first byte of the body reach the client in real time, then the clock stops.
        answered_rx.await.unwrap();
        tokio::time::pause();
        let before_pause = start.elapsed();

        let result = tokio::time::timeout(TIMEOUTS.total + Duration::from_secs(1), fetch)
            .await
            .expect("the body read has a timeout")
            .unwrap();

        assert!(
            matches!(result, Err(GetJsonError::Egress(EgressError::TimedOut))),
            "{result:?}"
        );
        let elapsed = start.elapsed();
        // The 2 ms cover the timer's 1 ms granularity.
        assert!(
            elapsed >= TIMEOUTS.total
                && elapsed <= before_pause + TIMEOUTS.total + Duration::from_millis(2),
            "timed out after {elapsed:?}"
        );
        assert_eq!(server.await.unwrap(), Vec::<u8>::new());
    }

    #[tokio::test(start_paused = true)]
    async fn get_gives_up_on_a_lookup_at_the_connect_timeout() {
        let client = client(EgressPolicy::Public, Arc::new(HangingLookup));
        let start = Instant::now();

        let result = client
            .get_json(&url("https://client.example/oauth/client.json"), CAP)
            .await;

        assert!(
            matches!(result, Err(GetJsonError::Egress(EgressError::TimedOut))),
            "{result:?}"
        );
        let elapsed = start.elapsed();
        assert!(
            elapsed >= TIMEOUTS.connect && elapsed <= TIMEOUTS.connect + Duration::from_millis(2),
            "timed out after {elapsed:?}"
        );
    }

    #[test]
    fn the_loopback_policy_relaxes_only_loopback() {
        let loopback = ip("127.0.0.1");
        assert_eq!(EgressPolicy::Loopback.vet(loopback), Ok(()));
        assert_eq!(
            EgressPolicy::Loopback.vet_url(&url("http://localhost/")),
            Ok(())
        );
        for (address, class) in [
            ("10.0.0.1", AddressClass::Private),
            ("169.254.169.254", AddressClass::LinkLocal),
            ("fd00:ec2::254", AddressClass::UniqueLocal),
            ("100.64.0.1", AddressClass::Shared),
            ("240.0.0.1", AddressClass::Reserved),
        ] {
            assert_eq!(
                EgressPolicy::Loopback.vet(ip(address)),
                Err(EgressBlock::Address {
                    address: ip(address),
                    class
                }),
                "{address}"
            );
        }
    }

    // Port 0 tells the connector to use the URL's port, or the scheme's default.
    #[tokio::test]
    async fn the_resolver_hands_over_exactly_the_vetted_addresses() {
        let answer = vec![ip("93.184.216.34"), ip("2606:4700:4700::1111")];
        let resolver = VettingResolver {
            lookup: ScriptedLookup::new(vec![answer.clone()]),
            policy: EgressPolicy::Public,
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

        let client = client(EgressPolicy::Public, ScriptedLookup::new(Vec::new()));
        let http = client.http.as_ref().unwrap();

        assert!(!format!("{http:?}").contains("proxies"), "{http:?}");
    }
}
