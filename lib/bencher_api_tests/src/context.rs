use std::{path::PathBuf, sync::Arc};

use rustls::crypto::aws_lc_rs;

use bencher_config::DEFAULT_MAX_BODY_SIZE;
use bencher_endpoint::Registrar as _;
use bencher_json::{Email, EmailHash, SessionToken, SessionTokenHash};
use bencher_rbac::init_rbac;
use bencher_schema::{
    ApiContext,
    context::{Database, DbConnection, Messenger, Relay},
    model::{
        organization::OrganizationId,
        user::{
            QueryUser,
            sign_in::{CODE_BUDGET, CodeKey},
        },
    },
    run_migrations_with_token_key, schema,
};
use bencher_token::{DEFAULT_SECRET_KEY, TokenKey};
use diesel::{
    Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
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
        Self::build(None, None, None, None, None, None, false).await
    }

    /// Create a new test server that logs to `log` instead of stderr.
    pub async fn new_with_log(log: slog::Logger) -> Self {
        Self::build(None, None, None, None, None, Some(log), false).await
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
            false,
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
            false,
        )
        .await
    }

    /// Create a new test server with an injectable clock that logs to `log` instead of stderr.
    #[cfg(feature = "plus")]
    pub async fn new_with_clock_and_log(clock: bencher_json::Clock, log: slog::Logger) -> Self {
        Self::build(None, None, Some(clock), None, None, Some(log), false).await
    }

    /// Create a new test server whose clock is frozen at the given time.
    #[cfg(feature = "plus")]
    pub async fn new_at(now: bencher_json::DateTime) -> Self {
        let clock = bencher_json::Clock::Custom(Arc::new(move || now));
        Self::build(None, None, Some(clock), None, None, None, false).await
    }

    /// Create a new test server with a custom runner self-update base URL.
    #[cfg(feature = "plus")]
    pub async fn new_with_runner_update_base_url(base_url: url::Url) -> Self {
        Self::build(None, None, None, Some(base_url), None, None, false).await
    }

    /// Create a new test server that also registers the Bencher Cloud only endpoints,
    /// with no biller and `is_bencher_cloud` still false.
    #[cfg(feature = "plus")]
    pub async fn new_with_cloud_routes() -> Self {
        Self::build(None, None, None, None, None, None, true).await
    }

    /// Create a new test server with custom creation rate limits.
    ///
    /// Only the database-backed creation ceilings move: the in-memory request
    /// limiters stay wide open, so a test reaches the ceiling it is testing rather
    /// than a throttle on the requests that get it there.
    #[cfg(feature = "plus")]
    pub async fn new_with_creation_limits(unclaimed_limit: u32, claimed_limit: u32) -> Self {
        Self::new_with_creation_and_plus_limits(unclaimed_limit, claimed_limit, u32::MAX).await
    }

    /// [`Self::new_with_creation_limits`] with the daily metrics limit of a paid
    /// organization set too.
    #[cfg(feature = "plus")]
    pub async fn new_with_creation_and_plus_limits(
        unclaimed_limit: u32,
        claimed_limit: u32,
        plus_limit: u32,
    ) -> Self {
        let rate_limiting =
            bencher_schema::context::RateLimiting::max_with_creation_and_plus_limits(
                unclaimed_limit,
                claimed_limit,
                plus_limit,
            );
        Self::build(None, None, None, None, Some(rate_limiting), None, false).await
    }

    /// Create a new test server with the given rate limits, in-memory request
    /// limiters included, for tests of the request throttle itself.
    #[cfg(feature = "plus")]
    pub async fn new_with_rate_limiting(
        rate_limiting: bencher_schema::context::RateLimiting,
    ) -> Self {
        Self::build(None, None, None, None, Some(rate_limiting), None, false).await
    }

    /// Create a new test server that serves `challenge` at `/.well-known/openai-apps-challenge`.
    #[cfg(feature = "plus")]
    pub async fn new_with_openai_apps_challenge(challenge: bencher_json::NonEmpty) -> Self {
        Self::new_with_options(TestServerOptions {
            openai_apps_challenge: Some(challenge),
            ..TestServerOptions::default()
        })
        .await
    }

    /// Create a new test server with the GitHub App, whose every GitHub request goes to `base`,
    /// with an injectable clock that logs to `log`.
    #[cfg(feature = "plus")]
    pub async fn new_with_github(
        base: url::Url,
        clock: bencher_json::Clock,
        log: slog::Logger,
    ) -> Self {
        Self::new_with_options(TestServerOptions {
            clock: Some(clock),
            log: Some(log),
            github_client: Some(crate::helpers::github_client().with_base_url(base.clone())),
            github_app: Some(Arc::new(crate::github::app(base))),
            ..TestServerOptions::default()
        })
        .await
    }

    #[cfg(feature = "plus")]
    async fn build(
        upload_timeout: Option<u64>,
        max_body_size: Option<u64>,
        clock: Option<bencher_json::Clock>,
        runner_update_base_url: Option<url::Url>,
        rate_limiting: Option<bencher_schema::context::RateLimiting>,
        log: Option<slog::Logger>,
        cloud_routes: bool,
    ) -> Self {
        Self::new_with_options(TestServerOptions {
            upload_timeout,
            max_body_size,
            clock,
            runner_update_base_url,
            rate_limiting,
            log,
            cloud_routes,
            openai_apps_challenge: None,
            oauth: None,
            github_client: None,
            github_app: None,
            google_client: None,
            relay_secret: None,
            turnstile: None,
            messenger: None,
            http_options: false,
        })
        .await
    }

    /// Create a new test server with any of its settings changed from the defaults.
    /// The server is built at the call, so a caller's future never holds the options.
    #[cfg(feature = "plus")]
    pub fn new_with_options(options: TestServerOptions) -> std::future::Ready<Self> {
        let (database, db_path, db_file) = temp_database();
        std::future::ready(Self::with_database(options, database, db_path, db_file))
    }

    /// Stop this server and start another on the same database file, with these settings and
    /// nothing else carried over, as the API's own restart does.
    #[cfg(feature = "plus")]
    pub async fn restart(self, options: TestServerOptions) -> Self {
        let Self {
            server,
            db_path,
            db_file,
            ..
        } = self;
        Self::shut_down(server).await;
        Self::with_database(options, database_at(&db_path), db_path, db_file)
    }

    #[cfg(feature = "plus")]
    #[expect(clippy::expect_used, reason = "test server setup with fallible init")]
    fn with_database(
        options: TestServerOptions,
        database: Database,
        db_path: String,
        db_file: NamedTempFile,
    ) -> Self {
        let TestServerOptions {
            upload_timeout,
            max_body_size,
            clock,
            runner_update_base_url,
            rate_limiting,
            log,
            cloud_routes,
            openai_apps_challenge,
            oauth,
            github_client,
            github_app,
            google_client,
            relay_secret,
            turnstile,
            messenger,
            http_options,
        } = options;
        // Create logger early so it can be used for OCI storage
        let log = log.unwrap_or_else(stderr_logger);

        // Build minimal ApiContext
        let token_key = TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let rbac = init_rbac().expect("Failed to init RBAC").into();

        let request_body_max_bytes = max_body_size.map_or(DEFAULT_MAX_BODY_SIZE, |s| {
            usize::try_from(s).expect("max_body_size exceeds usize")
        });
        let clock = clock.unwrap_or(bencher_json::Clock::System);
        let callback_key = bencher_callback::CallbackKey::new(&DEFAULT_SECRET_KEY)
            .expect("Failed to derive callback key");
        let shutdown = bencher_schema::context::CancellationToken::new();
        // Callbacks go to the recording sender, never through the real client.
        let callback_sender = Arc::new(crate::RecordingSender::default());
        let (callbacks, commit_titles) = background_tasks(
            &log,
            &database,
            &callback_key,
            &callback_sender,
            &shutdown,
            &clock,
            github_app.as_ref(),
        );
        let context = ApiContext {
            console_url: ISSUER.parse().expect("Invalid console URL"),
            request_body_max_bytes,
            token_key: TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY),
            rbac,
            messenger: messenger.unwrap_or_default(),
            database,
            relay: Relay::new(relay_secret.as_ref()),
            code_key: code_key(),
            rate_limiting: Arc::new(
                rate_limiting.unwrap_or_else(bencher_schema::context::RateLimiting::max),
            ),
            // The test server never spawns the periodic prune, so the slot stays empty.
            rate_limiting_prune: bencher_schema::context::RateLimitingPruneTask::default(),
            github_client,
            github_app,
            google_client,
            indexer: None,
            stats: bencher_schema::context::StatsSettings::default(),
            biller: None,
            licensor: bencher_license::Licensor::self_hosted().expect("Failed to create licensor"),
            turnstile,
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
            commit_titles,
            runner_update: bencher_schema::context::RunnerUpdate::new(runner_update_base_url),
            openai_apps_challenge,
            oauth,
            shutdown,
        };

        Self::start_server(
            context,
            &log,
            token_key,
            db_path,
            db_file,
            callback_sender,
            Routes {
                cloud: cloud_routes,
                http_options,
            },
        )
    }

    #[cfg(not(feature = "plus"))]
    #[expect(
        clippy::expect_used,
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "test server setup with fallible init; async for API parity"
    )]
    async fn build(
        upload_timeout: Option<u64>,
        max_body_size: Option<u64>,
        _clock: Option<()>,
        _runner_update_base_url: Option<url::Url>,
        _rate_limiting: Option<()>,
        log: Option<slog::Logger>,
        _cloud_routes: bool,
    ) -> Self {
        let log = log.unwrap_or_else(stderr_logger);

        let (database, db_path, db_file) = temp_database();

        // Build minimal ApiContext
        let token_key = TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let rbac = init_rbac().expect("Failed to init RBAC").into();

        let _ = (upload_timeout, max_body_size);
        let context = ApiContext {
            console_url: ISSUER.parse().expect("Invalid console URL"),
            request_body_max_bytes: DEFAULT_MAX_BODY_SIZE,
            token_key: TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY),
            rbac,
            messenger: Messenger::default(),
            database,
            relay: Relay::new(None),
            code_key: code_key(),
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
        #[cfg(feature = "plus")] routes: Routes,
    ) -> Self {
        // Create API description and register endpoints
        let mut api_description = ApiDescription::new();
        #[cfg(feature = "plus")]
        bencher_api::api::Api::register(&mut api_description, routes.http_options, routes.cloud)
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
    /// Use this to insert test data directly into the database, or to run its migrations.
    #[expect(clippy::expect_used, reason = "test helper establishing DB connection")]
    pub fn db_conn(&self) -> DbConnection {
        let mut conn = DbConnection::establish(&self.db_path)
            .expect("Failed to establish database connection");
        bencher_schema::register_migration_functions(&mut conn, Some(&self.token_key))
            .expect("Failed to register the migration functions");
        conn
    }

    /// Revert the migrations, newest first, through the one of `version`, so the database is in
    /// the shape that migration found it in, and a [`Self::restart`] runs them again.
    #[expect(clippy::expect_used, reason = "test helper reverting migrations")]
    pub fn revert_migrations_through(&self, version: &str) {
        let mut conn = self.db_conn();
        let mut migrations =
            diesel::migration::MigrationSource::<diesel::sqlite::Sqlite>::migrations(
                &bencher_schema::MIGRATIONS,
            )
            .expect("Failed to read the migrations");
        migrations.sort_by_cached_key(|migration| migration.name().version().to_string());
        // Off as in `run_migrations`, so a down migration that rebuilds a table does not cascade.
        conn.batch_execute("PRAGMA foreign_keys = OFF")
            .expect("Failed to turn foreign keys off");
        for migration in migrations.iter().rev() {
            let reverted = migration.name().version().to_string();
            conn.transaction::<_, Box<dyn std::error::Error + Send + Sync>, _>(|conn| {
                migration.revert(conn)?;
                diesel::RunQueryDsl::execute(
                    diesel::sql_query("DELETE FROM __diesel_schema_migrations WHERE version = ?")
                        .bind::<diesel::sql_types::Text, _>(&reverted),
                    conn,
                )?;
                Ok(())
            })
            .expect("Failed to revert a migration");
            if reverted == version {
                break;
            }
        }
        conn.batch_execute("PRAGMA foreign_keys = ON")
            .expect("Failed to turn foreign keys on");
    }

    /// Make every organization claimed in the eyes of the API's writer connection alone.
    /// A claim counts the organization's roles on the writer, after a check that read them on
    /// another connection, so this stands in for someone else's claim landing between the two.
    /// A role the writer still writes lands in the real table, so a caller asserts the claim's own
    /// refusal and, with [`Self::project_organization_roles`], that no role landed.
    #[expect(clippy::expect_used, reason = "test helper changing the writer's view")]
    pub async fn claim_after_the_check(&self) {
        self.context()
            .database
            .connection
            .lock()
            .await
            .batch_execute(
                "CREATE TEMP VIEW organization_role AS
                SELECT 0 AS id, 0 AS user_id, id AS organization_id, 'leader' AS role,
                    0 AS created, 0 AS modified
                FROM main.organization;
                CREATE TEMP TRIGGER organization_role_insert
                INSTEAD OF INSERT ON organization_role
                BEGIN
                    INSERT INTO main.organization_role
                        (user_id, organization_id, role, created, modified)
                    VALUES
                        (NEW.user_id, NEW.organization_id, NEW.role, NEW.created, NEW.modified);
                END;",
            )
            .expect("Failed to claim every organization for the writer");
    }

    /// Take every role of `user`'s away in the eyes of the API's writer connection alone.
    /// A removal counts an organization's Leaders on the writer, after a check that read them on
    /// another connection, so this stands in for the user's own removal landing between the two.
    /// A role the writer still deletes goes from the real table, so a caller asserts the removal's
    /// refusal and that no role went.
    #[expect(clippy::expect_used, reason = "test helper changing the writer's view")]
    pub async fn remove_after_the_check(&self, user: &crate::TestUser) {
        let user_id =
            QueryUser::get_id(&mut self.db_conn(), user.uuid).expect("Failed to get user ID");
        self.context()
            .database
            .connection
            .lock()
            .await
            .batch_execute(&format!(
                "CREATE TEMP VIEW organization_role AS
                SELECT * FROM main.organization_role WHERE user_id != {user_id};
                CREATE TEMP TRIGGER organization_role_delete
                INSTEAD OF DELETE ON organization_role
                BEGIN
                    DELETE FROM main.organization_role WHERE id = OLD.id;
                END;"
            ))
            .expect("Failed to remove the user for the writer");
    }

    /// Spend the whole budget of wrong codes for `email`, now, in the eyes of the API's writer
    /// connection alone. A proof checks the budget on the writer, inside its transaction, so this
    /// stands in for wrong codes that landed after any check another connection made. It also
    /// turns a wrong code's write of the budget on the writer into an error.
    #[expect(clippy::expect_used, reason = "test helper changing the writer's view")]
    pub async fn spend_code_budget_on_the_writer(&self, email: &str) {
        let email: Email = email.parse().expect("Invalid email");
        let email_hash = EmailHash::from(&email);
        let now = self.context().clock.now().timestamp();
        self.context()
            .database
            .connection
            .lock()
            .await
            .batch_execute(&format!(
                "CREATE TEMP VIEW wrong_code AS
                WITH RECURSIVE spent(id) AS (
                    SELECT 1 UNION ALL SELECT id + 1 FROM spent WHERE id < {CODE_BUDGET}
                )
                SELECT id, '{}' AS email_hash, {now} AS created FROM spent",
                email_hash.as_ref()
            ))
            .expect("Failed to spend the budget for the writer");
    }

    /// Keep `session` as it is now in the eyes of the API's reader connections, even once its row
    /// is gone, beside any row that then takes its ID. A request reads its session as it signs in,
    /// so this stands in for the session ending, and another session taking its row ID, after a
    /// request signed in with it.
    #[expect(clippy::expect_used, reason = "test helper changing the readers' view")]
    pub fn keep_session_for_the_readers(&self, session: &SessionToken) {
        let mut conn = self.db_conn();
        let id: i32 = schema::session::table
            .filter(schema::session::token_hash.eq(SessionTokenHash::from(session)))
            .select(schema::session::id)
            .first(&mut conn)
            .expect("Failed to find the session");
        conn.batch_execute(&format!(
            "CREATE TABLE session_as_read AS SELECT * FROM session WHERE id = {id}"
        ))
        .expect("Failed to keep the session");
        let database = &self.context().database;
        let readers = [
            &database.public_pool,
            &database.public_pool,
            &database.auth_pool,
            &database.auth_pool,
        ]
        .map(|pool| pool.get().expect("Failed to get a reader connection"));
        for mut reader in readers {
            reader
                .batch_execute(
                    "CREATE TEMP VIEW session AS
                    SELECT * FROM main.session UNION ALL SELECT * FROM main.session_as_read;",
                )
                .expect("Failed to keep the session for a reader");
        }
    }

    /// How many roles the organization of the project `slug` holds, as the database has them.
    #[expect(clippy::expect_used, reason = "test helper reading roles")]
    pub fn project_organization_roles(&self, slug: &str) -> i64 {
        let mut conn = self.db_conn();
        let organization_id: OrganizationId = schema::project::table
            .filter(schema::project::slug.eq(slug))
            .select(schema::project::organization_id)
            .first(&mut conn)
            .expect("Failed to find the project");
        schema::organization_role::table
            .filter(schema::organization_role::organization_id.eq(organization_id))
            .count()
            .get_result(&mut conn)
            .expect("Failed to count the roles")
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

    /// Answer the next callback the server sends with `status`, and those after it with 200.
    #[cfg(feature = "plus")]
    pub fn answer_next_callback(&self, status: http::StatusCode) {
        self.callback_sender.answer_next(status);
    }

    /// Wait for every callback delivery to settle; the server claims no callback afterwards.
    #[cfg(feature = "plus")]
    pub async fn drain_callbacks(&self) {
        let log = slog::Logger::root(slog::Discard, slog::o!());
        self.context().callbacks.drain(&log).await;
    }

    /// Shut down the server gracefully
    pub async fn close(self) {
        Self::shut_down(self.server).await;
    }

    async fn shut_down(server: HttpServer<ApiContext>) {
        #[cfg(feature = "plus")]
        let callbacks = server.app_private().callbacks.clone();
        #[cfg(feature = "plus")]
        server.app_private().shutdown.cancel();
        #[cfg(feature = "plus")]
        let log = slog::Logger::root(slog::Discard, slog::o!());
        #[cfg(feature = "plus")]
        server.app_private().commit_titles.drain(&log).await;
        drop(server.close().await);
        #[cfg(feature = "plus")]
        callbacks.drain(&log).await;
    }
}

/// The tasks a test server runs, each with the server's GitHub App if it has one: callbacks to
/// the recording sender, and the commit title task.
#[cfg(feature = "plus")]
fn background_tasks(
    log: &slog::Logger,
    database: &Database,
    callback_key: &bencher_callback::CallbackKey,
    callback_sender: &Arc<crate::RecordingSender>,
    shutdown: &bencher_schema::context::CancellationToken,
    clock: &bencher_json::Clock,
    github_app: Option<&Arc<bencher_github_client::GitHubApp>>,
) -> (
    bencher_schema::context::Callbacks,
    bencher_schema::context::CommitTitles,
) {
    let callbacks = bencher_schema::context::Callbacks::new_with_github_app(
        Arc::clone(&database.connection),
        database.auth_pool.clone(),
        callback_key.clone(),
        callback_sender.clone(),
        shutdown.clone(),
        clock.clone(),
        github_app.cloned(),
    );
    let commit_titles = bencher_schema::context::CommitTitles::spawn(
        log.clone(),
        Arc::clone(&database.connection),
        database.auth_pool.clone(),
        shutdown.clone(),
        clock.clone(),
        github_app.cloned(),
    );
    (callbacks, commit_titles)
}

/// The settings of a test server, each left at the plain test server's default when unset.
#[cfg(feature = "plus")]
#[derive(Default)]
pub struct TestServerOptions {
    pub upload_timeout: Option<u64>,
    pub max_body_size: Option<u64>,
    pub clock: Option<bencher_json::Clock>,
    pub runner_update_base_url: Option<url::Url>,
    pub rate_limiting: Option<bencher_schema::context::RateLimiting>,
    pub log: Option<slog::Logger>,
    pub cloud_routes: bool,
    pub openai_apps_challenge: Option<bencher_json::NonEmpty>,
    pub oauth: Option<bencher_schema::context::OAuthServer>,
    pub github_client: Option<bencher_github_client::GitHubClient>,
    pub github_app: Option<Arc<bencher_github_client::GitHubApp>>,
    pub google_client: Option<bencher_google_client::GoogleClient>,
    pub relay_secret: Option<bencher_json::Secret>,
    pub turnstile: Option<bencher_schema::context::Turnstile>,
    pub messenger: Option<Messenger>,
    /// Whether every route's `OPTIONS` preflight is registered, as the API server registers it.
    pub http_options: bool,
}

/// The routes a test server registers beyond the plain set.
#[cfg(feature = "plus")]
#[derive(Clone, Copy)]
struct Routes {
    /// The Bencher Cloud only endpoints.
    cloud: bool,
    /// Every route's `OPTIONS` preflight.
    http_options: bool,
}

/// A migrated database in a temporary file, with its path and the file that keeps it.
#[expect(clippy::expect_used, reason = "test server setup with fallible init")]
fn temp_database() -> (Database, String, NamedTempFile) {
    let db_file = NamedTempFile::new().expect("Failed to create temp db file");
    let db_path = db_file.path().to_str().expect("Invalid db path").to_owned();
    (database_at(&db_path), db_path, db_file)
}

/// The database in the file at `db_path`, migrated, on new connections.
#[expect(clippy::expect_used, reason = "test server setup with fallible init")]
fn database_at(db_path: &str) -> Database {
    let mut conn =
        DbConnection::establish(db_path).expect("Failed to establish database connection");
    set_busy_timeout(&mut conn).expect("Failed to set the busy timeout");
    // With the server's key, as the API's startup runs them.
    let token_key = TokenKey::new(ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
    run_migrations_with_token_key(&mut conn, Some(&token_key)).expect("Failed to run migrations");

    Database {
        path: PathBuf::from(db_path),
        busy_timeout: BUSY_TIMEOUT_MS,
        public_pool: connection_pool(db_path),
        auth_pool: connection_pool(db_path),
        connection: Arc::new(Mutex::new(conn)),
    }
}

#[expect(clippy::expect_used, reason = "test server setup with fallible init")]
fn code_key() -> CodeKey {
    CodeKey::new(&DEFAULT_SECRET_KEY).expect("Failed to derive the sign-in code key")
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
