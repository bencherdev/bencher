use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

use bencher_callback::{CallbackKey, CallbackSender};
use bencher_json::Clock;
use diesel::r2d2::{ConnectionManager, Pool};
use slog::Logger;
use tokio::task::{self, JoinError, JoinSet};
use tokio_util::sync::CancellationToken;

use super::DbConnection;
use crate::model::runner::{JobId, QueryJobCallback};

mod delivery;
mod governor;

use delivery::Delivery;

/// Owns the tasks that deliver job callbacks, at most one per job, so shutdown can await every one.
#[derive(Clone)]
pub struct Callbacks {
    delivery: Arc<Delivery>,
    tasks: Arc<Mutex<Tasks>>,
}

#[derive(Default)]
struct Tasks {
    running: JoinSet<()>,
    /// The job each live task delivers.
    claims: HashMap<task::Id, JobId>,
    closed: bool,
}

impl Callbacks {
    pub fn new(
        connection: Arc<tokio::sync::Mutex<DbConnection>>,
        reader: Pool<ConnectionManager<DbConnection>>,
        key: CallbackKey,
        sender: Arc<dyn CallbackSender>,
        shutdown: CancellationToken,
        clock: Clock,
    ) -> Self {
        // Counted at each export, on a connection only if one is free, so a busy pool skips a point.
        #[cfg(feature = "otel")]
        {
            let reader = reader.clone();
            bencher_otel::ApiMeter::observe(bencher_otel::ApiGauge::CallbackPending, move || {
                QueryJobCallback::count_pending(&mut *reader.try_get()?).ok()
            });
        }
        Self {
            delivery: Arc::new(Delivery::new(
                connection, reader, key, sender, shutdown, clock,
            )),
            tasks: Arc::default(),
        }
    }

    /// Fire a job's callback once the job's terminal status has committed, delivering it from a
    /// task so the caller never waits on the network. The task sends only a pending callback of a
    /// terminal job.
    pub fn fire(&self, log: &Logger, job_id: JobId) {
        let mut tasks = self.lock();
        while let Some(joined) = tasks.running.try_join_next_with_id() {
            let (id, joined) = match joined {
                Ok((id, ())) => (id, Ok(())),
                Err(e) => (e.id(), Err(e)),
            };
            tasks.claims.remove(&id);
            log_join(log, joined);
        }
        if tasks.closed {
            slog::debug!(log, "Not firing a job callback during shutdown"; "job_id" => ?job_id);
            return;
        }
        if tasks.claims.values().any(|claimed| *claimed == job_id) {
            return;
        }
        let delivery = Arc::clone(&self.delivery);
        let task_log = log.clone();
        let id = tasks
            .running
            .spawn(async move {
                delivery.deliver(&task_log, job_id).await;
            })
            .id();
        tasks.claims.insert(id, job_id);
    }

    /// Fire every pending callback whose job is already terminal, which no process has delivered.
    pub async fn fire_pending(&self, log: &Logger) {
        let pending = QueryJobCallback::pending_on_terminal_jobs(
            &mut *self.delivery.connection().lock().await,
        );
        let job_ids = match pending {
            Ok(job_ids) => job_ids,
            Err(e) => {
                slog::error!(log, "Failed to query pending job callbacks: {e}");
                return;
            },
        };
        if !job_ids.is_empty() {
            slog::info!(log, "Firing {} pending job callback(s)", job_ids.len());
        }
        for job_id in job_ids {
            self.fire(log, job_id);
        }
    }

    /// Stop firing, then wait for every delivery task; the shutdown signal cuts each one short.
    pub async fn drain(&self, log: &Logger) {
        let mut running = {
            let mut tasks = self.lock();
            tasks.closed = true;
            std::mem::take(&mut tasks.running)
        };
        while let Some(joined) = running.join_next().await {
            log_join(log, joined);
        }
    }

    // A fire and a drain share one lock, so no task starts once a drain has begun.
    fn lock(&self) -> std::sync::MutexGuard<'_, Tasks> {
        self.tasks.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn log_join(log: &Logger, joined: Result<(), JoinError>) {
    if let Err(e) = joined {
        slog::error!(log, "Job callback delivery task failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        error::Error as _,
        fmt::{self, Write as _},
        iter,
        net::{IpAddr, Ipv4Addr},
        sync::{Arc, Mutex},
        time::Duration,
    };

    use async_trait::async_trait;
    use bencher_callback::{
        AddressClass, CallbackAttempt, CallbackBlock, CallbackKey, CallbackRequest, CallbackSender,
        RateLimit,
    };
    use bencher_json::{
        BENCHER_API_VERSION, Clock, DateTime, JobStatus, JobUuid, JsonNewCallback, Secret,
        runner::JobCallbackState,
    };
    use diesel::{
        Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
        connection::SimpleConnection as _,
        r2d2::{ConnectionManager, Pool},
    };
    use http::StatusCode;
    use pretty_assertions::assert_eq;
    use slog::{Drain, KV as _, Logger, OwnedKVList, Record};
    use tokio::time::Instant;
    use tokio_util::sync::CancellationToken;

    use super::Callbacks;
    use crate::{
        context::DbConnection,
        model::{
            project::report::{QueryReport, ReportMode},
            runner::{InsertJobCallback, JobId, QueryJob, QueryJobCallback},
        },
        run_migrations, schema,
        test_util::{JobFixture, create_job, create_job_fixture},
    };

    const USERINFO_MARKER: &str = "USERINFO-6b1f";
    const PATH_MARKER: &str = "PATH-2c9d";
    const QUERY_MARKER: &str = "QUERY-8e4a";
    const HEADER_MARKER: &str = "HEADER-5f07";
    const MARKERS: [&str; 4] = [USERINFO_MARKER, PATH_MARKER, QUERY_MARKER, HEADER_MARKER];
    const REPORT_UUID: &str = "00000000-0000-0000-0000-000000000040";
    const NO_READ_CONNECTION: &str = "No read connection for a job callback";
    const DOCS_URL_COMMENT: &str = "(+https://bencher.dev/docs/explanation/images/)";

    /// What the scripted receiver does with its next request.
    enum Answer {
        Status(u16),
        /// A 403 or a 429 with its rate limit headers.
        Limited(u16, RateLimit),
        /// A 200 that takes this long to arrive.
        Slow(Duration),
        TimedOut,
        Connection,
        Blocked(CallbackBlock),
        Hang,
        Panic,
    }

    /// Answers each request with the next scripted answer, then with 200.
    struct ScriptedSender {
        answers: Mutex<VecDeque<Answer>>,
        sent: Mutex<Vec<(Instant, CallbackRequest)>>,
    }

    #[async_trait]
    impl CallbackSender for ScriptedSender {
        async fn send(&self, request: &CallbackRequest) -> CallbackAttempt {
            self.sent
                .lock()
                .unwrap()
                .push((Instant::now(), request.clone()));
            let answer = self
                .answers
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Answer::Status(200));
            match answer {
                Answer::Status(code) => {
                    let status = StatusCode::from_u16(code).unwrap();
                    if status.is_success() {
                        CallbackAttempt::Delivered(status)
                    } else {
                        CallbackAttempt::Refused(status, RateLimit::default())
                    }
                },
                Answer::Limited(code, rate_limit) => {
                    CallbackAttempt::Refused(StatusCode::from_u16(code).unwrap(), rate_limit)
                },
                Answer::Slow(duration) => {
                    tokio::time::sleep(duration).await;
                    CallbackAttempt::Delivered(StatusCode::OK)
                },
                Answer::TimedOut => CallbackAttempt::TimedOut,
                Answer::Connection => CallbackAttempt::Connection(Arc::new(connection_error())),
                Answer::Blocked(block) => CallbackAttempt::Blocked(block),
                Answer::Hang => std::future::pending().await,
                Answer::Panic => panic!("the receiver panicked"),
            }
        }
    }

    // A connection error with a cause, which needs neither a client nor a crypto provider this way.
    fn connection_error() -> reqwest::Error {
        reqwest::Request::try_from(http::Request::builder().uri("/relative").body("").unwrap())
            .unwrap_err()
    }

    /// Keeps every log line, its message and its key-value pairs, as text.
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
        fixture: JobFixture,
        key: CallbackKey,
        sender: Arc<ScriptedSender>,
        shutdown: CancellationToken,
        callbacks: Callbacks,
        capture: Capture,
        log: Logger,
    }

