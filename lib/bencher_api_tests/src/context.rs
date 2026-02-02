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
    /// Keep the temp file alive for the duration of the test
    _db_file: NamedTempFile,
}

impl TestServer {
    /// Create a new test server with an in-memory database.
    #[expect(clippy::expect_used, clippy::unused_async)]
    pub async fn new() -> Self {
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

        #[cfg(feature = "plus")]
        let context = ApiContext {
            console_url: ISSUER.parse().expect("Invalid console URL"),
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
            oci_storage: bencher_oci_storage::OciStorage::try_from_config(
                None,
                std::path::Path::new(&db_path),
                None, // Use default upload timeout
            )
            .expect("Failed to create OCI storage"),
        };
        #[cfg(not(feature = "plus"))]
        let context = ApiContext {
            console_url: ISSUER.parse().expect("Invalid console URL"),
            token_key: TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY),
            rbac,
            messenger: Messenger::default(),
            database,
            restart_tx,
        };

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

        // Create logger
        let log_config = ConfigLogging::StderrTerminal {
            level: ConfigLoggingLevel::Warn,
        };
        let log = log_config
            .to_logger("bencher_api_tests")
            .expect("Failed to create logger");

        // Start the server
        let server = dropshot::HttpServerStarter::new(&config, api_description, context, &log)
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
            _db_file: db_file,
        }
    }

    /// Get the token key for generating test tokens
    pub fn token_key(&self) -> &TokenKey {
        &self.token_key
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
