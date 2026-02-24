use std::{path::PathBuf, sync::Arc};

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
    r2d2::{ConnectionManager, Pool},
};
use dropshot::{ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingLevel, HttpServer};
use tempfile::NamedTempFile;
use tokio::sync::{Mutex, mpsc};

const ISSUER: &str = "http://localhost:3000";

/// A test server for running API integration tests.
#[expect(clippy::partial_pub_fields)]
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
        Self::build(None, None, None).await
    }

    /// Create a new test server with custom upload timeout and max body size.
    #[cfg(feature = "plus")]
    pub async fn new_with_limits(upload_timeout: u64, max_body_size: u64) -> Self {
        Self::build(Some(upload_timeout), Some(max_body_size), None).await
    }

    /// Create a new test server with custom upload timeout, max body size, and injectable clock.
    #[cfg(feature = "plus")]
    pub async fn new_with_clock(
        upload_timeout: u64,
        max_body_size: u64,
        clock: bencher_json::Clock,
    ) -> Self {
        Self::build(Some(upload_timeout), Some(max_body_size), Some(clock)).await
    }

    #[cfg(feature = "plus")]
    #[expect(clippy::expect_used, clippy::unused_async)]
    async fn build(
        upload_timeout: Option<u64>,
        max_body_size: Option<u64>,
        clock: Option<bencher_json::Clock>,
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
        let (restart_tx, _restart_rx) = mpsc::channel(1);

        let database = Database {
            path: PathBuf::from(&db_path),
            public_pool,
            auth_pool,
            connection: Arc::new(Mutex::new(conn)),
            data_store: None,
        };

        let context = ApiContext {
            console_url: ISSUER.parse().expect("Invalid console URL"),
            request_body_max_bytes: 1024 * 1024,
            token_key: TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY),
            rbac,
            messenger: Messenger::default(),
            database,
            restart_tx,
            rate_limiting: bencher_schema::context::RateLimiting::max(),
            github_client: None,
            google_client: None,
            indexer: None,
            stats: bencher_schema::context::StatsSettings::default(),
            biller: None,
            licensor: bencher_license::Licensor::self_hosted().expect("Failed to create licensor"),
            recaptcha_client: None,
            is_bencher_cloud: false,
            clock: clock.clone().unwrap_or(bencher_json::Clock::System),
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
            job_timeout_grace_period: std::time::Duration::from_secs(60),
            heartbeat_tasks: bencher_schema::context::HeartbeatTasks::new(),
        };

        Self::start_server(context, &log, token_key, db_path, db_file)
    }

    #[cfg(not(feature = "plus"))]
    #[expect(clippy::expect_used, clippy::unused_async, clippy::too_many_lines)]
    async fn build(
        upload_timeout: Option<u64>,
        max_body_size: Option<u64>,
        _clock: Option<()>,
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
        let (restart_tx, _restart_rx) = mpsc::channel(1);

        let database = Database {
            path: PathBuf::from(&db_path),
            public_pool,
            auth_pool,
            connection: Arc::new(Mutex::new(conn)),
            data_store: None,
        };

        let _ = (upload_timeout, max_body_size);
        let context = ApiContext {
            console_url: ISSUER.parse().expect("Invalid console URL"),
            request_body_max_bytes: 1024 * 1024,
            token_key: TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY),
            rbac,
            messenger: Messenger::default(),
            database,
            restart_tx,
            clock: bencher_json::Clock::System,
        };

        Self::start_server(context, &log, token_key, db_path, db_file)
    }

    #[expect(clippy::expect_used)]
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
            default_request_body_max_bytes: 1024 * 1024,
            default_handler_task_mode: dropshot::HandlerTaskMode::Detached,
            log_headers: Vec::new(),
        };

        // Start the server
        let server = dropshot::HttpServerStarter::new(&config, api_description, context, log)
            .expect("Failed to create server")
            .start();

        let url = format!("http://{}", server.local_addr());

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

    /// Get a database connection for test setup.
    /// Use this to insert test data directly into the database.
    #[expect(clippy::expect_used)]
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

    /// Create a bearer token header value
    pub fn bearer(&self, token: &str) -> String {
        format!("Bearer {token}")
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