    impl Harness {
        fn new<A>(answers: A) -> Self
        where
            A: IntoIterator<Item = Answer>,
        {
            // A file, so the delivery's read pool sees what the writer commits.
            let dir = tempfile::TempDir::new().unwrap();
            let path = dir.path().join("callbacks.db").to_str().unwrap().to_owned();
            let mut conn = DbConnection::establish(&path).unwrap();
            conn.batch_execute("PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;")
                .unwrap();
            conn.batch_execute("PRAGMA journal_mode = WAL").unwrap();
            run_migrations(&mut conn).unwrap();
            let fixture = create_job_fixture(&mut conn);
            // A short wait for a connection, so a test that holds both waits out the pool quickly.
            let reader = Pool::builder()
                .max_size(2)
                .connection_timeout(Duration::from_millis(50))
                .build(ConnectionManager::<DbConnection>::new(&path))
                .unwrap();
            let connection = Arc::new(tokio::sync::Mutex::new(conn));
            let key = CallbackKey::new(&secret("callbacks-test-secret")).unwrap();
            let sender = Arc::new(ScriptedSender {
                answers: Mutex::new(answers.into_iter().collect()),
                sent: Mutex::new(Vec::new()),
            });
            let shutdown = CancellationToken::new();
            let callbacks = Callbacks::new(
                Arc::clone(&connection),
                reader.clone(),
                key.clone(),
                sender.clone(),
                shutdown.clone(),
                Clock::Custom(Arc::new(|| DateTime::TEST)),
            );
            let capture = Capture::default();
            let log = Logger::root(capture.clone(), slog::o!());
            Self {
                _dir: dir,
                connection,
                reader,
                fixture,
                key,
                sender,
                shutdown,
                callbacks,
                capture,
                log,
            }
        }

        /// A job in `status` with a pending callback sealed from `callback`.
        async fn job(&self, status: JobStatus, callback: &JsonNewCallback) -> (JobId, JobUuid) {
            self.job_sealed(status, &serde_json::to_vec(callback).unwrap(), &self.key)
                .await
        }

        async fn job_sealed(
            &self,
            status: JobStatus,
            plaintext: &[u8],
            key: &CallbackKey,
        ) -> (JobId, JobUuid) {
            let conn = &mut *self.connection.lock().await;
            let job_id = create_job(conn, self.fixture, status);
            let job_uuid = QueryJob::get(conn, job_id).unwrap().uuid;
            InsertJobCallback::pending(job_id, key.seal(job_uuid, plaintext).unwrap(), at(-1))
                .insert(conn)
                .unwrap();
            (job_id, job_uuid)
        }

        fn fire(&self, job_id: JobId) {
            self.callbacks.fire(&self.log, job_id);
        }

        /// Fire a job's callback and wait for its delivery to settle.
        async fn deliver(&self, job_id: JobId) {
            self.fire(job_id);
            self.callbacks.drain(&self.log).await;
        }

        async fn row(&self, job_id: JobId) -> QueryJobCallback {
            QueryJobCallback::get(&mut *self.connection.lock().await, job_id).unwrap()
        }

        /// The row's state, attempt count, last status, and whether it still holds its request.
        async fn settled(&self, job_id: JobId) -> (JobCallbackState, i32, Option<u16>, bool) {
            let row = self.row(job_id).await;
            (
                row.state,
                row.attempts,
                row.status.map(u16::from),
                row.request.is_some(),
            )
        }

        fn sent(&self) -> usize {
            self.sender.sent.lock().unwrap().len()
        }

        fn sent_at(&self) -> Vec<Instant> {
            self.sender
                .sent
                .lock()
                .unwrap()
                .iter()
                .map(|(at, _)| *at)
                .collect()
        }

        /// When each request went out, and the job its body names.
        fn sent_jobs(&self) -> Vec<(Instant, JobUuid)> {
            self.sender
                .sent
                .lock()
                .unwrap()
                .iter()
                .map(|(at, request)| {
                    let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
                    (*at, body["job"]["uuid"].as_str().unwrap().parse().unwrap())
                })
                .collect()
        }

