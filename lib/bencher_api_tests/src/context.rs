use std::{path::PathBuf, sync::Arc};

use rustls::crypto::aws_lc_rs;

use bencher_config::DEFAULT_MAX_BODY_SIZE;
use bencher_endpoint::Registrar as _;
#[cfg(feature = "plus")]
use bencher_json::system::config::JsonReplication;
use bencher_rbac::init_rbac;
#[cfg(feature = "plus")]
use bencher_replica::{ReplicaConfig, ReplicaDb, SyncEngine};
use bencher_schema::{
    ApiContext,
    context::{Database, DbConnection, Messenger},
    run_migrations,
};
use bencher_token::{DEFAULT_SECRET_KEY, TokenKey};
use diesel::{
    Connection as _,
    r2d2::{ConnectionManager, Pool},
};
use dropshot::{ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingLevel, HttpServer};
use tempfile::NamedTempFile;
use tokio::sync::Mutex;

const ISSUER: &str = "http://localhost:3000";

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
}

impl TestServer {
    /// Create a new test server with default settings.
    pub async fn new() -> Self {
        Self::build(None, None, None, None, false).await
    }

    /// Create a new test server with custom upload timeout and max body size.
    #[cfg(feature = "plus")]
    pub async fn new_with_limits(upload_timeout: u64, max_body_size: u64) -> Self {
        Self::build(Some(upload_timeout), Some(max_body_size), None, None, false).await
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
            false,
        )
        .await
    }

    /// Create a new test server with a custom runner self-update base URL.
    #[cfg(feature = "plus")]
    pub async fn new_with_runner_update_base_url(base_url: url::Url) -> Self {
        Self::build(None, None, None, Some(base_url), false).await
    }

    /// Create a test server whose database replicates to the given target,
    /// returning the step-driven replication engine alongside it (tests
    /// drive `sync_once` and friends deterministically instead of racing a
    /// background tick task). The database runs in WAL mode with
    /// `wal_autocheckpoint = 0`, matching production replication (I2).
    #[cfg(feature = "plus")]
    #[expect(clippy::expect_used, reason = "test server setup with fallible init")]
    pub async fn new_with_replica(
        replica: JsonReplication,
        clock: Option<bencher_json::Clock>,
        shadow: bool,
    ) -> (Self, SyncEngine<DbConnection>) {
        let server = Self::build(None, None, clock, None, true).await;
        let config = ReplicaConfig::try_from(replica).expect("invalid replica config");
        let context = server.context();
        let log = slog::Logger::root(slog::Discard, slog::o!());
        let engine = SyncEngine::new(
            log,
            config,
            ReplicaDb {
                db_path: camino::Utf8PathBuf::from(server.db_path.clone()),
                writer: context.database.connection.clone(),
                busy_timeout_ms: context.database.busy_timeout,
            },
            context.clock.clone(),
            shadow,
        )
        .await
        .expect("failed to build replication engine");
        (server, engine)
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
        replicated: bool,
    ) -> Self {
        // Create logger early so it can be used for OCI storage
        let log_config = ConfigLogging::StderrTerminal {
            level: ConfigLoggingLevel::Warn,
        };
        let log = log_config
            .to_logger("bencher_api_tests")
            .expect("Failed to create logger");

        // Create a temporary database file
        let db_file = NamedTempFile::new().expect("Failed to create temp db file");
        let db_path = db_file.path().to_str().expect("Invalid db path").to_owned();

        // Establish connection and run migrations
        let mut conn =
            DbConnection::establish(&db_path).expect("Failed to establish database connection");
        if replicated {
            // Match the production replication PRAGMAs (invariant I2: the
            // replicator is the sole checkpointer). journal_mode is persistent
            // in the database header; the rest route through the shared
            // standalone-connection configurator so tests exercise the same
            // PRAGMAs as production.
            diesel::connection::SimpleConnection::batch_execute(
                &mut conn,
                "PRAGMA journal_mode = WAL",
            )
            .expect("Failed to set WAL journal mode");
            bencher_schema::context::configure_standalone_connection(&mut conn, 5_000, true)
                .expect("Failed to set replication PRAGMAs");
        }
        run_migrations(&mut conn).expect("Failed to run migrations");

        // Create connection pools
        let public_pool = Pool::builder()
            .max_size(2)
            .build(ConnectionManager::<DbConnection>::new(&db_path))
            .expect("Failed to create public pool");
        let auth_pool = Pool::builder()
            .max_size(2)
            .build(ConnectionManager::<DbConnection>::new(&db_path))
            .expect("Failed to create auth pool");

        // Build minimal ApiContext
        let token_key = TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let rbac = init_rbac().expect("Failed to init RBAC").into();

        let database = Database {
            path: PathBuf::from(&db_path),
            busy_timeout: 5_000,
            replicated,
            public_pool,
            auth_pool,
            connection: Arc::new(Mutex::new(conn)),
            data_store: None,
        };

        let request_body_max_bytes = max_body_size.map_or(DEFAULT_MAX_BODY_SIZE, |s| {
            usize::try_from(s).expect("max_body_size exceeds usize")
        });
        let context = ApiContext {
            console_url: ISSUER.parse().expect("Invalid console URL"),
            request_body_max_bytes,
            token_key: TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY),
            rbac,
            messenger: Messenger::default(),
            database,
            rate_limiting: Arc::new(bencher_schema::context::RateLimiting::max()),
            github_client: None,
            google_client: None,
            indexer: None,
            stats: bencher_schema::context::StatsSettings::default(),
            biller: None,
            licensor: bencher_license::Licensor::self_hosted().expect("Failed to create licensor"),
            recaptcha_client: None,
            is_bencher_cloud: false,
            clock: clock.clone().unwrap_or(bencher_json::Clock::System),
            registry_url: bencher_json::LOCALHOST_BENCHER_REGISTRY_URL.clone(),
            oci_storage: bencher_oci_storage::OciStorage::try_from_config(
                log.clone(),
                None,
                std::path::Path::new(&db_path),
                upload_timeout,
                max_body_size,
                clock,
            )
            .expect("Failed to create OCI storage"),
            heartbeat_timeout: std::time::Duration::from_secs(5),
            job_timeout_grace_period: std::time::Duration::from_mins(1),
            heartbeat_tasks: bencher_schema::context::HeartbeatTasks::new(),
            runner_update: bencher_schema::context::RunnerUpdate::new(runner_update_base_url),
            shutdown: bencher_schema::context::CancellationToken::new(),
        };

        Self::start_server(context, &log, token_key, db_path, db_file)
    }

    #[cfg(not(feature = "plus"))]
    #[expect(
        clippy::expect_used,
        clippy::unused_async,
        clippy::too_many_lines,
        reason = "test server setup with fallible init; async for API parity"
    )]
    async fn build(
        upload_timeout: Option<u64>,
        max_body_size: Option<u64>,
        _clock: Option<()>,
        _runner_update_base_url: Option<url::Url>,
        _replicated: bool,
    ) -> Self {
        // Create logger early so it can be used for OCI storage
        let log_config = ConfigLogging::StderrTerminal {
            level: ConfigLoggingLevel::Warn,
        };
        let log = log_config
            .to_logger("bencher_api_tests")
            .expect("Failed to create logger");

        // Create a temporary database file
        let db_file = NamedTempFile::new().expect("Failed to create temp db file");
        let db_path = db_file.path().to_str().expect("Invalid db path").to_owned();

        // Establish connection and run migrations
        let mut conn =
            DbConnection::establish(&db_path).expect("Failed to establish database connection");
        run_migrations(&mut conn).expect("Failed to run migrations");

        // Create connection pools
        let public_pool = Pool::builder()
            .max_size(2)
            .build(ConnectionManager::<DbConnection>::new(&db_path))
            .expect("Failed to create public pool");
        let auth_pool = Pool::builder()
            .max_size(2)
            .build(ConnectionManager::<DbConnection>::new(&db_path))
            .expect("Failed to create auth pool");

        // Build minimal ApiContext
        let token_key = TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let rbac = init_rbac().expect("Failed to init RBAC").into();

        let database = Database {
            path: PathBuf::from(&db_path),
            busy_timeout: 5_000,
            replicated: false,
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

    /// Shut down the server gracefully
    pub async fn close(self) {
        drop(self.server.close().await);
    }
}
