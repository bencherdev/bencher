use std::{path::PathBuf, sync::Arc};

use bencher_endpoint::Registrar;
use bencher_rbac::init_rbac;
use bencher_schema::{
    context::{Database, DbConnection, Messenger},
    run_migrations, ApiContext,
};
use bencher_token::{TokenKey, DEFAULT_SECRET_KEY};
use diesel::{
    Connection as _,
    r2d2::{ConnectionManager, Pool},
};
use dropshot::{ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingLevel, HttpServer};
use tempfile::NamedTempFile;
use tokio::sync::{Mutex, mpsc};

const ISSUER: &str = "http://localhost:3000";

pub struct TestServer {
    server: HttpServer<ApiContext>,
    pub client: reqwest::Client,
    pub url: String,
    token_key: TokenKey,
    // Keep the temp file alive for the duration of the test
    _db_file: NamedTempFile,
}

impl TestServer {
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

        let context = ApiContext {
            console_url: ISSUER.parse().expect("Invalid console URL"),
            token_key: TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY),
            rbac,
            messenger: Messenger::default(),
            database,
            restart_tx,
            #[cfg(feature = "plus")]
            rate_limiting: bencher_schema::context::RateLimiting::max(),
            #[cfg(feature = "plus")]
            github_client: None,
            #[cfg(feature = "plus")]
            google_client: None,
            #[cfg(feature = "plus")]
            indexer: None,
            #[cfg(feature = "plus")]
            stats: bencher_schema::context::StatsSettings::default(),
            #[cfg(feature = "plus")]
            biller: None,
            #[cfg(feature = "plus")]
            licensor: bencher_license::Licensor::self_hosted()
                .expect("Failed to create licensor"),
            #[cfg(feature = "plus")]
            recaptcha_client: None,
            #[cfg(feature = "plus")]
            is_bencher_cloud: false,
        };

        // Create API description and register endpoints
        let mut api_description = ApiDescription::new();
        bencher_api::api::Api::register(
            &mut api_description,
            false, // http_options
            #[cfg(feature = "plus")]
            false, // is_bencher_cloud
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