        /// When each request went out, and its host.
        fn sent_hosts(&self) -> Vec<(Instant, String)> {
            self.sender
                .sent
                .lock()
                .unwrap()
                .iter()
                .map(|(at, request)| (*at, request.url.host_str().unwrap().to_owned()))
                .collect()
        }

        fn logs(&self) -> Vec<String> {
            self.capture.0.lock().unwrap().clone()
        }

        fn logged(&self, message: &str) -> Vec<String> {
            self.logs()
                .into_iter()
                .filter(|line| line.starts_with(message))
                .collect()
        }

        /// Drain once shutdown is signalled; a delivery that ignores the signal fails the test.
        async fn drain_after_shutdown(&self) {
            tokio::time::timeout(Duration::from_secs(1), self.callbacks.drain(&self.log))
                .await
                .expect("the drain ends once shutdown cuts every delivery");
        }

        /// The fixture's report, as the report endpoint builds it.
        async fn report_json(&self) -> serde_json::Value {
            let conn = &mut *self.connection.lock().await;
            let report: QueryReport = schema::report::table
                .find(self.fixture.report_id)
                .first(conn)
                .unwrap();
            serde_json::to_value(report.into_json(&self.log, conn, ReportMode::Full).unwrap())
                .unwrap()
        }

        /// Delete the fixture's testbed behind its report's back, so the report no longer builds.
        async fn lose_the_testbed(&self) {
            self.connection
                .lock()
                .await
                .batch_execute(
                    "PRAGMA foreign_keys = OFF; DELETE FROM testbed; PRAGMA foreign_keys = ON;",
                )
                .unwrap();
        }

        /// Soft-delete the fixture's project, as deleting it in the console does.
        async fn delete_the_project(&self) {
            diesel::update(schema::project::table)
                .set(schema::project::deleted.eq(Some(DateTime::TEST)))
                .execute(&mut *self.connection.lock().await)
                .unwrap();
        }

        /// Delete the fixture's report, as deleting it in the console does, and with it its jobs and
        /// their callbacks.
        async fn delete_the_report(&self) {
            diesel::delete(schema::report::table.find(self.fixture.report_id))
                .execute(&mut *self.connection.lock().await)
                .unwrap();
        }

        /// Record an attempt on the pending callback, as an earlier delivery did.
        async fn record_attempt(&self, job_id: JobId, status: StatusCode) {
            assert!(
                QueryJobCallback::record_attempt(
                    &mut *self.connection.lock().await,
                    job_id,
                    Some(status),
                    None,
                    at(0),
                )
                .unwrap()
            );
        }

        /// Wait, without moving the paused clock, until the job's callback settles.
        async fn until_settled(&self, job_id: JobId) {
            for _ in 0..10_000 {
                if self.row(job_id).await.state != JobCallbackState::Pending {
                    return;
                }
                tokio::task::yield_now().await;
            }
            panic!("the callback never settled");
        }

        /// Wait, without moving the paused clock, until the receiver has seen `count` requests.
        async fn until_sent(&self, count: usize) {
            for _ in 0..10_000 {
                if self.sent() >= count {
                    return;
                }
                tokio::task::yield_now().await;
            }
            panic!("the receiver never saw {count} request(s)");
        }

        /// Wait, without moving the paused clock, until a line starting with `message` is logged.
        async fn until_logged(&self, message: &str) {
            for _ in 0..10_000 {
                if !self.logged(message).is_empty() {
                    return;
                }
                tokio::task::yield_now().await;
            }
            panic!("{message} was never logged");
        }
    }

    fn secret(value: &str) -> Secret {
        value.parse().unwrap()
    }

    fn at(seconds: i64) -> DateTime {
        DateTime::from(DateTime::TEST.into_inner() + chrono::Duration::seconds(seconds))
    }

    /// A body with the five values, and without the report.
    fn callback() -> JsonNewCallback {
        callback_with(Some(serde_json::json!({
            "job": { "uuid": "{{ job.uuid }}", "status": "{{ job.status }}" },
            "report": { "uuid": "{{ report.uuid }}" },
            "project": {
                "uuid": "{{ project.uuid }}",
                "name": "{{ project.name }}",
                "slug": "{{ project.slug }}",
            },
        })))
    }

    fn callback_with(body: Option<serde_json::Value>) -> JsonNewCallback {
        JsonNewCallback::new(
            &format!(
                "https://bencher:{USERINFO_MARKER}@receiver.example/hooks/{PATH_MARKER}?token={QUERY_MARKER}"
            ),
            [
                (
                    "Authorization".to_owned(),
                    format!("Bearer {HEADER_MARKER}"),
                ),
                ("X-Receiver".to_owned(), "bencher".to_owned()),
            ],
            body,
        )
        .unwrap()
    }

    /// A callback that names its job, sent to `url` with `authorization`.
    fn callback_to(url: &str, authorization: &str) -> JsonNewCallback {
        JsonNewCallback::new(
            url,
            [("Authorization".to_owned(), authorization.to_owned())],
            Some(serde_json::json!({ "job": { "uuid": "{{ job.uuid }}" } })),
        )
        .unwrap()
    }

    fn retry_after(seconds: u64) -> RateLimit {
        RateLimit {
            retry_after: Some(seconds),
            ..RateLimit::default()
        }
    }

