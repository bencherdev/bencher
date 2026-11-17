use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use bencher_github_client::{
    GitHubApiError, GitHubApp, GitHubAppError, GitHubCommitsError, GitHubReadToken,
};
use bencher_json::{
    Clock, CommitTitle, DateTime, GitHash, GitHubInstallationId, GitHubRepositoryId,
};
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use http::{HeaderMap, StatusCode};
use slog::Logger;
use tokio::{
    sync::mpsc::{self, error::TrySendError},
    task::JoinHandle,
    time::Instant,
};
use tokio_util::sync::CancellationToken;

#[cfg(feature = "otel")]
use bencher_otel::{ApiCounter, ApiMeter, CommitTitlesFailureReason as Failure};

use super::DbConnection;
use crate::model::project::{
    ProjectId,
    commit::{self, InsertCommit},
    linked_repository::QueryLinkedRepository,
};

/// The most work the task holds; more is dropped, and the next sweep finds it.
const QUEUE: usize = 1024;
/// The most commits one GraphQL request asks for.
const BATCH: usize = 100;
const SWEEP_PERIOD: Duration = Duration::from_hours(24);
/// How far back a sweep or a fill looks, by report start.
const SWEEP_WINDOW: i64 = 90 * 24 * 60 * 60;
/// A cached token is used until this long before it expires.
const TOKEN_MARGIN: i64 = 5 * 60;
/// The pause of a rate limit that names no time to try again.
const FALLBACK_PAUSE: i64 = 60;

/// Owns the task that fetches the title of each commit from GitHub, one time, and stores it.
/// A server without the GitHub App runs no task and fetches no title.
pub struct CommitTitles {
    queue: Option<Queue>,
    task: Mutex<Option<JoinHandle<()>>>,
}

struct Queue {
    sender: mpsc::Sender<Work>,
    log: Logger,
}

/// What the task is asked to fetch.
#[derive(Debug)]
enum Work {
    /// The hashes of reports that just committed.
    Hashes(ProjectId, Vec<GitHash>),
    /// Every recent hash of a project that a link just connected.
    Fill(ProjectId),
}

impl CommitTitles {
    /// No task, for a server without the GitHub App.
    pub fn none() -> Self {
        Self {
            queue: None,
            task: Mutex::new(None),
        }
    }

    /// Start the task, which sweeps at once and then every 24 hours, until `shutdown`.
    pub fn spawn(
        log: Logger,
        connection: Arc<tokio::sync::Mutex<DbConnection>>,
        reader: Pool<ConnectionManager<DbConnection>>,
        shutdown: CancellationToken,
        clock: Clock,
        github_app: Option<Arc<GitHubApp>>,
    ) -> Self {
        let Some(app) = github_app else {
            return Self::none();
        };
        let (sender, receiver) = mpsc::channel(QUEUE);
        let titles = Titles::new(connection, reader, app, clock);
        let task = tokio::spawn(run(titles, receiver, shutdown, log.clone()));
        Self {
            queue: Some(Queue { sender, log }),
            task: Mutex::new(Some(task)),
        }
    }

    /// Whether the task runs, so a report's hash is worth checking for it.
    pub fn is_running(&self) -> bool {
        self.queue.is_some()
    }

    /// Give the task the hash of a report that just committed, without waiting on it.
    pub fn ingest(&self, project_id: ProjectId, hash: GitHash) {
        self.send(Work::Hashes(project_id, vec![hash]));
    }

    /// Fill a project whose link just connected, without waiting on it.
    pub fn fill(&self, project_id: ProjectId) {
        self.send(Work::Fill(project_id));
    }

    fn send(&self, work: Work) {
        let Some(Queue { sender, log }) = &self.queue else {
            return;
        };
        match sender.try_send(work) {
            Ok(()) => {},
            Err(TrySendError::Full(work)) => {
                slog::warn!(log, "Commit title queue is full, the next sweep finds the work"; "work" => ?work);
                #[cfg(feature = "otel")]
                ApiMeter::increment(ApiCounter::GitHubCommitTitlesFailed(Failure::Dropped));
            },
            Err(TrySendError::Closed(work)) => {
                slog::debug!(log, "Commit title task has stopped"; "work" => ?work);
            },
        }
    }

    /// Wait for the task, which the shutdown signal stops.
    pub async fn drain(&self, log: &Logger) {
        let task = self
            .task
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(task) = task
            && let Err(e) = task.await
        {
            slog::error!(log, "Commit title task failed: {e}");
        }
    }
}

/// What the task's loop drives, apart from the loop so its schedule can be tested alone.
trait Fetch {
    /// Fill every project with a connected link.
    fn sweep(&mut self, log: &Logger) -> impl Future<Output = ()> + Send;

    /// Fetch the work the queue held.
    fn work(&mut self, log: &Logger, batch: Vec<Work>) -> impl Future<Output = ()> + Send;
}

/// Sweep at once and then every 24 hours, and fetch the work the queue holds in between, with
/// each project's hashes and fills gathered, in arrival order, until `shutdown`.
async fn run<F: Fetch>(
    mut titles: F,
    mut receiver: mpsc::Receiver<Work>,
    shutdown: CancellationToken,
    log: Logger,
) {
    let mut next_sweep = Instant::now();
    loop {
        // Checked first, so a queue that never empties still sweeps on time.
        let done = if Instant::now() >= next_sweep {
            next_sweep = Instant::now() + SWEEP_PERIOD;
            shutdown.run_until_cancelled(titles.sweep(&log)).await
        } else {
            let next = shutdown
                .run_until_cancelled(tokio::time::timeout_at(next_sweep, receiver.recv()))
                .await;
            match next {
                None | Some(Ok(None)) => return,
                Some(Err(_elapsed)) => Some(()),
                Some(Ok(Some(work))) => {
                    let mut batch = vec![work];
                    while let Ok(work) = receiver.try_recv() {
                        batch.push(work);
                    }
                    shutdown.run_until_cancelled(titles.work(&log, batch)).await
                },
            }
        };
        if done.is_none() {
            return;
        }
    }
}

