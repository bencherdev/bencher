use std::{path::PathBuf, sync::Arc};

use rustls::crypto::aws_lc_rs;

use bencher_config::DEFAULT_MAX_BODY_SIZE;
use bencher_endpoint::Registrar as _;
use bencher_rbac::init_rbac;
use bencher_schema::{
    ApiContext,
    context::{Database, DbConnection, Messenger},
    run_migrations,
};
use bencher_token::{DEFAULT_SECRET_KEY, TokenKey};
use diesel::{
    Connection as _,
    connection::SimpleConnection as _,
    r2d2::{ConnectionManager, CustomizeConnection, Pool},
};
use dropshot::{ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingLevel, HttpServer};
use tempfile::NamedTempFile;
use tokio::sync::Mutex;

const ISSUER: &str = "http://localhost:3000";
/// How long a connection waits out another's lock, as the server's own connections do.
const BUSY_TIMEOUT_MS: u32 = 5_000;

/// A test server for running API integration tests.
#[expect(
    clippy::partial_pub_fields,
    reason = "intentional mix of pub and private fields with accessors"
)]
pub struct TestServer {
    /// The Dropshot HTTP server (private - use `close()` to shut down)
    server: HttpServer<ApiContext>,
    /// HTTP client for making requests
    pub client: reqwest::Client,
    /// Base URL of the server
    pub url: String,
    /// Token key for generating test tokens (private - use `token_key()` accessor)
    token_key: TokenKey,
    /// Database path for test setup (private - use `db_conn()` accessor)
    db_path: String,
    /// Keep the temp file alive for the duration of the test
    db_file: NamedTempFile,
    /// Receives every callback the server sends
    #[cfg(feature = "plus")]
    callback_sender: Arc<crate::RecordingSender>,
}

impl TestServer {
    /// Create a new test server with default settings.
    pub async fn new() -> Self {
        Self::build(None, None, None, None, None, None).await
    }

    /// Create a new test server that logs to `log` instead of stderr.
    pub async fn new_with_log(log: slog::Logger) -> Self {
        Self::build(None, None, None, None, None, Some(log)).await
    }

    /// Create a new test server with custom upload timeout and max body size.
    #[cfg(feature = "plus")]
    pub async fn new_with_limits(upload_timeout: u64, max_body_size: u64) -> Self {
        Self::build(
            Some(upload_timeout),
            Some(max_body_size),
            None,
            None,
            None,
            None,
        )
        .await
    }

    /// Create a new test server with custom upload timeout, max body size, and injectable clock.
    #[cfg(feature = "plus")]
    pub async fn new_with_clock(
        upload_timeout: u64,
        max_body_size: u64,
        clock: bencher_json::Clock,
    ) -> Self {
        Self::build(
            Some(upload_timeout),
            Some(max_body_size),
            Some(clock),
            None,
            None,
            None,
        )
        .await
    }

    /// Create a new test server whose clock is frozen at the given time.
    #[cfg(feature = "plus")]
    pub async fn new_at(now: bencher_json::DateTime) -> Self {
        let clock = bencher_json::Clock::Custom(Arc::new(move || now));
        Self::build(None, None, Some(clock), None, None, None).await
    }

    /// Create a new test server with a custom runner self-update base URL.
    #[cfg(feature = "plus")]
    pub async fn new_with_runner_update_base_url(base_url: url::Url) -> Self {
        Self::build(None, None, None, Some(base_url), None, None).await
    }

    /// Create a new test server with custom creation rate limits.
    ///
    /// Only the database-backed creation ceilings move: the in-memory request
    /// limiters stay wide open, so a test reaches the ceiling it is testing rather
    /// than a throttle on the requests that get it there.
    #[cfg(feature = "plus")]
    pub async fn new_with_creation_limits(unclaimed_limit: u32, claimed_limit: u32) -> Self {
        let rate_limiting = bencher_schema::context::RateLimiting::max_with_creation_limits(
            unclaimed_limit,
            claimed_limit,
        );
        Self::build(None, None, None, None, Some(rate_limiting), None).await
    }

