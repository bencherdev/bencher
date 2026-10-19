use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError},
    time::Duration,
};

use bencher_egress::{EgressClient, EgressTimeouts};
use bencher_json::{Clock, OAuthClientMetadata};
use tokio::sync::Semaphore;
use url::Url;

use crate::{OAuthClientError, Throttled};

const TIMEOUTS: EgressTimeouts = EgressTimeouts {
    connect: Duration::from_secs(3),
    total: Duration::from_secs(5),
};
const MAX_DOCUMENT_BYTES: usize = 5 << 10;
const MAX_FETCHES: usize = 1 << 4;
const MAX_DOCUMENTS: usize = 1 << 10;
const MIN_TTL: Duration = Duration::from_mins(5);
const DEFAULT_TTL: Duration = Duration::from_hours(1);
const MAX_TTL: Duration = Duration::from_hours(24);

/// Client metadata documents, fetched from their client IDs, with the valid ones cached.
pub(crate) struct Documents {
    /// Built on first use, once the process has installed its TLS crypto provider.
    egress: OnceLock<EgressClient>,
    cache: Mutex<Cache>,
    fetches: Semaphore,
}

impl Documents {
    pub(crate) fn new() -> Self {
        Self {
            egress: OnceLock::new(),
            cache: Mutex::new(Cache::new(MAX_DOCUMENTS)),
            fetches: Semaphore::new(MAX_FETCHES),
        }
    }

    pub(crate) fn with_egress(egress: EgressClient) -> Self {
        Self {
            egress: OnceLock::from(egress),
            ..Self::new()
        }
    }

    /// The metadata in the document at `url`, fetched unless it is cached.
    /// `charge` is called before a fetch, and may refuse it.
    pub(crate) async fn get<F>(
        &self,
        url: &Url,
        clock: &Clock,
        charge: F,
    ) -> Result<Arc<OAuthClientMetadata>, OAuthClientError>
    where
        F: FnOnce() -> Result<(), Throttled>,
    {
        let cached = self.cache().get(url.as_str(), clock.timestamp());
        if let Some(metadata) = cached {
            return Ok(metadata);
        }
        charge().map_err(|Throttled| OAuthClientError::Throttled)?;
        // A lookup that times out holds a blocking thread until the system resolver gives up,
        // so a fetch over the limit fails at once rather than waiting its turn.
        let Ok(_permit) = self.fetches.try_acquire() else {
            return Err(OAuthClientError::Busy);
        };
        let document = self
            .egress()
            .get_json(url, MAX_DOCUMENT_BYTES)
            .await
            .map_err(OAuthClientError::Fetch)?;
        let metadata = Arc::new(
            OAuthClientMetadata::from_document(&document.body, url)
                .map_err(OAuthClientError::Document)?,
        );
        let now = clock.timestamp();
        self.cache().insert(
            url.as_str().to_owned(),
            Arc::clone(&metadata),
            now.saturating_add(ttl(document.max_age)),
            now,
        );
        Ok(metadata)
    }

    fn egress(&self) -> &EgressClient {
        self.egress.get_or_init(|| EgressClient::new(TIMEOUTS))
    }