/// What the task keeps between fetches: each repository's read token until shortly before it
/// expires, and each rate limited installation's reset.
struct Titles {
    connection: Arc<tokio::sync::Mutex<DbConnection>>,
    reader: Pool<ConnectionManager<DbConnection>>,
    app: Arc<GitHubApp>,
    clock: Clock,
    tokens: HashMap<(GitHubInstallationId, GitHubRepositoryId), GitHubReadToken>,
    limited: HashMap<GitHubInstallationId, DateTime>,
}

impl Fetch for Titles {
    /// Each project's queued work once, in the order it first arrived: its fill, then the
    /// hashes that the fill left without a row.
    async fn work(&mut self, log: &Logger, batch: Vec<Work>) {
        let mut projects: Vec<(ProjectId, bool, Vec<GitHash>)> = Vec::new();
        for work in batch {
            let (project_id, fill, hashes) = match work {
                Work::Hashes(project_id, hashes) => (project_id, false, hashes),
                Work::Fill(project_id) => (project_id, true, Vec::new()),
            };
            if let Some((_, queued_fill, queued)) = projects
                .iter_mut()
                .find(|(queued, ..)| *queued == project_id)
            {
                *queued_fill |= fill;
                queued.extend(hashes);
            } else {
                projects.push((project_id, fill, hashes));
            }
        }
        for (project_id, fill, hashes) in projects {
            if fill {
                self.fill(log, project_id).await;
            }
            self.ingest(log, project_id, &hashes).await;
        }
    }

    /// Fill every project with a connected link, one at a time.
    async fn sweep(&mut self, log: &Logger) {
        let projects = QueryLinkedRepository::connected_projects(&mut *self.read().await);
        let projects = match projects {
            Ok(projects) => projects,
            Err(e) => {
                slog::error!(log, "Failed to read the connected links for commit titles"; "error" => %e);
                return;
            },
        };
        for project_id in projects {
            self.fill(log, project_id).await;
        }
    }
}

/// Why a fetch for a project stopped short, storing nothing more.
enum Stop {
    /// Its installation is rate limited, so nothing more goes to it until the reset.
    Limited,
    Failed,
}

impl Titles {
    fn new(
        connection: Arc<tokio::sync::Mutex<DbConnection>>,
        reader: Pool<ConnectionManager<DbConnection>>,
        app: Arc<GitHubApp>,
        clock: Clock,
    ) -> Self {
        Self {
            connection,
            reader,
            app,
            clock,
            tokens: HashMap::new(),
            limited: HashMap::new(),
        }
    }

    /// Fetch every hash of the project's reports from the last 90 days with no row, newest first.
    async fn fill(&mut self, log: &Logger, project_id: ProjectId) {
        let since = DateTime::try_from(self.clock.timestamp() - SWEEP_WINDOW).unwrap_or_default();
        let hashes = commit::recent_unstored(&mut *self.read().await, project_id, since);
        match hashes {
            Ok(hashes) => self.fetch(log, project_id, hashes).await,
            Err(e) => {
                slog::error!(log, "Failed to read a project's commits without a title"; "project_id" => ?project_id, "error" => %e);
            },
        }
    }

    /// Fetch each of the hashes with no row.
    async fn ingest(&mut self, log: &Logger, project_id: ProjectId, hashes: &[GitHash]) {
        if hashes.is_empty() {
            return;
        }
        let hashes = commit::unstored(&mut *self.read().await, project_id, hashes);
        match hashes {
            Ok(hashes) => self.fetch(log, project_id, hashes).await,
            Err(e) => {
                slog::error!(log, "Failed to read a project's commits"; "project_id" => ?project_id, "error" => %e);
            },
        }
    }

    /// Fetch the hashes, in lower case and with no row, from the project's link as it stands
    /// now, 100 to a request, and store what GitHub answers. A failure stores nothing more.
    async fn fetch(&mut self, log: &Logger, project_id: ProjectId, hashes: Vec<GitHash>) {
        if hashes.is_empty() {
            return;
        }
        let link = QueryLinkedRepository::get_connected(&mut *self.read().await, project_id);
        let link = match link {
            Ok(Some(link)) => link,
            Ok(None) => return,
            Err(e) => {
                slog::error!(log, "Failed to read a linked repository for commit titles"; "project_id" => ?project_id, "error" => %e);
                return;
            },
        };
        for batch in hashes.chunks(BATCH) {
            if self.fetch_batch(log, &link, batch).await.is_err() {
                return;
            }
        }
    }

    async fn fetch_batch(
        &mut self,
        log: &Logger,
        link: &QueryLinkedRepository,
        hashes: &[GitHash],
    ) -> Result<(), Stop> {
        let installation = link.installation_id;
        let now = self.clock.now();
        if self
            .limited
            .get(&installation)
            .is_some_and(|until| now.timestamp() < until.timestamp())
        {
            return Err(Stop::Limited);
        }
        let key = (installation, link.repository_id);
        if self
            .tokens
            .get(&key)
            .is_none_or(|token| now.timestamp() >= token.expires_at.timestamp() - TOKEN_MARGIN)
        {
            self.tokens.remove(&key);
            let minted = self
                .app
                .mint_read(now, installation, link.repository_id)
                .await;
            match minted {
                Ok(token) => {
                    self.tokens.insert(key, token);
                },
                Err(e) => {
                    return Err(self.stop(log, installation, now, MintOrFetch::Mint(e)));
                },
            }
        }
        let Some(GitHubReadToken {
            token, full_name, ..
        }) = self.tokens.get(&key)
        else {
            return Err(Stop::Failed);
        };
        let answered = self.app.commit_messages(token, full_name, hashes).await;
        let messages = match answered {
            Ok(messages) => messages,
            Err(e) => {
                if let GitHubCommitsError::Api(GitHubApiError::Status(
                    StatusCode::UNAUTHORIZED,
                    _,
                )) = e
                {
                    self.tokens.remove(&key);
                }
                return Err(self.stop(log, installation, now, MintOrFetch::Fetch(e)));
            },
        };
        let created = self.clock.now();
        let commits = messages
            .into_iter()
            .map(|(hash, message)| InsertCommit {
                project_id: link.project_id,
                hash: hash.to_ascii_lowercase(),
                title: message.as_deref().and_then(CommitTitle::from_message),
                created,
            })
            .collect::<Vec<_>>();
        let stored = self
            .connection
            .lock()
            .await
            .immediate_transaction(|conn| InsertCommit::insert_new(conn, &commits));
        if let Err(e) = stored {
            slog::error!(log, "Failed to store commit titles"; "project_id" => ?link.project_id, "error" => %e);
            #[cfg(feature = "otel")]
            ApiMeter::increment(ApiCounter::GitHubCommitTitlesFailed(Failure::Store));
            return Err(Stop::Failed);
        }
        Ok(())
    }