    fn assert_no_marker(text: &str) {
        for marker in MARKERS {
            assert!(!text.contains(marker), "{marker} leaked into: {text}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_fire_delivers_the_rendered_request() {
        let harness = Harness::new([]);
        let (job_id, job_uuid) = harness.job(JobStatus::Processed, &callback()).await;

        harness.deliver(job_id).await;

        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Delivered, 1, Some(200), false)
        );
        let sent = harness.sender.sent.lock().unwrap();
        assert_eq!(sent.len(), 1, "one request");
        let (_, request) = sent.first().unwrap();
        assert_eq!(&request.url, callback().url());
        let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "job": { "uuid": job_uuid.to_string(), "status": "processed" },
                "report": { "uuid": REPORT_UUID },
                "project": {
                    "uuid": "00000000-0000-0000-0000-000000000002",
                    "name": "Test Project",
                    "slug": "test-project",
                },
            })
        );
        let header = |name: &str| request.headers.get(name).unwrap().to_str().unwrap();
        assert_eq!(header("content-type"), "application/json");
        assert_eq!(
            header("user-agent"),
            format!("Bencher/{BENCHER_API_VERSION} {DOCS_URL_COMMENT}")
        );
        assert_eq!(header("authorization"), format!("Bearer {HEADER_MARKER}"));
        assert_eq!(header("x-receiver"), "bencher");
    }

    #[tokio::test(start_paused = true)]
    async fn the_body_names_the_status_that_fired() {
        let harness = Harness::new([]);
        for (sent, (status, name)) in (1..).zip([
            (JobStatus::Processed, "processed"),
            (JobStatus::Failed, "failed"),
            (JobStatus::Canceled, "canceled"),
        ]) {
            let (job_id, _) = harness.job(status, &callback()).await;
            harness.fire(job_id);
            // One credential sends a second apart.
            tokio::time::sleep(Duration::from_secs(1)).await;
            harness.until_sent(sent).await;
            let body = harness
                .sender
                .sent
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .1
                .body
                .clone();
            let body: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(body["job"]["status"], name, "{status:?}");
        }
        harness.callbacks.drain(&harness.log).await;
    }

    #[tokio::test(start_paused = true)]
    async fn a_callback_without_a_body_sends_the_report() {
        let harness = Harness::new([]);
        let (job_id, job_uuid) = harness
            .job(JobStatus::Processed, &callback_with(None))
            .await;

        harness.deliver(job_id).await;

        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Delivered, 1, Some(200), false)
        );
        let report = harness.report_json().await;
        assert_eq!(report["uuid"], REPORT_UUID);
        assert_eq!(report["job"], job_uuid.to_string());
        let sent = harness.sender.sent.lock().unwrap();
        assert_eq!(sent.len(), 1, "one request");
        let (_, request) = sent.first().unwrap();
        let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
        assert_eq!(body["uuid"], REPORT_UUID);
        assert_eq!(body["job"], job_uuid.to_string());
        assert_eq!(body, report);
        let header = |name: &str| request.headers.get(name).unwrap().to_str().unwrap();
        assert_eq!(header("content-type"), "application/json");
        assert_eq!(
            header("user-agent"),
            format!("Bencher/{BENCHER_API_VERSION} {DOCS_URL_COMMENT}")
        );
        assert_eq!(header("authorization"), format!("Bearer {HEADER_MARKER}"));
        // The report can hold private results, so the delivery never logs it.
        let delivery = harness.logs();
        assert_ne!(delivery, Vec::<String>::new());
        for line in delivery {
            for private in [REPORT_UUID, "Test Project"] {
                assert!(!line.contains(private), "{private} in {line}");
            }
        }
    }

    /// Without a read connection for the report, the delivery tries again each pool timeout, and logs
    /// only the first failure and the recovery.
    #[tokio::test(start_paused = true)]
    async fn a_delivery_waits_for_a_read_connection() {
        let harness = Harness::new([]);
        let (job_id, _) = harness
            .job(JobStatus::Processed, &callback_with(None))
            .await;
        let held = [harness.reader.get().unwrap(), harness.reader.get().unwrap()];
        let timeout = harness.reader.connection_timeout();
        let fired = Instant::now();

        harness.fire(job_id);
        harness.until_logged(NO_READ_CONNECTION).await;
        // The first try and two more.
        tokio::time::sleep(timeout * 2 + timeout / 2).await;
        assert_eq!(harness.sent(), 0);
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Pending, 0, None, true)
        );

        drop(held);
        harness.callbacks.drain(&harness.log).await;
        assert_eq!(harness.sent_at(), [fired + timeout * 3]);
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Delivered, 1, Some(200), false)
        );
        assert_eq!(
            harness.logged(NO_READ_CONNECTION).len(),
            1,
            "logged once, not per try"
        );
        assert_eq!(
            harness
                .logged("Got a read connection for a job callback")
                .len(),
            1
        );
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_ends_the_wait_for_a_read_connection() {
        let harness = Harness::new([]);
        let (job_id, _) = harness
            .job(JobStatus::Processed, &callback_with(None))
            .await;
        let _held = [harness.reader.get().unwrap(), harness.reader.get().unwrap()];

        harness.fire(job_id);
        harness.until_logged(NO_READ_CONNECTION).await;
        harness.shutdown.cancel();
        harness.drain_after_shutdown().await;

        assert_eq!(harness.sent(), 0);
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Pending, 0, None, true)
        );
    }

    /// A body that sends the report fails as the default body does, and one without it never builds it.
    #[tokio::test(start_paused = true)]
    async fn a_report_that_does_not_build_is_never_sent() {
        let harness = Harness::new([]);
        let (default, _) = harness
            .job(JobStatus::Processed, &callback_with(None))
            .await;
        let (placeholder, _) = harness
            .job(
                JobStatus::Processed,
                &callback_with(Some(serde_json::json!({ "report": "{{ report }}" }))),
            )
            .await;
        let (without, _) = harness.job(JobStatus::Processed, &callback()).await;
        harness.lose_the_testbed().await;

        for job_id in [default, placeholder, without] {
            harness.fire(job_id);
        }
        harness.callbacks.drain(&harness.log).await;

        assert_eq!(
            harness.sent(),
            1,
            "only the body without the report is sent"
        );
        for job_id in [default, placeholder] {
            assert_eq!(
                harness.settled(job_id).await,
                (JobCallbackState::Failed, 0, None, false)
            );
        }
        assert_eq!(
            harness.settled(without).await,
            (JobCallbackState::Delivered, 1, Some(200), false)
        );
        let unsent = harness.logged("Callback request not sent");
        assert_eq!(unsent.len(), 2, "{unsent:?}");
        for line in &unsent {
            assert!(line.contains("stage=report"), "{unsent:?}");
        }
        let failed = harness.logged("Callback failed");
        assert_eq!(failed.len(), 2, "{failed:?}");
        for line in &failed {
            assert!(line.contains("reason=Render"), "{failed:?}");
        }
    }

    /// The report endpoint no longer returns a deleted project's report, so neither does a callback.
    #[tokio::test(start_paused = true)]
    async fn a_deleted_project_fires_placeholders_but_never_its_report() {
        let harness = Harness::new([]);
        let (placeholders, placeholders_uuid) =
            harness.job(JobStatus::Processed, &callback()).await;
        let (report, _) = harness
            .job(JobStatus::Processed, &callback_with(None))
            .await;
        harness.delete_the_project().await;

        harness.fire(placeholders);
        harness.fire(report);
        harness.callbacks.drain(&harness.log).await;

        assert_eq!(
            harness.settled(placeholders).await,
            (JobCallbackState::Delivered, 1, Some(200), false)
        );
        {
            let sent = harness.sender.sent.lock().unwrap();
            assert_eq!(sent.len(), 1, "only the body without the report is sent");
            let body: serde_json::Value =
                serde_json::from_str(&sent.first().unwrap().1.body).unwrap();
            assert_eq!(body["job"]["uuid"], placeholders_uuid.to_string());
        }
        assert_eq!(
            harness.settled(report).await,
            (JobCallbackState::Failed, 0, None, false)
        );
        let unsent = harness.logged("Callback request not sent");
        assert_eq!(unsent.len(), 1, "{unsent:?}");
        assert!(unsent[0].contains("stage=report"), "{unsent:?}");
        let failed = harness.logged("Callback failed");
        assert_eq!(failed.len(), 1, "{failed:?}");
        assert!(failed[0].contains("reason=Render"), "{failed:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn no_other_status_fires() {
        let harness = Harness::new([]);
        for status in [
            JobStatus::Pending,
            JobStatus::Claimed,
            JobStatus::Running,
            JobStatus::Unknown,
            JobStatus::Completed,
        ] {
            let (job_id, _) = harness.job(status, &callback()).await;
            harness.fire(job_id);
            assert_eq!(
                harness.settled(job_id).await,
                (JobCallbackState::Pending, 0, None, true),
                "{status:?}"
            );
        }
        harness.callbacks.drain(&harness.log).await;
        assert_eq!(harness.sent(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_second_fire_of_one_job_sends_nothing() {
        let harness = Harness::new([Answer::Status(500)]);
        let (job_id, _) = harness.job(JobStatus::Canceled, &callback()).await;

        harness.fire(job_id);
        harness.fire(job_id);
        // And one more while the delivery waits to retry.
        harness.until_sent(1).await;
        harness.fire(job_id);
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(4)).await;
        harness.until_sent(2).await;
        harness.until_settled(job_id).await;
        // And one more after it settles: its task is gone, and the callback is no longer pending.
        harness.fire(job_id);
        harness.callbacks.drain(&harness.log).await;

        assert_eq!(harness.sent(), 2);
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Delivered, 2, Some(200), false)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn retries_wait_four_then_sixteen_seconds() {
        let harness = Harness::new([Answer::Status(500), Answer::Status(500)]);
        let (job_id, _) = harness.job(JobStatus::Processed, &callback()).await;

        harness.deliver(job_id).await;

        let sent_at = harness.sent_at();
        assert_eq!(sent_at.len(), 3);
        let gaps: Vec<Duration> = sent_at.windows(2).map(|w| w[1] - w[0]).collect();
        assert_eq!(gaps, [Duration::from_secs(4), Duration::from_secs(16)]);
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Delivered, 3, Some(200), false)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn three_retryable_failures_exhaust_the_callback() {
        let harness = Harness::new([
            Answer::Status(500),
            Answer::Status(502),
            Answer::Status(500),
        ]);
        let (job_id, _) = harness.job(JobStatus::Processed, &callback()).await;

        harness.deliver(job_id).await;

        assert_eq!(harness.sent(), 3);
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Failed, 3, Some(500), false)
        );
        let failed = harness.logged("Callback failed");
        assert_eq!(failed.len(), 1, "{failed:?}");
        assert!(failed[0].contains("reason=Exhausted"), "{failed:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn a_delivery_stops_once_its_write_finds_no_pending_callback() {
        let harness = Harness::new([
            Answer::Status(500),
            Answer::Status(500),
            Answer::Status(500),
        ]);
        let (job_id, _) = harness.job(JobStatus::Processed, &callback()).await;

        harness.fire(job_id);
        harness.until_sent(1).await;
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Pending, 1, Some(500), true)
        );
        // While the delivery waits to retry.
        harness.delete_the_report().await;
        harness.callbacks.drain(&harness.log).await;

        // The retry finds no callback to record, so there is no third attempt.
        assert_eq!(harness.sent(), 2);
        assert_eq!(harness.logged("Job callback is no longer pending").len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn only_408_429_and_5xx_retry() {
        for code in [408, 429, 500, 503, 599] {
            let harness = Harness::new([Answer::Status(code)]);
            let (job_id, _) = harness.job(JobStatus::Processed, &callback()).await;
            harness.deliver(job_id).await;
            assert_eq!(harness.sent(), 2, "{code}");
            assert_eq!(
                harness.settled(job_id).await,
                (JobCallbackState::Delivered, 2, Some(200), false),
                "{code}"
            );
        }
        for (code, class) in [
            (101, "other"),
            (302, "3xx"),
            (400, "4xx"),
            (401, "4xx"),
            (404, "4xx"),
            (600, "other"),
        ] {
            let harness = Harness::new([Answer::Status(code)]);
            let (job_id, _) = harness.job(JobStatus::Processed, &callback()).await;
            harness.deliver(job_id).await;
            assert_eq!(harness.sent(), 1, "{code}");
            assert_eq!(
                harness.settled(job_id).await,
                (JobCallbackState::Failed, 1, Some(code), false),
                "{code}"
            );
            let attempt = harness.logged("Callback attempt");
            assert!(
                attempt[0].contains(&format!("outcome={class}")),
                "{attempt:?}"
            );
            let failed = harness.logged("Callback failed");
            assert!(failed[0].contains("reason=Refused"), "{failed:?}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_timeout_or_a_connection_error_retries() {
        for (answer, class) in [
            (Answer::TimedOut, "timeout"),
            (Answer::Connection, "connection"),
        ] {
            let harness = Harness::new([Answer::Status(503), answer]);
            let (job_id, _) = harness.job(JobStatus::Processed, &callback()).await;
            harness.deliver(job_id).await;
            assert_eq!(harness.sent(), 3, "{class}");
            // An attempt with no answer keeps the last status until one arrives.
            assert_eq!(
                harness.settled(job_id).await,
                (JobCallbackState::Delivered, 3, Some(200), false),
                "{class}"
            );
            let attempts = harness.logged("Callback attempt");
            assert!(
                attempts[1].contains(&format!("outcome={class}")),
                "{attempts:?}"
            );
            // slog writes an absent value as nothing.
            assert!(attempts[1].contains(" status= "), "{attempts:?}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_connection_error_logs_its_cause() {
        let error = connection_error();
        let cause = error.source().expect("the error has a cause").to_string();
        let harness = Harness::new([Answer::Connection]);
        let (job_id, _) = harness.job(JobStatus::Processed, &callback()).await;
        harness.deliver(job_id).await;
        let attempts = harness.logged("Callback attempt");
        assert!(
            attempts[0].contains(&format!("error={error}: {cause}")),
            "{attempts:?}"
        );
        for line in harness.logs() {
            assert_no_marker(&line);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_blocked_attempt_is_final() {
        let metadata = IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254));
        for (block, reason, label) in [
            (CallbackBlock::NotHttps, "Blocked(NotHttps)", " address= "),
            (
                CallbackBlock::Address {
                    address: metadata,
                    class: AddressClass::LinkLocal,
                },
                "Blocked(Address(LinkLocal))",
                "address=169.254.169.254",
            ),
        ] {
            let harness = Harness::new([Answer::Blocked(block)]);
            let (job_id, _) = harness.job(JobStatus::Processed, &callback()).await;
            harness.deliver(job_id).await;
            assert_eq!(harness.sent(), 1, "{reason}");
            assert_eq!(
                harness.settled(job_id).await,
                (JobCallbackState::Failed, 1, None, false),
                "{reason}"
            );
            let attempt = harness.logged("Callback attempt");
            assert!(attempt[0].contains("outcome=blocked"), "{attempt:?}");
            assert!(attempt[0].contains(label), "{attempt:?}");
            let failed = harness.logged("Callback failed");
            assert!(
                failed[0].contains(&format!("reason={reason}")),
                "{failed:?}"
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_request_sealed_under_another_secret_is_never_sent() {
        let harness = Harness::new([]);
        let other = CallbackKey::new(&secret("a-rotated-secret-key")).unwrap();
        let (job_id, _) = harness
            .job_sealed(
                JobStatus::Processed,
                &serde_json::to_vec(&callback()).unwrap(),
                &other,
            )
            .await;

        harness.deliver(job_id).await;

        assert_eq!(harness.sent(), 0);
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Failed, 0, None, false)
        );
        let failed = harness.logged("Callback failed");
        assert!(failed[0].contains("reason=Open"), "{failed:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn a_stored_request_that_no_longer_validates_is_never_sent() {
        let harness = Harness::new([]);
        // Plain `http` passed no release's validation, so this stands in for a rule tightened after a submit.
        let plaintext = serde_json::json!({
            "url": format!("http://bencher:{USERINFO_MARKER}@receiver.example/{PATH_MARKER}?q={QUERY_MARKER}"),
            "headers": { "authorization": HEADER_MARKER },
            "body": format!("{{\"marker\": \"{HEADER_MARKER}\"}}"),
        });
        let (job_id, _) = harness
            .job_sealed(
                JobStatus::Processed,
                plaintext.to_string().as_bytes(),
                &harness.key,
            )
            .await;

        harness.deliver(job_id).await;

        assert_eq!(harness.sent(), 0);
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Failed, 0, None, false)
        );
        let failed = harness.logged("Callback failed");
        assert!(failed[0].contains("reason=Render"), "{failed:?}");
        for line in harness.logs() {
            assert_no_marker(&line);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_callback_with_three_recorded_attempts_is_never_sent_again() {
        let harness = Harness::new([]);
        let (job_id, _) = harness.job(JobStatus::Processed, &callback()).await;
        for _ in 0..3 {
            harness
                .record_attempt(job_id, StatusCode::BAD_GATEWAY)
                .await;
        }

        harness.deliver(job_id).await;

        assert_eq!(harness.sent(), 0);
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Failed, 3, Some(502), false)
        );
        let failed = harness.logged("Callback failed");
        assert!(failed[0].contains("reason=Exhausted"), "{failed:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn a_restarted_delivery_continues_from_its_count() {
        let harness = Harness::new([]);
        let (job_id, _) = harness.job(JobStatus::Processed, &callback()).await;
        harness
            .record_attempt(job_id, StatusCode::BAD_GATEWAY)
            .await;
        let fired = Instant::now();

        harness.deliver(job_id).await;

        // Its next attempt is the second, so it waits the second attempt's 4 s first.
        assert_eq!(harness.sent_at(), [fired + Duration::from_secs(4)]);
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Delivered, 2, Some(200), false)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_between_attempts_leaves_the_callback_pending() {
        let harness = Harness::new([Answer::Status(500)]);
        let (job_id, _) = harness.job(JobStatus::Processed, &callback()).await;

        harness.fire(job_id);
        harness.until_sent(1).await;
        harness.shutdown.cancel();
        harness.drain_after_shutdown().await;

        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Pending, 1, Some(500), true)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_cuts_an_attempt_short_and_the_next_start_makes_it_again() {
        let harness = Harness::new([Answer::Hang]);
        let (job_id, _) = harness.job(JobStatus::Processed, &callback()).await;

        harness.fire(job_id);
        harness.until_sent(1).await;
        harness.shutdown.cancel();
        harness.drain_after_shutdown().await;

        // The cut attempt was never answered, so it is not recorded.
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Pending, 0, None, true)
        );

        let next = Callbacks::new(
            Arc::clone(&harness.connection),
            harness.reader.clone(),
            harness.key.clone(),
            harness.sender.clone(),
            CancellationToken::new(),
            Clock::Custom(Arc::new(|| DateTime::TEST)),
        );
        let started = Instant::now();
        next.fire_pending(&harness.log).await;
        next.drain(&harness.log).await;

        // The first attempt again, so no wait before it.
        assert_eq!(harness.sent_at().last(), Some(&started));
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Delivered, 1, Some(200), false)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_drained_owner_fires_nothing() {
        let harness = Harness::new([]);
        let (job_id, _) = harness.job(JobStatus::Processed, &callback()).await;

        harness.callbacks.drain(&harness.log).await;
        harness.fire(job_id);
        harness.callbacks.fire_pending(&harness.log).await;
        harness.callbacks.drain(&harness.log).await;

        assert_eq!(harness.sent(), 0);
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Pending, 0, None, true)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_panicking_delivery_is_logged_when_reaped_and_when_drained() {
        let harness = Harness::new([Answer::Panic, Answer::Status(200), Answer::Panic]);
        let (reaped, _) = harness.job(JobStatus::Processed, &callback()).await;
        let (drained, _) = harness.job(JobStatus::Processed, &callback()).await;

        harness.fire(reaped);
        harness.until_sent(1).await;
        tokio::task::yield_now().await;
        // The fire reaps the panicked task, and with it the job's claim, so the job fires again.
        harness.fire(reaped);
        assert_eq!(
            harness.logged("Job callback delivery task failed").len(),
            1,
            "the next fire reaps the finished task"
        );
        harness.until_sent(2).await;
        harness.until_settled(reaped).await;
        harness.fire(drained);
        // One credential sends a second apart.
        tokio::time::sleep(Duration::from_secs(1)).await;
        harness.until_sent(3).await;
        harness.callbacks.drain(&harness.log).await;

        let failed = harness.logged("Job callback delivery task failed");
        assert_eq!(failed.len(), 2, "{failed:?}");
        assert!(
            failed.iter().all(|line| line.contains("panicked")),
            "{failed:?}"
        );
        assert_eq!(
            harness.settled(reaped).await,
            (JobCallbackState::Delivered, 1, Some(200), false)
        );
        // The next start fires a callback that a panic left pending.
        assert_eq!(
            harness.settled(drained).await,
            (JobCallbackState::Pending, 0, None, true)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn every_attempt_logs_its_host_and_never_a_secret() {
        let harness = Harness::new([Answer::Status(500)]);
        let (job_id, job_uuid) = harness.job(JobStatus::Processed, &callback()).await;

        harness.deliver(job_id).await;

        let attempts = harness.logged("Callback attempt");
        assert_eq!(attempts.len(), 2, "{attempts:?}");
        for (line, (number, outcome, status)) in
            attempts.iter().zip([(1, "5xx", 500), (2, "2xx", 200)])
        {
            for field in [
                format!("job={job_uuid}"),
                "organization=00000000-0000-0000-0000-000000000001".to_owned(),
                "project=00000000-0000-0000-0000-000000000002".to_owned(),
                "host=receiver.example".to_owned(),
                format!("attempt={number}"),
                format!("outcome={outcome}"),
                format!("status={status}"),
                "duration_ms=0".to_owned(),
            ] {
                assert!(line.contains(&field), "{field} in {line}");
            }
        }
        let logs = harness.logs();
        assert_ne!(logs, Vec::<String>::new());
        for line in logs {
            assert_no_marker(&line);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_startup_fire_delivers_callbacks_on_terminal_jobs_only() {
        let harness = Harness::new([]);
        let (terminal, _) = harness.job(JobStatus::Failed, &callback()).await;
        let (running, _) = harness.job(JobStatus::Running, &callback()).await;

        harness.callbacks.fire_pending(&harness.log).await;
        // A second startup fire, or a terminal site racing it, finds the delivery already running.
        harness.callbacks.fire_pending(&harness.log).await;
        harness.fire(terminal);
        harness.callbacks.drain(&harness.log).await;

        assert_eq!(harness.sent(), 1);
        assert_eq!(
            harness.settled(terminal).await,
            (JobCallbackState::Delivered, 1, Some(200), false)
        );
        assert_eq!(
            harness.settled(running).await,
            (JobCallbackState::Pending, 0, None, true)
        );
    }

    const RECEIVER: &str = "https://receiver.example/hook";

    #[tokio::test(start_paused = true)]
    async fn one_credential_sends_one_request_at_a_time_a_second_apart() {
        let harness = Harness::new([Answer::Slow(Duration::from_secs(5))]);
        let mut jobs = Vec::new();
        for _ in 0..3 {
            let callback = callback_to(RECEIVER, "Bearer token");
            jobs.push(harness.job(JobStatus::Processed, &callback).await);
        }
        let fired = Instant::now();

        for (job_id, _) in &jobs {
            harness.fire(*job_id);
        }
        harness.callbacks.drain(&harness.log).await;

        // The first answer takes 5 s, and each next request waits a second past the one before.
        let sent: Vec<_> = jobs
            .iter()
            .zip([0, 6, 7])
            .map(|((_, job_uuid), seconds)| (fired + Duration::from_secs(seconds), *job_uuid))
            .collect();
        assert_eq!(harness.sent_jobs(), sent);
    }

    #[tokio::test(start_paused = true)]
    async fn a_retry_after_pauses_its_credential_and_no_other() {
        let harness = Harness::new([Answer::Limited(429, retry_after(30))]);
        let (limited, limited_uuid) = harness
            .job(JobStatus::Processed, &callback_to(RECEIVER, "Bearer token"))
            .await;
        let (same, same_uuid) = harness
            .job(JobStatus::Processed, &callback_to(RECEIVER, "Bearer token"))
            .await;
        let (other, other_uuid) = harness
            .job(JobStatus::Processed, &callback_to(RECEIVER, "Bearer other"))
            .await;
        let fired = Instant::now();

        harness.fire(limited);
        harness.until_sent(1).await;
        harness.fire(same);
        harness.fire(other);
        harness.callbacks.drain(&harness.log).await;

        assert_eq!(
            harness.sent_jobs(),
            [
                (fired, limited_uuid),
                (fired, other_uuid),
                (fired + Duration::from_secs(30), same_uuid),
                // The retry waits out the pause too, then a second behind the callback ahead of it.
                (fired + Duration::from_secs(31), limited_uuid),
            ]
        );
        for job_id in [same, other] {
            assert_eq!(
                harness.settled(job_id).await,
                (JobCallbackState::Delivered, 1, Some(200), false)
            );
        }
        assert_eq!(
            harness.settled(limited).await,
            (JobCallbackState::Delivered, 2, Some(200), false)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_403_out_of_requests_retries_at_the_reset_and_any_other_403_is_final() {
        let reset = DateTime::TEST.timestamp() + 600;
        let harness = Harness::new([Answer::Limited(
            403,
            RateLimit {
                retry_after: None,
                remaining: Some(0),
                reset: Some(reset),
            },
        )]);
        let (job_id, _) = harness
            .job(JobStatus::Processed, &callback_to(RECEIVER, "Bearer token"))
            .await;
        let fired = Instant::now();

        harness.deliver(job_id).await;

        assert_eq!(harness.sent_at(), [fired, fired + Duration::from_mins(10)]);
        assert_eq!(
            harness.settled(job_id).await,
            (JobCallbackState::Delivered, 2, Some(200), false)
        );

        for answer in [
            Answer::Status(403),
            Answer::Limited(
                403,
                RateLimit {
                    retry_after: None,
                    remaining: Some(5),
                    reset: Some(reset),
                },
            ),
        ] {
            let harness = Harness::new([answer]);
            let (job_id, _) = harness
                .job(JobStatus::Processed, &callback_to(RECEIVER, "Bearer token"))
                .await;

            harness.deliver(job_id).await;

            assert_eq!(harness.sent(), 1);
            assert_eq!(
                harness.settled(job_id).await,
                (JobCallbackState::Failed, 1, Some(403), false)
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_fallback_pause_doubles_while_limits_repeat_and_a_2xx_resets_it() {
        let harness = Harness::new([
            Answer::Status(429),
            Answer::Status(429),
            Answer::Status(200),
            Answer::Status(429),
        ]);
        let (first, _) = harness
            .job(JobStatus::Processed, &callback_to(RECEIVER, "Bearer token"))
            .await;
        let (second, _) = harness
            .job(JobStatus::Processed, &callback_to(RECEIVER, "Bearer token"))
            .await;
        let fired = Instant::now();

        harness.fire(first);
        tokio::time::sleep(Duration::from_mins(3)).await;
        harness.until_sent(3).await;
        harness.fire(second);
        harness.callbacks.drain(&harness.log).await;

        // A minute, then two; after the 2xx, the second callback's limit pauses a minute again.
        assert_eq!(
            harness.sent_at(),
            [
                fired,
                fired + Duration::from_mins(1),
                fired + Duration::from_mins(3),
                fired + Duration::from_secs(181),
                fired + Duration::from_secs(241),
            ]
        );
        for job_id in [first, second] {
            assert_eq!(harness.settled(job_id).await.0, JobCallbackState::Delivered);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_pause_lasts_an_hour_at_most() {
        let harness = Harness::new([Answer::Limited(429, retry_after(7200))]);
        let (job_id, _) = harness
            .job(JobStatus::Processed, &callback_to(RECEIVER, "Bearer token"))
            .await;
        let fired = Instant::now();

        harness.deliver(job_id).await;

        assert_eq!(harness.sent_at(), [fired, fired + Duration::from_hours(1)]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_host_has_at_most_64_requests_in_flight() {
        let harness =
            Harness::new(iter::repeat_with(|| Answer::Slow(Duration::from_secs(10))).take(65));
        let mut jobs = Vec::new();
        for token in 0..65 {
            let callback = callback_to(RECEIVER, &format!("Bearer token-{token}"));
            jobs.push(harness.job(JobStatus::Processed, &callback).await.0);
        }
        let callback = callback_to("https://other.example/hook", "Bearer token-0");
        jobs.push(harness.job(JobStatus::Processed, &callback).await.0);
        let fired = Instant::now();

        for job_id in jobs {
            harness.fire(job_id);
        }
        harness.callbacks.drain(&harness.log).await;

        let sent = harness.sent_hosts();
        let count = |at: Instant, host: &str| {
            sent.iter()
                .filter(|(sent_at, sent_to)| *sent_at == at && sent_to == host)
                .count()
        };
        let later = fired + Duration::from_secs(10);
        assert_eq!(sent.len(), 66);
        assert_eq!(count(fired, "receiver.example"), 64);
        assert_eq!(count(fired, "other.example"), 1);
        assert_eq!(count(later, "receiver.example"), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn callbacks_waiting_out_a_pause_hold_no_host_slot() {
        let harness = Harness::new(
            iter::once(Answer::Limited(429, retry_after(600)))
                .chain(iter::repeat_with(|| Answer::Slow(Duration::from_secs(10))).take(64)),
        );
        let (limited, _) = harness
            .job(JobStatus::Processed, &callback_to(RECEIVER, "Bearer token"))
            .await;
        let mut others = Vec::new();
        for token in 0..64 {
            let callback = callback_to(RECEIVER, &format!("Bearer other-{token}"));
            others.push(harness.job(JobStatus::Processed, &callback).await.0);
        }

        harness.fire(limited);
        harness.until_sent(1).await;
        // Past the retry's 4 s, so it waits out the pause.
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(5)).await;
        tokio::task::yield_now().await;
        let waited = Instant::now();
        for job_id in others {
            harness.fire(job_id);
        }
        harness.until_sent(65).await;

        assert_eq!(harness.sent_at()[1..], [waited; 64]);
        harness.shutdown.cancel();
        harness.drain_after_shutdown().await;
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_during_a_pause_leaves_every_waiting_callback_pending() {
        let harness = Harness::new([Answer::Limited(429, retry_after(600))]);
        let (limited, _) = harness
            .job(JobStatus::Processed, &callback_to(RECEIVER, "Bearer token"))
            .await;
        let (waiting, _) = harness
            .job(JobStatus::Processed, &callback_to(RECEIVER, "Bearer token"))
            .await;

        harness.fire(limited);
        harness.until_sent(1).await;
        harness.fire(waiting);
        // Past the retry's 4 s, so both wait on the pause.
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(5)).await;
        tokio::task::yield_now().await;
        harness.shutdown.cancel();
        harness.drain_after_shutdown().await;

        assert_eq!(harness.sent(), 1);
        assert_eq!(
            harness.settled(limited).await,
            (JobCallbackState::Pending, 1, Some(429), true)
        );
        assert_eq!(
            harness.settled(waiting).await,
            (JobCallbackState::Pending, 0, None, true)
        );
    }
}