    fn cache(&self) -> MutexGuard<'_, Cache> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// How long a document stays cached, in seconds: its `max-age`, within bounds.
fn ttl(max_age: Option<Duration>) -> i64 {
    let ttl = max_age.unwrap_or(DEFAULT_TTL).clamp(MIN_TTL, MAX_TTL);
    i64::try_from(ttl.as_secs()).unwrap_or(i64::MAX)
}

/// Valid metadata documents by URL, at most `capacity` of them.
struct Cache {
    capacity: usize,
    documents: HashMap<String, Cached>,
    /// Counts every use, so the least recently used document is the one to evict.
    uses: u64,
}

struct Cached {
    metadata: Arc<OAuthClientMetadata>,
    /// The Unix timestamp the document expires at.
    expiration: i64,
    last_used: u64,
}

impl Cache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            documents: HashMap::new(),
            uses: 0,
        }
    }

    fn get(&mut self, url: &str, now: i64) -> Option<Arc<OAuthClientMetadata>> {
        let use_count = self.next_use();
        let cached = self.documents.get_mut(url)?;
        if cached.expiration > now {
            cached.last_used = use_count;
            return Some(Arc::clone(&cached.metadata));
        }
        self.documents.remove(url);
        None
    }

    fn insert(
        &mut self,
        url: String,
        metadata: Arc<OAuthClientMetadata>,
        expiration: i64,
        now: i64,
    ) {
        if self.documents.len() >= self.capacity && !self.documents.contains_key(&url) {
            self.documents.retain(|_, cached| cached.expiration > now);
            if self.documents.len() >= self.capacity
                && let Some(least_recently_used) = self
                    .documents
                    .iter()
                    .min_by_key(|(_, cached)| cached.last_used)
                    .map(|(url, _)| url.clone())
            {
                self.documents.remove(&least_recently_used);
            }
        }
        let last_used = self.next_use();
        self.documents.insert(
            url,
            Cached {
                metadata,
                expiration,
                last_used,
            },
        );
    }

    fn next_use(&mut self) -> u64 {
        self.uses = self.uses.wrapping_add(1);
        self.uses
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        net::{IpAddr, Ipv4Addr},
        sync::{
            Arc,
            atomic::{AtomicI64, AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use bencher_egress::{EgressClient, EgressPolicy, EgressTimeouts, GetJsonError, Lookup};
    use bencher_json::{Clock, DateTime, OAuthClientMetadata, OAuthClientMetadataError};
    use futures::future::{join, join_all};
    use serde_json::json;
    use tokio::{
        io::{AsyncReadExt as _, AsyncWriteExt as _},
        net::{TcpListener, TcpStream},
        sync::{Notify, Semaphore},
        task::JoinHandle,
    };
    use tokio_rustls::rustls::crypto::aws_lc_rs;
    use url::Url;

    use super::{Cache, Documents, MAX_DOCUMENT_BYTES, MAX_FETCHES};
    use crate::{OAuthClientError, Throttled};

    const NOW: i64 = 1_800_000_000;
    const TIMEOUTS: EgressTimeouts = EgressTimeouts {
        connect: Duration::from_mins(1),
        total: Duration::from_mins(1),
    };

    /// A server on loopback that answers each request with the next response, then stops.
    struct Server {
        url: Url,
        requests: Arc<AtomicUsize>,
        task: JoinHandle<()>,
    }

    impl Server {
        /// `responses` is given the URL of the document the server serves.
        async fn new<F>(responses: F) -> Self
        where
            F: FnOnce(&Url) -> Vec<Vec<u8>>,
        {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
            let address = listener.local_addr().unwrap();
            let url = Url::parse(&format!("http://{address}/oauth/client.json")).unwrap();
            let responses = responses(&url);
            let requests = Arc::new(AtomicUsize::new(0));
            let counted = Arc::clone(&requests);
            let task = tokio::spawn(async move {
                for response in responses {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    read_head(&mut stream).await;
                    counted.fetch_add(1, Ordering::SeqCst);
                    stream.write_all(&response).await.unwrap();
                }
            });
            Self {
                url,
                requests,
                task,
            }
        }

        fn requests(&self) -> usize {
            self.requests.load(Ordering::SeqCst)
        }

        /// Wait for every response to have been served.
        async fn finish(self) {
            self.task.await.unwrap();
        }
    }

    async fn read_head(stream: &mut TcpStream) {
        let mut head = Vec::new();
        let mut chunk = [0; 1024];
        while !head.ends_with(b"\r\n\r\n") {
            let read = stream.read(&mut chunk).await.unwrap();
            assert_ne!(read, 0, "the client closed before the request head ended");
            head.extend_from_slice(&chunk[..read]);
        }
    }

    fn response(
        status: u16,
        content_type: &str,
        cache_control: Option<&str>,
        body: &[u8],
    ) -> Vec<u8> {
        let cache_control = cache_control
            .map(|value| format!("cache-control: {value}\r\n"))
            .unwrap_or_default();
        let mut response = format!(
            "HTTP/1.1 {status} Status\r\ncontent-type: {content_type}\r\n{cache_control}content-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        response
    }

    fn json_response(body: &[u8]) -> Vec<u8> {
        response(200, "application/json", None, body)
    }

    fn document(client_id: &str) -> Vec<u8> {
        json!({
            "client_id": client_id,
            "client_name": "Example",
            "redirect_uris": ["https://client.example/callback"],
        })
        .to_string()
        .into_bytes()
    }

    /// A valid document padded with trailing whitespace to exactly `len` bytes.
    fn document_of_len(client_id: &str, len: usize) -> Vec<u8> {
        let mut document = document(client_id);
        document.resize(len, b' ');
        document
    }

    /// Answers every lookup with nothing: the URLs fetched here are IP literals.
    struct NoLookup;

    #[async_trait]
    impl Lookup for NoLookup {
        async fn lookup(&self, _host: &str) -> io::Result<Vec<IpAddr>> {
            Err(io::Error::from(io::ErrorKind::NotFound))
        }
    }

    fn documents_with(lookup: Arc<dyn Lookup>) -> Documents {
        // Another test in this process may have installed it already.
        let _provider = aws_lc_rs::default_provider().install_default();
        Documents::with_egress(EgressClient::with_lookup(
            EgressPolicy::Loopback,
            TIMEOUTS,
            lookup,
        ))
    }

    fn documents() -> Documents {
        documents_with(Arc::new(NoLookup))
    }

    fn clock(now: &Arc<AtomicI64>) -> Clock {
        let now = Arc::clone(now);
        Clock::Custom(Arc::new(move || {
            DateTime::try_from(now.load(Ordering::SeqCst)).unwrap()
        }))
    }

    fn frozen() -> Clock {
        clock(&Arc::new(AtomicI64::new(NOW)))
    }

    #[expect(clippy::unnecessary_wraps, reason = "a charge that is never refused")]
    fn free() -> Result<(), Throttled> {
        Ok(())
    }

    #[tokio::test]
    async fn a_document_is_cached_for_its_max_age_within_bounds() {
        for (cache_control, ttl) in [
            (Some("max-age=60"), 5 * 60),
            (Some("max-age=1000"), 1000),
            (Some("public, max-age=172800"), 24 * 60 * 60),
            (Some("max-age=99999999999999999999"), 24 * 60 * 60),
            (None, 60 * 60),
        ] {
            let server = Server::new(|url| {
                let answer = response(
                    200,
                    "application/json",
                    cache_control,
                    &document(url.as_str()),
                );
                vec![answer.clone(), answer]
            })
            .await;
            let now = Arc::new(AtomicI64::new(NOW));
            let clock = clock(&now);
            let documents = documents();

            documents.get(&server.url, &clock, free).await.unwrap();
            assert_eq!(server.requests(), 1, "{cache_control:?}");
            now.store(NOW + ttl - 1, Ordering::SeqCst);
            documents.get(&server.url, &clock, free).await.unwrap();
            assert_eq!(server.requests(), 1, "{cache_control:?}");
            now.store(NOW + ttl, Ordering::SeqCst);
            documents.get(&server.url, &clock, free).await.unwrap();
            assert_eq!(server.requests(), 2, "{cache_control:?}");
            server.finish().await;
        }
    }

    #[tokio::test]
    async fn a_failed_fetch_is_never_cached() {
        let server = Server::new(|url| {
            vec![
                response(500, "application/json", None, &document(url.as_str())),
                response(200, "text/plain", None, &document(url.as_str())),
                json_response(&document_of_len(url.as_str(), MAX_DOCUMENT_BYTES + 1)),
                json_response(&document("http://127.0.0.1/oauth/client.json")),
                json_response(b"{\"client_id\":"),
                json_response(&document(url.as_str())),
            ]
        })
        .await;
        let clock = frozen();
        let documents = documents();

        assert!(matches!(
            documents.get(&server.url, &clock, free).await,
            Err(OAuthClientError::Fetch(GetJsonError::Status))
        ));
        assert!(matches!(
            documents.get(&server.url, &clock, free).await,
            Err(OAuthClientError::Fetch(GetJsonError::ContentType))
        ));
        assert!(matches!(
            documents.get(&server.url, &clock, free).await,
            Err(OAuthClientError::Fetch(GetJsonError::TooLarge))
        ));
        assert!(matches!(
            documents.get(&server.url, &clock, free).await,
            Err(OAuthClientError::Document(
                OAuthClientMetadataError::ClientId
            ))
        ));
        assert!(matches!(
            documents.get(&server.url, &clock, free).await,
            Err(OAuthClientError::Document(OAuthClientMetadataError::Json(
                _
            )))
        ));
        let metadata = documents.get(&server.url, &clock, free).await.unwrap();
        assert_eq!(metadata.name.as_ref(), "Example");
        assert_eq!(server.requests(), 6);
        assert_eq!(
            documents.get(&server.url, &clock, free).await.unwrap(),
            metadata
        );
        assert_eq!(server.requests(), 6);
        server.finish().await;
    }

    #[tokio::test]
    async fn a_document_is_at_most_5_kib() {
        let server = Server::new(|url| {
            vec![json_response(&document_of_len(
                url.as_str(),
                MAX_DOCUMENT_BYTES,
            ))]
        })
        .await;
        let metadata: Arc<OAuthClientMetadata> =
            documents().get(&server.url, &frozen(), free).await.unwrap();
        assert_eq!(metadata.name.as_ref(), "Example");
        server.finish().await;
    }

    #[tokio::test]
    async fn only_a_fetch_is_charged() {
        let server = Server::new(|url| vec![json_response(&document(url.as_str()))]).await;
        let clock = frozen();
        let documents = documents();
        let charges = AtomicUsize::new(0);
        let charge = || {
            charges.fetch_add(1, Ordering::SeqCst);
            Ok(())
        };

        documents.get(&server.url, &clock, charge).await.unwrap();
        documents.get(&server.url, &clock, charge).await.unwrap();
        assert_eq!(charges.load(Ordering::SeqCst), 1);
        assert_eq!(server.requests(), 1);
        server.finish().await;

        let refused = Server::new(|url| vec![json_response(&document(url.as_str()))]).await;
        assert!(matches!(
            documents.get(&refused.url, &clock, || Err(Throttled)).await,
            Err(OAuthClientError::Throttled)
        ));
        assert_eq!(refused.requests(), 0);
        documents.get(&refused.url, &clock, free).await.unwrap();
        refused.finish().await;
    }

    /// Holds every lookup until released, so fetches stay in flight.
    struct HeldLookup {
        started: AtomicUsize,
        all_started: Notify,
        release: Semaphore,
    }

    #[async_trait]
    impl Lookup for HeldLookup {
        async fn lookup(&self, _host: &str) -> io::Result<Vec<IpAddr>> {
            if self.started.fetch_add(1, Ordering::SeqCst) + 1 == MAX_FETCHES {
                self.all_started.notify_one();
            }
            let _released = self.release.acquire().await.map_err(io::Error::other)?;
            Err(io::Error::from(io::ErrorKind::NotFound))
        }
    }

    #[tokio::test]
    async fn a_fetch_over_the_limit_fails_at_once() {
        let lookup = Arc::new(HeldLookup {
            started: AtomicUsize::new(0),
            all_started: Notify::new(),
            release: Semaphore::new(0),
        });
        let held: Arc<HeldLookup> = Arc::clone(&lookup);
        let documents = documents_with(held);
        let clock = frozen();
        let urls = (0..=MAX_FETCHES)
            .map(|i| Url::parse(&format!("http://client-{i}.test/oauth/client.json")).unwrap())
            .collect::<Vec<_>>();
        let (held, over) = urls.split_at(MAX_FETCHES);
        let over = &over[0];

        let in_flight = join_all(held.iter().map(|url| documents.get(url, &clock, free)));
        let probe = async {
            lookup.all_started.notified().await;
            let busy =
                tokio::time::timeout(Duration::from_secs(1), documents.get(over, &clock, free))
                    .await
                    .expect("a fetch over the limit waited for a turn");
            assert!(matches!(busy, Err(OAuthClientError::Busy)), "{busy:?}");
            assert_eq!(lookup.started.load(Ordering::SeqCst), MAX_FETCHES);
            lookup.release.add_permits(MAX_FETCHES);
        };
        let (fetched, ()) = join(in_flight, probe).await;
        for result in fetched {
            assert!(
                matches!(result, Err(OAuthClientError::Fetch(_))),
                "{result:?}"
            );
        }

        // A finished fetch frees its place.
        let result = documents.get(over, &clock, free).await;
        assert!(
            matches!(result, Err(OAuthClientError::Fetch(_))),
            "{result:?}"
        );
    }

    #[test]
    fn the_cache_drops_expired_then_least_recently_used_documents() {
        let metadata = Arc::new(
            OAuthClientMetadata::from_document(
                &document("http://127.0.0.1/oauth/client.json"),
                &Url::parse("http://127.0.0.1/oauth/client.json").unwrap(),
            )
            .unwrap(),
        );
        let mut cache = Cache::new(3);
        for url in ["a", "b", "c"] {
            cache.insert(url.to_owned(), Arc::clone(&metadata), NOW + 10, NOW);
        }
        cache.get("a", NOW).unwrap();
        cache.insert("d".to_owned(), Arc::clone(&metadata), NOW + 10, NOW);
        assert!(cache.get("b", NOW).is_none());
        for url in ["a", "c", "d"] {
            assert!(cache.get(url, NOW).is_some(), "{url}");
        }

        cache.insert("e".to_owned(), Arc::clone(&metadata), NOW + 20, NOW);
        assert_eq!(cache.documents.len(), 3);
        assert!(cache.get("a", NOW).is_none());

        cache.insert("f".to_owned(), Arc::clone(&metadata), NOW + 20, NOW + 10);
        assert_eq!(cache.documents.len(), 2);
        assert!(cache.get("e", NOW + 10).is_some());
        assert!(cache.get("f", NOW + 10).is_some());
    }
}