    /// Log a failure, and hold back the installation until the reset of a rate limit.
    fn stop(
        &mut self,
        log: &Logger,
        installation: GitHubInstallationId,
        now: DateTime,
        failure: MintOrFetch,
    ) -> Stop {
        if let Some(until) = failure.rate_limited_until(now) {
            slog::warn!(log, "GitHub rate limited the commit titles of an installation"; "installation" => %installation, "until" => %until);
            #[cfg(feature = "otel")]
            ApiMeter::increment(ApiCounter::GitHubCommitTitlesFailed(Failure::RateLimited));
            self.limited.insert(installation, until);
            return Stop::Limited;
        }
        match failure {
            MintOrFetch::Mint(e) => {
                slog::warn!(log, "Failed to mint a token for commit titles"; "installation" => %installation, "error" => %e);
                #[cfg(feature = "otel")]
                ApiMeter::increment(ApiCounter::GitHubCommitTitlesFailed(Failure::Mint));
            },
            MintOrFetch::Fetch(e) => {
                slog::warn!(log, "Failed to fetch commit titles"; "installation" => %installation, "error" => %e);
                #[cfg(feature = "otel")]
                ApiMeter::increment(ApiCounter::GitHubCommitTitlesFailed(Failure::Fetch));
            },
        }
        Stop::Failed
    }

    /// A read connection, however long the pool stays busy; shutdown cuts the wait short.
    async fn read(&self) -> PooledConnection<ConnectionManager<DbConnection>> {
        loop {
            if let Some(conn) = self.reader.try_get() {
                return conn;
            }
            tokio::time::sleep(self.reader.connection_timeout()).await;
        }
    }
}

enum MintOrFetch {
    Mint(GitHubAppError),
    Fetch(GitHubCommitsError),
}

impl MintOrFetch {
    /// When a rate limited installation may ask again: a 429, a 403 with `retry-after`, a spent
    /// limit, or a secondary rate limit, or a GraphQL answer that says so, until `retry-after`,
    /// else the reset GitHub gives for a spent limit, else a minute on. GitHub sends its limit's
    /// reset on every answer, so a reset with requests left says nothing about this one.
    fn rate_limited_until(&self, now: DateTime) -> Option<DateTime> {
        let headers = match self {
            Self::Mint(GitHubAppError::Mint(GitHubApiError::SecondaryRateLimit(headers)))
            | Self::Fetch(
                GitHubCommitsError::Api(GitHubApiError::SecondaryRateLimit(headers))
                | GitHubCommitsError::RateLimited(headers),
            ) => headers,
            Self::Mint(GitHubAppError::Mint(GitHubApiError::Status(status, headers)))
            | Self::Fetch(GitHubCommitsError::Api(GitHubApiError::Status(status, headers))) => {
                let limited = *status == StatusCode::TOO_MANY_REQUESTS
                    || (*status == StatusCode::FORBIDDEN
                        && (number(headers, "retry-after").is_some()
                            || number(headers, "x-ratelimit-remaining") == Some(0)));
                limited.then_some(headers)?
            },
            Self::Mint(_) | Self::Fetch(_) => return None,
        };
        let now = now.timestamp();
        let until = if let Some(retry_after) = number(headers, "retry-after") {
            now.saturating_add(retry_after)
        } else if number(headers, "x-ratelimit-remaining") == Some(0)
            && let Some(reset) = number(headers, "x-ratelimit-reset")
        {
            reset
        } else {
            now.saturating_add(FALLBACK_PAUSE)
        };
        DateTime::try_from(until).ok()
    }
}