    #[cfg(feature = "plus")]
    #[expect(
        clippy::expect_used,
        clippy::unused_async,
        reason = "test server setup with fallible init; async for API parity"
    )]
    async fn build(
        upload_timeout: Option<u64>,
        max_body_size: Option<u64>,
        clock: Option<bencher_json::Clock>,
        runner_update_base_url: Option<url::Url>,
        rate_limiting: Option<bencher_schema::context::RateLimiting>,
        log: Option<slog::Logger>,
    ) -> Self {
        // Create logger early so it can be used for OCI storage
        let log = log.unwrap_or_else(stderr_logger);

        // Create a temporary database file
        let db_file = NamedTempFile::new().expect("Failed to create temp db file");
        let db_path = db_file.path().to_str().expect("Invalid db path").to_owned();

        // Establish connection and run migrations
        let mut conn =
            DbConnection::establish(&db_path).expect("Failed to establish database connection");
        set_busy_timeout(&mut conn).expect("Failed to set the busy timeout");
        run_migrations(&mut conn).expect("Failed to run migrations");

        // Create connection pools
        let public_pool = connection_pool(&db_path);
        let auth_pool = connection_pool(&db_path);

        // Build minimal ApiContext
        let token_key = TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let rbac = init_rbac().expect("Failed to init RBAC").into();

        let database = Database {
            path: PathBuf::from(&db_path),
            busy_timeout: BUSY_TIMEOUT_MS,
            public_pool,
            auth_pool,
            connection: Arc::new(Mutex::new(conn)),
            data_store: None,
        };

        let request_body_max_bytes = max_body_size.map_or(DEFAULT_MAX_BODY_SIZE, |s| {
            usize::try_from(s).expect("max_body_size exceeds usize")
        });
        let clock = clock.unwrap_or(bencher_json::Clock::System);
        let callback_key = bencher_callback::CallbackKey::new(&DEFAULT_SECRET_KEY)
            .expect("Failed to derive callback key");
        let shutdown = bencher_schema::context::CancellationToken::new();
        // Callbacks go to the recording sender, never through the real client.
        let callback_sender = Arc::new(crate::RecordingSender::default());
        let callbacks = bencher_schema::context::Callbacks::new(
            Arc::clone(&database.connection),
            database.auth_pool.clone(),
            callback_key.clone(),
            callback_sender.clone(),
            shutdown.clone(),
            clock.clone(),
        );
        let context = ApiContext {
            console_url: ISSUER.parse().expect("Invalid console URL"),
            request_body_max_bytes,
            token_key: TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY),
            rbac,
            messenger: Messenger::default(),
            database,
            rate_limiting: Arc::new(
                rate_limiting.unwrap_or_else(bencher_schema::context::RateLimiting::max),
            ),
            // The test server never spawns the periodic prune, so the slot stays empty.
            rate_limiting_prune: bencher_schema::context::RateLimitingPruneTask::default(),
            github_client: None,
            google_client: None,
            indexer: None,
            stats: bencher_schema::context::StatsSettings::default(),
            biller: None,
            licensor: bencher_license::Licensor::self_hosted().expect("Failed to create licensor"),
            recaptcha_client: None,
            is_bencher_cloud: false,
            clock: clock.clone(),
            registry_url: bencher_json::LOCALHOST_BENCHER_REGISTRY_URL.clone(),
            oci_storage: bencher_oci_storage::OciStorage::try_from_config(
                log.clone(),
                None,
                std::path::Path::new(&db_path),
                upload_timeout,
                max_body_size,
                Some(clock),
            )
            .expect("Failed to create OCI storage"),
            heartbeat_timeout: std::time::Duration::from_secs(5),
            job_timeout_grace_period: std::time::Duration::from_mins(1),
            heartbeat_tasks: bencher_schema::context::HeartbeatTasks::new(),
            callback_key,
            callbacks,
            runner_update: bencher_schema::context::RunnerUpdate::new(runner_update_base_url),
            shutdown,
        };

        Self::start_server(context, &log, token_key, db_path, db_file, callback_sender)
    }

    #[cfg(not(feature = "plus"))]
    #[expect(
        clippy::expect_used,
        clippy::unused_async,
        reason = "test server setup with fallible init; async for API parity"
    )]
    async fn build(
        upload_timeout: Option<u64>,
        max_body_size: Option<u64>,
        _clock: Option<()>,
        _runner_update_base_url: Option<url::Url>,
        _rate_limiting: Option<()>,
        log: Option<slog::Logger>,
    ) -> Self {
        let log = log.unwrap_or_else(stderr_logger);

        // Create a temporary database file
        let db_file = NamedTempFile::new().expect("Failed to create temp db file");
        let db_path = db_file.path().to_str().expect("Invalid db path").to_owned();

        // Establish connection and run migrations
        let mut conn =
            DbConnection::establish(&db_path).expect("Failed to establish database connection");
        set_busy_timeout(&mut conn).expect("Failed to set the busy timeout");
        run_migrations(&mut conn).expect("Failed to run migrations");

        // Create connection pools
        let public_pool = connection_pool(&db_path);
        let auth_pool = connection_pool(&db_path);

        // Build minimal ApiContext
        let token_key = TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let rbac = init_rbac().expect("Failed to init RBAC").into();

        let database = Database {
            path: PathBuf::from(&db_path),
            busy_timeout: BUSY_TIMEOUT_MS,
            public_pool,
            auth_pool,
            connection: Arc::new(Mutex::new(conn)),
            data_store: None,
        };

        let _ = (upload_timeout, max_body_size);
        let context = ApiContext {
            console_url: ISSUER.parse().expect("Invalid console URL"),
            request_body_max_bytes: DEFAULT_MAX_BODY_SIZE,
            token_key: TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY),
            rbac,
            messenger: Messenger::default(),
            database,
            clock: bencher_json::Clock::System,
        };

        Self::start_server(context, &log, token_key, db_path, db_file)
    }

    #[expect(clippy::expect_used, reason = "test server startup with fallible init")]
    fn start_server(
        context: ApiContext,
        log: &slog::Logger,
        token_key: TokenKey,
        db_path: String,
        db_file: NamedTempFile,
        #[cfg(feature = "plus")] callback_sender: Arc<crate::RecordingSender>,
    ) -> Self {
        // Create API description and register endpoints
        let mut api_description = ApiDescription::new();
        #[cfg(feature = "plus")]
        bencher_api::api::Api::register(
            &mut api_description,
            false, // http_options
            false, // is_bencher_cloud
        )
        .expect("Failed to register endpoints");
        #[cfg(not(feature = "plus"))]
        bencher_api::api::Api::register(
            &mut api_description,
            false, // http_options
        )
        .expect("Failed to register endpoints");

        // Configure server to bind to random port
        let config = ConfigDropshot {
            bind_address: "127.0.0.1:0".parse().expect("Invalid bind address"),
            default_request_body_max_bytes: DEFAULT_MAX_BODY_SIZE,
            default_handler_task_mode: dropshot::HandlerTaskMode::Detached,
            log_headers: Vec::new(),
            compression: dropshot::CompressionConfig::default(),
        };

        // Start the server
        let server = dropshot::HttpServerStarter::new(&config, api_description, context, log)
            .expect("Failed to create server")
            .start();

        let url = format!("http://{}", server.local_addr());

        let _provider = aws_lc_rs::default_provider().install_default();

        let client = reqwest::Client::builder()
            .build()
            .expect("Failed to create HTTP client");

        Self {
            server,
            client,
            url,
            token_key,
            db_path,
            db_file,
            #[cfg(feature = "plus")]
            callback_sender,
        }
    }

    /// Get the shared API context for direct access (e.g. OCI storage, clock).
    pub fn context(&self) -> &ApiContext {
        self.server.app_private()
    }

    /// Get a database connection for test setup.
    /// Use this to insert test data directly into the database.
    #[expect(clippy::expect_used, reason = "test helper establishing DB connection")]
    pub fn db_conn(&self) -> DbConnection {
        DbConnection::establish(&self.db_path).expect("Failed to establish database connection")
    }

    /// Get the token key for generating test tokens
    pub fn token_key(&self) -> &TokenKey {
        &self.token_key
    }

    /// Get the path to the temporary database file
    pub fn db_path(&self) -> &std::path::Path {
        self.db_file.path()
    }

    /// Get the base URL for API requests
    pub fn api_url(&self, path: &str) -> String {
        format!("{}{}", self.url, path)
    }

    /// The callbacks the server has sent, in the order it sent them.
    #[cfg(feature = "plus")]
    pub fn callback_requests(&self) -> Vec<crate::CallbackRequest> {
        self.callback_sender.requests()
    }

    /// Wait for every callback delivery to settle; the server claims no callback afterwards.
    #[cfg(feature = "plus")]
    pub async fn drain_callbacks(&self) {
        let log = slog::Logger::root(slog::Discard, slog::o!());
        self.context().callbacks.drain(&log).await;
    }

    /// Shut down the server gracefully
    pub async fn close(self) {
        #[cfg(feature = "plus")]
        let callbacks = self.context().callbacks.clone();
        #[cfg(feature = "plus")]
        self.context().shutdown.cancel();
        drop(self.server.close().await);
        #[cfg(feature = "plus")]
        callbacks
            .drain(&slog::Logger::root(slog::Discard, slog::o!()))
            .await;
    }
}

/// Waits out a lock instead of failing at once, as the server's own pools do.
#[derive(Debug)]
struct BusyTimeout;

impl CustomizeConnection<DbConnection, diesel::r2d2::Error> for BusyTimeout {
    fn on_acquire(&self, conn: &mut DbConnection) -> Result<(), diesel::r2d2::Error> {
        set_busy_timeout(conn).map_err(diesel::r2d2::Error::QueryError)
    }
}

fn set_busy_timeout(conn: &mut DbConnection) -> diesel::QueryResult<()> {
    conn.batch_execute(&format!("PRAGMA busy_timeout = {BUSY_TIMEOUT_MS}"))
}

#[expect(clippy::expect_used, reason = "test server setup with fallible init")]
fn connection_pool(db_path: &str) -> Pool<ConnectionManager<DbConnection>> {
    Pool::builder()
        .max_size(2)
        .connection_customizer(Box::new(BusyTimeout))
        .build(ConnectionManager::<DbConnection>::new(db_path))
        .expect("Failed to create a connection pool")
}

#[expect(clippy::expect_used, reason = "test server setup with fallible init")]
fn stderr_logger() -> slog::Logger {
    ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Warn,
    }
    .to_logger("bencher_api_tests")
    .expect("Failed to create logger")
}