fn number(headers: &HeaderMap, name: &str) -> Option<i64> {
    headers.get(name)?.to_str().ok()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use std::{
        fmt::{self, Write as _},
        sync::{
            Arc, Mutex,
            atomic::{AtomicI64, Ordering},
        },
        time::Duration,
    };

    use bencher_github_client::{
        GitHubApp,
        test_server::{Answer, Received, Server, app_key, install_crypto_provider},
    };
    use bencher_json::{
        Clock, DateTime, GitHash, GitHubInstallationId, GitHubRepositoryId,
        project::github::LinkedRepositoryReason,
    };
    use diesel::{
        Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
        connection::SimpleConnection as _,
        r2d2::{ConnectionManager, Pool},
    };
    use pretty_assertions::assert_eq;
    use slog::{Drain, KV as _, Logger, OwnedKVList, Record};
    use tokio::{sync::mpsc, time::Instant};
    use tokio_util::sync::CancellationToken;

    use super::{Fetch as _, QUEUE, Titles, Work, run};
    use crate::{
        context::DbConnection,
        model::{
            project::{
                ProjectId, branch::head::HeadId, linked_repository::QueryLinkedRepository,
                testbed::TestbedId,
            },
            user::UserId,
        },
        run_migrations, schema,
        test_util::{create_base_entities, create_branch_with_head, create_testbed},
    };

    const INSTALLATION: u64 = 2048;
    const REPOSITORY: u64 = 4096;
    const TOKEN: &str = "read-token-marker";
    const DAY: i64 = 24 * 60 * 60;
    const MINT: &str = "POST /github/app/installations/2048/access_tokens HTTP/1.1";
    const GRAPHQL: &str = "POST /github/graphql HTTP/1.1";
    /// A hash with letters, so its upper case differs.
    const LETTERS: u64 = 0xabc_def;

    /// Keeps the message of every log line, with its key-value pairs.
    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<String>>>);

    struct Line(String);

    impl Drain for Capture {
        type Ok = ();
        type Err = slog::Never;

        fn log(&self, record: &Record<'_>, values: &OwnedKVList) -> Result<(), slog::Never> {
            self.keep(record, values);
            Ok(())
        }
    }

    impl Capture {
        fn keep(&self, record: &Record<'_>, values: &OwnedKVList) {
            let mut line = Line(record.msg().to_string());
            record.kv().serialize(record, &mut line).unwrap();
            values.serialize(record, &mut line).unwrap();
            self.0.lock().unwrap().push(line.0);
        }
    }

    impl slog::Serializer for Line {
        fn emit_arguments(&mut self, key: slog::Key, val: &fmt::Arguments<'_>) -> slog::Result {
            write!(self.0, " {key}={val}").map_err(slog::Error::Fmt)
        }
    }

    struct Harness {
        /// Holds the database, so removing it removes the WAL files too, whatever closes last.
        _dir: tempfile::TempDir,
        connection: Arc<tokio::sync::Mutex<DbConnection>>,
        reader: Pool<ConnectionManager<DbConnection>>,
        /// What the clock reads, in Unix seconds.
        now: Arc<AtomicI64>,
        capture: Capture,
        log: Logger,
        user_id: UserId,
        project_id: ProjectId,
        head_id: HeadId,
        testbed_id: TestbedId,
        reports: usize,
    }

    impl Harness {
        fn new() -> Self {
            // A file, so the task's read pool sees what the writer commits.
            let dir = tempfile::TempDir::new().unwrap();
            let path = dir.path().join("titles.db").to_str().unwrap().to_owned();
            let mut conn = DbConnection::establish(&path).unwrap();
            conn.batch_execute("PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;")
                .unwrap();
            conn.batch_execute("PRAGMA journal_mode = WAL").unwrap();
            run_migrations(&mut conn).unwrap();
            let base = create_base_entities(&mut conn);
            let branch = create_branch_with_head(
                &mut conn,
                base.project_id,
                "00000000-0000-0000-0000-000000000010",
                "main",
                "main",
                "00000000-0000-0000-0000-000000000011",
            );
            let testbed_id = create_testbed(
                &mut conn,
                base.project_id,
                "00000000-0000-0000-0000-000000000020",
                "localhost",
                "localhost",
            );
            diesel::insert_into(schema::user::table)
                .values((
                    schema::user::uuid.eq("00000000-0000-0000-0000-000000000030"),
                    schema::user::name.eq("Test User"),
                    schema::user::slug.eq("test-user"),
                    schema::user::email.eq("test@example.com"),
                    schema::user::admin.eq(false),
                    schema::user::locked.eq(false),
                    schema::user::created.eq(DateTime::TEST),
                    schema::user::modified.eq(DateTime::TEST),
                ))
                .execute(&mut conn)
                .unwrap();
            let user_id = schema::user::table
                .select(schema::user::id)
                .first(&mut conn)
                .unwrap();
            let reader = Pool::builder()
                .max_size(2)
                .build(ConnectionManager::<DbConnection>::new(&path))
                .unwrap();
            let capture = Capture::default();
            let log = Logger::root(capture.clone(), slog::o!());
            Self {
                _dir: dir,
                connection: Arc::new(tokio::sync::Mutex::new(conn)),
                reader,
                now: Arc::new(AtomicI64::new(DateTime::TEST.timestamp())),
                capture,
                log,
                user_id,
                project_id: base.project_id,
                head_id: branch.head_id,
                testbed_id,
                reports: 0,
            }
        }

        /// The task's state, with an app whose every request goes to `server`.
        fn titles(&self, server: &Server) -> Titles {
            install_crypto_provider();
            let (private_key, _public_key) = app_key();
            let app = GitHubApp::new(
                "Iv1.0123456789abcdef".parse().unwrap(),
                "bencher-test".parse().unwrap(),
                &private_key,
                "webhook-secret-marker".parse().unwrap(),
            )
            .unwrap()
            .with_base_url(server.base())
            .unwrap();
            let now = Arc::clone(&self.now);
            Titles::new(
                Arc::clone(&self.connection),
                self.reader.clone(),
                Arc::new(app),
                Clock::Custom(Arc::new(move || {
                    DateTime::try_from(now.load(Ordering::SeqCst)).unwrap()
                })),
            )
        }

        fn at(&self, timestamp: i64) {
            self.now.store(timestamp, Ordering::SeqCst);
        }

        fn later(&self, seconds: i64) {
            self.now.fetch_add(seconds, Ordering::SeqCst);
        }

        async fn link(&self, project_id: ProjectId) -> QueryLinkedRepository {
            let link = QueryLinkedRepository::connected(
                project_id,
                GitHubInstallationId::try_from(INSTALLATION).unwrap(),
                GitHubRepositoryId::try_from(REPOSITORY).unwrap(),
                "octo-cat/stale-name".parse().unwrap(),
                self.user_id,
                DateTime::TEST,
            );
            link.replace(&mut *self.connection.lock().await).unwrap();
            link
        }

        /// A report of the project with `hash` that started `age` seconds before now, on a
        /// version of its own.
        async fn report(&mut self, project_id: ProjectId, hash: &str, age: i64) {
            self.reports += 1;
            let n = self.reports;
            let start_time = DateTime::try_from(self.now.load(Ordering::SeqCst) - age).unwrap();
            let conn = &mut *self.connection.lock().await;
            diesel::insert_into(schema::version::table)
                .values((
                    schema::version::uuid.eq(format!("10000000-0000-0000-0000-{n:012}")),
                    schema::version::project_id.eq(project_id),
                    schema::version::number.eq(i32::try_from(n).unwrap()),
                    schema::version::hash.eq(hash),
                ))
                .execute(conn)
                .unwrap();
            let version_id: i32 = schema::version::table
                .select(schema::version::id)
                .order(schema::version::id.desc())
                .first(conn)
                .unwrap();
            diesel::insert_into(schema::head_version::table)
                .values((
                    schema::head_version::head_id.eq(self.head_id),
                    schema::head_version::version_id.eq(version_id),
                ))
                .execute(conn)
                .unwrap();
            diesel::insert_into(schema::report::table)
                .values((
                    schema::report::uuid.eq(format!("20000000-0000-0000-0000-{n:012}")),
                    schema::report::project_id.eq(project_id),
                    schema::report::head_id.eq(self.head_id),
                    schema::report::version_id.eq(version_id),
                    schema::report::testbed_id.eq(self.testbed_id),
                    schema::report::adapter.eq(0),
                    schema::report::start_time.eq(start_time),
                    schema::report::end_time.eq(start_time),
                    schema::report::created.eq(start_time),
                ))
                .execute(conn)
                .unwrap();
        }

        /// Every stored commit of the project, by hash.
        async fn rows(&self, project_id: ProjectId) -> Vec<(String, Option<String>)> {
            schema::commit::table
                .filter(schema::commit::project_id.eq(project_id))
                .order(schema::commit::hash)
                .select((schema::commit::hash, schema::commit::title))
                .load(&mut *self.connection.lock().await)
                .unwrap()
        }

        /// The log lines that say a request to GitHub failed.
        fn failures(&self) -> Vec<String> {
            self.capture
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|line| line.starts_with("Failed to"))
                .cloned()
                .collect()
        }

        /// Another project of the same organization.
        async fn project(&self, slug: &str) -> ProjectId {
            let conn = &mut *self.connection.lock().await;
            let organization_id: i32 = schema::organization::table
                .select(schema::organization::id)
                .first(conn)
                .unwrap();
            diesel::insert_into(schema::project::table)
                .values((
                    schema::project::uuid
                        .eq(format!("30000000-0000-0000-0000-{:0>12}", slug.len())),
                    schema::project::organization_id.eq(organization_id),
                    schema::project::name.eq(slug),
                    schema::project::slug.eq(slug),
                    schema::project::visibility.eq(0),
                    schema::project::created.eq(DateTime::TEST),
                    schema::project::modified.eq(DateTime::TEST),
                ))
                .execute(conn)
                .unwrap();
            schema::project::table
                .filter(schema::project::slug.eq(slug))
                .select(schema::project::id)
                .first(conn)
                .unwrap()
        }
    }

    fn hash(n: u64) -> String {
        format!("{n:0>40x}")
    }

    fn git_hash(n: u64) -> GitHash {
        hash(n).parse().unwrap()
    }

    /// A read token for the repository, under its current name, that expires at `expires`.
    fn minted(expires: i64) -> Answer {
        let expires = DateTime::try_from(expires).unwrap();
        (
            201,
            Vec::new(),
            format!(
                r#"{{"token": "{TOKEN}", "expires_at": "{}", "permissions": {{"contents": "read", "metadata": "read"}}, "repository_selection": "selected", "repositories": [{{"id": {REPOSITORY}, "name": "renamed", "full_name": "octo-cat/renamed", "owner": {{"login": "octo-cat", "id": 1}}}}]}}"#,
                serde_json::to_value(expires).unwrap().as_str().unwrap()
            ),
        )
    }

    /// A token that outlives every test.
    fn minted_long() -> Answer {
        minted(DateTime::TEST.timestamp() + 365 * DAY)
    }

    /// The answer for each alias in turn: a message, or none for a hash with no commit.
    fn commits(messages: &[Option<&str>]) -> Answer {
        let objects = messages
            .iter()
            .enumerate()
            .map(|(index, message)| match message {
                Some(message) => format!(
                    r#""c{index}": {{"message": {}}}"#,
                    serde_json::to_string(message).unwrap()
                ),
                None => format!(r#""c{index}": null"#),
            })
            .collect::<Vec<_>>()
            .join(", ");
        (
            200,
            Vec::new(),
            format!(r#"{{"data": {{"repository": {{{objects}}}}}}}"#),
        )
    }

    const SECONDARY: &str =
        "You have exceeded a secondary rate limit. Please wait a few minutes before you try again.";

    /// The rate-limit headers GitHub sends on every answer: `remaining` requests left, until a
    /// reset `seconds` after the test's start.
    fn left(remaining: u32, seconds: i64) -> Vec<(&'static str, String)> {
        vec![
            ("x-ratelimit-remaining", remaining.to_string()),
            (
                "x-ratelimit-reset",
                (DateTime::TEST.timestamp() + seconds).to_string(),
            ),
        ]
    }

    /// A 403 that a secondary rate limit stopped.
    fn secondary(headers: Vec<(&'static str, String)>) -> Answer {
        (403, headers, format!(r#"{{"message": "{SECONDARY}"}}"#))
    }

    fn status(status: u16, headers: Vec<(&'static str, String)>) -> Answer {
        (status, headers, r#"{"message": "No"}"#.to_owned())
    }

    fn request_lines(received: &[Received]) -> Vec<&str> {
        received.iter().map(Received::request_line).collect()
    }

    /// The hashes a GraphQL request asked for, in alias order.
    fn asked(request: &Received) -> Vec<String> {
        let query = request.json()["query"].as_str().unwrap().to_owned();
        query
            .split("object(oid: \"")
            .skip(1)
            .map(|rest| rest.split('"').next().unwrap().to_owned())
            .collect()
    }

    // Kills storing a missing commit's row with a title or none at all, storing a hash in its
    // upper case, asking the repository under the link's stale name instead of the minted one,
    // and fetching a stored hash, found or not, again.
    #[tokio::test]
    async fn a_commit_stores_its_title_or_none_and_is_never_fetched_again() {
        let harness = Harness::new();
        harness.link(harness.project_id).await;
        let server = Server::start_with_headers(vec![
            minted_long(),
            commits(&[Some("Fix the thing\n\nBody"), None]),
        ])
        .await;
        let mut titles = harness.titles(&server);
        let found = hash(LETTERS).to_uppercase().parse::<GitHash>().unwrap();

        titles
            .ingest(
                &harness.log,
                harness.project_id,
                &[found.clone(), git_hash(2)],
            )
            .await;

        assert_eq!(
            harness.rows(harness.project_id).await,
            [
                (hash(2), None),
                (hash(LETTERS), Some("Fix the thing".to_owned()))
            ]
        );
        let received = server.received().await;
        assert_eq!(request_lines(&received), [MINT, GRAPHQL]);
        let [_mint, graphql] = received.as_slice() else {
            panic!("Not two requests");
        };
        let graphql = graphql.json();
        assert_eq!(
            graphql["variables"],
            serde_json::json!({ "owner": "octo-cat", "name": "renamed" })
        );

        // Nothing answers now, so any request would fail and log.
        let silent = Server::start_with_headers(Vec::new()).await;
        let mut titles = harness.titles(&silent);
        titles
            .ingest(&harness.log, harness.project_id, &[found, git_hash(2)])
            .await;
        titles.fill(&harness.log, harness.project_id).await;
        assert!(silent.received().await.is_empty());
        assert_eq!(harness.failures(), Vec::<String>::new());
    }

    // Kills storing anything for an outage or a refused mint, and keeping a stop past the next
    // sweep, which would leave the hash without its title.
    #[tokio::test]
    async fn a_failure_stores_nothing_and_the_next_sweep_fetches_the_hash() {
        for (what, failure) in [
            ("a 5xx", vec![minted_long(), status(502, Vec::new())]),
            ("a refused mint", vec![status(404, Vec::new())]),
            ("an unauthorized mint", vec![status(401, Vec::new())]),
            (
                "an answer with errors",
                vec![
                    minted_long(),
                    (
                        200,
                        Vec::new(),
                        r#"{"data": {"repository": null}, "errors": [{"type": "NOT_FOUND"}]}"#
                            .to_owned(),
                    ),
                ],
            ),
            (
                "errors beside an answer",
                vec![
                    minted_long(),
                    (
                        200,
                        Vec::new(),
                        r#"{"data": {"repository": {"c0": null}}, "errors": [{"type": "SERVICE_UNAVAILABLE"}]}"#
                            .to_owned(),
                    ),
                ],
            ),
        ] {
            let mut harness = Harness::new();
            harness.link(harness.project_id).await;
            harness.report(harness.project_id, &hash(1), DAY).await;
            let mut script = failure;
            let token_cached = script.len() == 2;
            if !token_cached {
                script.push(minted_long());
            }
            script.push(commits(&[Some("Found")]));
            let server = Server::start_with_headers(script).await;
            let mut titles = harness.titles(&server);

            titles.sweep(&harness.log).await;
            assert_eq!(harness.rows(harness.project_id).await, [], "{what}");

            titles.sweep(&harness.log).await;
            assert_eq!(
                harness.rows(harness.project_id).await,
                [(hash(1), Some("Found".to_owned()))],
                "{what}"
            );
            server.received().await;
        }
    }

    /// Each form of a rate limit, and how many seconds it holds the installation.
    fn rate_limits() -> Vec<(&'static str, Vec<Answer>, i64)> {
        let reset = |seconds: i64| (DateTime::TEST.timestamp() + seconds).to_string();
        vec![
            (
                "a spent GraphQL limit",
                vec![
                    minted_long(),
                    (
                        200,
                        vec![
                            ("x-ratelimit-remaining", "0".to_owned()),
                            ("x-ratelimit-reset", reset(600)),
                        ],
                        r#"{"data": null, "errors": [{"type": "RATE_LIMITED", "message": "No"}]}"#
                            .to_owned(),
                    ),
                ],
                600,
            ),
            (
                "a 403 with a spent limit",
                vec![
                    minted_long(),
                    status(
                        403,
                        vec![
                            ("x-ratelimit-remaining", "0".to_owned()),
                            ("x-ratelimit-reset", reset(900)),
                        ],
                    ),
                ],
                900,
            ),
            (
                "a 429 with retry-after",
                vec![
                    minted_long(),
                    status(429, vec![("retry-after", "300".to_owned())]),
                ],
                300,
            ),
            (
                "a rate limited mint",
                vec![status(
                    403,
                    vec![
                        ("x-ratelimit-remaining", "0".to_owned()),
                        ("x-ratelimit-reset", reset(1200)),
                    ],
                )],
                1200,
            ),
            (
                "a secondary rate limit with no header",
                vec![minted_long(), secondary(Vec::new())],
                60,
            ),
            (
                "a secondary rate limit with requests left",
                vec![minted_long(), secondary(left(10, 3000))],
                60,
            ),
            (
                "a secondary rate limit with a spent limit",
                vec![minted_long(), secondary(left(0, 900))],
                900,
            ),
            (
                "a 429 with requests left",
                vec![minted_long(), status(429, left(10, 3000))],
                60,
            ),
            (
                "a GraphQL secondary rate limit with requests left",
                vec![
                    minted_long(),
                    (
                        200,
                        left(10, 3000),
                        format!(r#"{{"data": null, "errors": [{{"message": "{SECONDARY}"}}]}}"#),
                    ),
                ],
                60,
            ),
            (
                "a mint at a secondary rate limit",
                vec![secondary(left(10, 3000))],
                60,
            ),
        ]
    }

    // Kills telling a rate limit apart from any other failure, a mint's secondary limit included,
    // waiting for any other time than the one GitHub gives, waiting for a reset that comes with
    // requests left, holding back only the project rather than its installation, and storing
    // anything for a rate limited answer.
    #[tokio::test]
    async fn a_rate_limited_installation_sends_nothing_more_until_its_reset() {
        for (what, limited, wait) in rate_limits() {
            let harness = Harness::new();
            harness.link(harness.project_id).await;
            let other = harness.project("other-project").await;
            harness.link(other).await;
            let mut script = limited;
            if script.len() == 1 {
                script.push(minted_long());
            }
            script.push(commits(&[Some("Found")]));
            let server = Server::start_with_headers(script).await;
            let mut titles = harness.titles(&server);

            titles
                .ingest(&harness.log, harness.project_id, &[git_hash(1)])
                .await;
            // Just before the reset, neither project of the installation asks, or it would take
            // the answer that stores a row.
            harness.at(DateTime::TEST.timestamp() + wait - 1);
            titles
                .ingest(&harness.log, harness.project_id, &[git_hash(1)])
                .await;
            titles.ingest(&harness.log, other, &[git_hash(1)]).await;
            assert_eq!(harness.rows(harness.project_id).await, [], "{what}");
            assert_eq!(harness.rows(other).await, [], "{what}");

            harness.at(DateTime::TEST.timestamp() + wait);
            titles
                .ingest(&harness.log, harness.project_id, &[git_hash(1)])
                .await;
            assert_eq!(
                harness.rows(harness.project_id).await,
                [(hash(1), Some("Found".to_owned()))],
                "{what}"
            );
            server.received().await;
        }
    }

    // Kills a sweep that reads past 90 days, asks again for a stored hash, asks for more than 100
    // at once or in any order but newest first, or asks for an unlinked or disconnected project.
    #[tokio::test]
    async fn the_sweep_fetches_recent_hashes_without_a_row_in_batches_of_100() {
        let mut harness = Harness::new();
        harness.link(harness.project_id).await;
        // Newest first: hash 1 started a minute ago, hash 150 started 150 minutes ago.
        for n in (1..=150).rev() {
            harness
                .report(harness.project_id, &hash(n), i64::try_from(n).unwrap() * 60)
                .await;
        }
        harness
            .report(harness.project_id, &hash(1000), 91 * DAY)
            .await;
        harness
            .report(harness.project_id, &hash(1001), 89 * DAY)
            .await;
        diesel::insert_into(schema::commit::table)
            .values((
                schema::commit::project_id.eq(harness.project_id),
                schema::commit::hash.eq(hash(1001)),
                schema::commit::title.eq(Some("Stored")),
                schema::commit::created.eq(DateTime::TEST),
            ))
            .execute(&mut *harness.connection.lock().await)
            .unwrap();
        let unlinked = harness.project("unlinked").await;
        harness.report(unlinked, &hash(2000), DAY).await;
        let disconnected = harness.project("disconnected").await;
        harness
            .link(disconnected)
            .await
            .disconnect(
                &mut *harness.connection.lock().await,
                LinkedRepositoryReason::Removed,
            )
            .unwrap();
        harness.report(disconnected, &hash(3000), DAY).await;
        let server = Server::start_with_headers(vec![
            minted_long(),
            commits(&[Some("Title"); 100]),
            commits(&[Some("Title"); 50]),
        ])
        .await;
        let mut titles = harness.titles(&server);

        titles.sweep(&harness.log).await;

        let received = server.received().await;
        assert_eq!(request_lines(&received), [MINT, GRAPHQL, GRAPHQL]);
        let expected = (1..=150).map(hash).collect::<Vec<_>>();
        let [_mint, first, second] = received.as_slice() else {
            panic!("Not three requests");
        };
        assert_eq!(asked(first), expected[..100]);
        assert_eq!(asked(second), expected[100..]);
        assert_eq!(harness.rows(harness.project_id).await.len(), 151);
        assert_eq!(harness.rows(unlinked).await, []);
        assert_eq!(harness.rows(disconnected).await, []);
        assert_eq!(harness.failures(), Vec::<String>::new());
    }

    // Kills a fill that waits for the next sweep, and one that reads another project.
    #[tokio::test]
    async fn a_fill_fetches_its_project_at_once() {
        let mut harness = Harness::new();
        let other = harness.project("other-project").await;
        harness.link(other).await;
        harness.report(other, &hash(9), DAY).await;
        harness.report(harness.project_id, &hash(1), DAY).await;
        harness.link(harness.project_id).await;
        let server =
            Server::start_with_headers(vec![minted_long(), commits(&[Some("Filled")])]).await;
        let mut titles = harness.titles(&server);

        titles
            .work(&harness.log, vec![Work::Fill(harness.project_id)])
            .await;

        assert_eq!(
            harness.rows(harness.project_id).await,
            [(hash(1), Some("Filled".to_owned()))]
        );
        assert_eq!(harness.rows(other).await, []);
        server.received().await;
    }

    // Kills a token that is minted for every fetch, kept within five minutes of its expiry, or
    // kept after GitHub refuses it.
    #[tokio::test]
    async fn fetches_inside_the_token_life_mint_one_time() {
        let harness = Harness::new();
        harness.link(harness.project_id).await;
        let expires = DateTime::TEST.timestamp() + 60 * 60;
        let server = Server::start_with_headers(vec![
            minted(expires),
            commits(&[Some("One")]),
            commits(&[Some("Two")]),
            // Inside the last five minutes, the cached token is not used.
            minted(expires + 60 * 60),
            commits(&[Some("Three")]),
            // A refused token is dropped, so the retry mints.
            status(401, Vec::new()),
            minted(expires + 2 * 60 * 60),
            commits(&[Some("Four")]),
        ])
        .await;
        let mut titles = harness.titles(&server);

        titles
            .ingest(&harness.log, harness.project_id, &[git_hash(1)])
            .await;
        harness.later(30 * 60);
        titles
            .ingest(&harness.log, harness.project_id, &[git_hash(2)])
            .await;
        harness.at(expires - 5 * 60);
        titles
            .ingest(&harness.log, harness.project_id, &[git_hash(3)])
            .await;
        titles
            .ingest(&harness.log, harness.project_id, &[git_hash(4)])
            .await;
        titles
            .ingest(&harness.log, harness.project_id, &[git_hash(4)])
            .await;

        let received = server.received().await;
        assert_eq!(
            request_lines(&received),
            [
                MINT, GRAPHQL, GRAPHQL, MINT, GRAPHQL, GRAPHQL, MINT, GRAPHQL
            ]
        );
        assert_eq!(harness.rows(harness.project_id).await.len(), 4);
    }

    // Kills fetching one hash once per report or per letter case, which two branch heads at one
    // commit send.
    #[tokio::test]
    async fn a_hash_on_two_branch_heads_makes_one_row_and_one_fetch() {
        let harness = Harness::new();
        harness.link(harness.project_id).await;
        let server =
            Server::start_with_headers(vec![minted_long(), commits(&[Some("Shared")])]).await;
        let mut titles = harness.titles(&server);
        let upper = hash(LETTERS).to_uppercase().parse::<GitHash>().unwrap();

        titles
            .work(
                &harness.log,
                vec![
                    Work::Hashes(harness.project_id, vec![git_hash(LETTERS)]),
                    Work::Hashes(harness.project_id, vec![upper.clone()]),
                ],
            )
            .await;
        titles
            .work(
                &harness.log,
                vec![Work::Hashes(harness.project_id, vec![upper])],
            )
            .await;

        let received = server.received().await;
        assert_eq!(request_lines(&received), [MINT, GRAPHQL]);
        let [_mint, graphql] = received.as_slice() else {
            panic!("Not two requests");
        };
        assert_eq!(asked(graphql), [hash(LETTERS)]);
        assert_eq!(
            harness.rows(harness.project_id).await,
            [(hash(LETTERS), Some("Shared".to_owned()))]
        );
        assert_eq!(harness.failures(), Vec::<String>::new());
    }

    /// Records when each sweep starts, and with `hang`, never ends a sweep, or with `hang_work`,
    /// any work.
    struct Schedule {
        sweeps: Arc<Mutex<Vec<Instant>>>,
        hang: bool,
        hang_work: bool,
    }

    impl super::Fetch for Schedule {
        async fn sweep(&mut self, _log: &Logger) {
            self.sweeps.lock().unwrap().push(Instant::now());
            if self.hang {
                std::future::pending::<()>().await;
            }
        }

        async fn work(&mut self, _log: &Logger, _batch: Vec<Work>) {
            if self.hang_work {
                std::future::pending::<()>().await;
            }
        }
    }

    /// Let the loop run until it waits again.
    async fn settle() {
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
    }

    // Kills a loop that waits a period for its first sweep, never sweeps again, or sweeps before
    // the period ends.
    #[tokio::test(start_paused = true)]
    async fn the_loop_sweeps_at_start_and_every_24_hours() {
        // Written out, so the test does not take the period from the code it checks.
        let day = Duration::from_hours(24);
        let sweeps = Arc::new(Mutex::new(Vec::new()));
        let (_sender, receiver) = mpsc::channel(QUEUE);
        let shutdown = CancellationToken::new();
        let start = Instant::now();
        let task = tokio::spawn(run(
            Schedule {
                sweeps: Arc::clone(&sweeps),
                hang: false,
                hang_work: false,
            },
            receiver,
            shutdown.clone(),
            Logger::root(slog::Discard, slog::o!()),
        ));

        settle().await;
        assert_eq!(*sweeps.lock().unwrap(), [start]);
        tokio::time::advance(day.checked_sub(Duration::from_secs(1)).unwrap()).await;
        settle().await;
        assert_eq!(*sweeps.lock().unwrap(), [start]);
        tokio::time::advance(Duration::from_secs(1)).await;
        settle().await;
        assert_eq!(*sweeps.lock().unwrap(), [start, start + day]);

        shutdown.cancel();
        task.await.unwrap();
    }

    // Kills a sweep that shutdown cannot cut short, which would hold up the server's shutdown.
    #[tokio::test(start_paused = true)]
    async fn shutdown_cuts_a_sweep_short() {
        let sweeps = Arc::new(Mutex::new(Vec::new()));
        let (_sender, receiver) = mpsc::channel(QUEUE);
        let shutdown = CancellationToken::new();
        let task = tokio::spawn(run(
            Schedule {
                sweeps: Arc::clone(&sweeps),
                hang: true,
                hang_work: false,
            },
            receiver,
            shutdown.clone(),
            Logger::root(slog::Discard, slog::o!()),
        ));
        settle().await;
        assert_eq!(sweeps.lock().unwrap().len(), 1);

        shutdown.cancel();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("The loop waited out the sweep")
            .unwrap();
    }

    // Kills work that shutdown cannot cut short, which would hold up the server's shutdown.
    #[tokio::test(start_paused = true)]
    async fn shutdown_cuts_work_short() {
        let sweeps = Arc::new(Mutex::new(Vec::new()));
        let (sender, receiver) = mpsc::channel(QUEUE);
        let shutdown = CancellationToken::new();
        let task = tokio::spawn(run(
            Schedule {
                sweeps: Arc::clone(&sweeps),
                hang: false,
                hang_work: true,
            },
            receiver,
            shutdown.clone(),
            Logger::root(slog::Discard, slog::o!()),
        ));
        settle().await;
        sender.try_send(Work::Fill(ProjectId::from_raw(1))).unwrap();
        settle().await;
        // The work holds the only item, so the queue is empty again.
        assert_eq!(sender.capacity(), QUEUE);

        shutdown.cancel();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("The loop waited out the work")
            .unwrap();
    }
}
