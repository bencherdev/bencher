use std::{
    collections::{HashSet, VecDeque},
    sync::{Arc, Mutex, PoisonError},
};

use bencher_json::Clock;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use slog::Logger;
use tokio::{
    sync::mpsc::{self, error::TrySendError},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use super::{Database, DbConnection};
use crate::model::project::threshold::changepoint_scan::{Scan, ScanError, WatchedLine};

/// The most reports whose lines the task holds; the lines of more are dropped, and each is
/// scanned again at its next report.
const QUEUE: usize = 1024;

/// Owns the task that scans each line a changepoint model watches after a report touches it.
#[derive(Clone)]
pub struct Changepoints {
    queue: Queue,
    task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

#[derive(Clone)]
struct Queue {
    sender: mpsc::Sender<Vec<WatchedLine>>,
    log: Logger,
}

impl Changepoints {
    /// Start the task, which scans the lines that reports hand it until `shutdown`.
    pub fn spawn(
        log: Logger,
        database: &Database,
        shutdown: CancellationToken,
        clock: Clock,
    ) -> Self {
        let scanner = Scanner {
            connection: Arc::clone(&database.connection),
            reader: database.auth_pool.clone(),
            clock,
            read_line: Arc::new(Scan::read),
        };
        Self::start(log, scanner, shutdown)
    }

    fn start<S: Scans + Send + 'static>(
        log: Logger,
        scanner: S,
        shutdown: CancellationToken,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(QUEUE);
        let task = tokio::spawn(run(scanner, receiver, shutdown, log.clone()));
        Self {
            queue: Queue { sender, log },
            task: Arc::new(Mutex::new(Some(task))),
        }
    }

    /// Give the task the lines of a report that just committed, without waiting on it.
    pub fn queue(&self, lines: Vec<WatchedLine>) {
        let Queue { sender, log } = &self.queue;
        if lines.is_empty() {
            return;
        }
        match sender.try_send(lines) {
            Ok(()) => {},
            Err(TrySendError::Full(lines)) => {
                slog::warn!(log, "Changepoint queue is full, each line is scanned at its next report"; "lines" => lines.len());
            },
            Err(TrySendError::Closed(lines)) => {
                slog::debug!(log, "Changepoint task has stopped"; "lines" => lines.len());
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
            slog::error!(log, "Changepoint task failed: {e}");
        }
    }
}

/// What the task's loop drives, apart from the loop so its queue can be tested alone.
trait Scans {
    /// Scan the line. Once `shutdown` is cancelled, the scan ends at its next step.
    fn scan(
        &mut self,
        log: &Logger,
        line: WatchedLine,
        shutdown: &CancellationToken,
    ) -> impl Future<Output = ()> + Send;
}

/// Scan the lines the queue holds, each once however many reports queued it before its scan
/// starts, in the order each first arrived, until `shutdown`.
async fn run<S: Scans>(
    mut scanner: S,
    mut receiver: mpsc::Receiver<Vec<WatchedLine>>,
    shutdown: CancellationToken,
    log: Logger,
) {
    let mut pending = Pending::default();
    loop {
        if pending.is_empty() {
            let Some(Some(lines)) = shutdown.run_until_cancelled(receiver.recv()).await else {
                return;
            };
            pending.extend(lines);
        }
        // The reports that came in during the last scan join the lines still waiting.
        while let Ok(lines) = receiver.try_recv() {
            pending.extend(lines);
        }
        let Some(line) = pending.next() else {
            continue;
        };
        scanner.scan(&log, line, &shutdown).await;
        if shutdown.is_cancelled() {
            return;
        }
    }
}

/// The lines waiting for a scan, in the order each arrived, each held once. A line leaves when its
/// scan starts, so a report that queues it during its scan has it scanned again.
#[derive(Default)]
struct Pending {
    order: VecDeque<WatchedLine>,
    held: HashSet<WatchedLine>,
}

impl Pending {
    fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    fn extend(&mut self, lines: Vec<WatchedLine>) {
        for line in lines {
            if self.held.insert(line.clone()) {
                self.order.push_back(line);
            }
        }
    }

    fn next(&mut self) -> Option<WatchedLine> {
        let line = self.order.pop_front()?;
        self.held.remove(&line);
        Some(line)
    }
}

/// What the task scans with: the read pool for the line, and the writer for its changepoints.
struct Scanner {
    connection: Arc<tokio::sync::Mutex<DbConnection>>,
    reader: Pool<ConnectionManager<DbConnection>>,
    clock: Clock,
    read_line: ReadLine,
}

/// How a scan reads and analyses its line: [`Scan::read`], apart from the scanner so a test can
/// slow it or fail it.
type ReadLine = Arc<
    dyn Fn(
            &Logger,
            &mut DbConnection,
            &WatchedLine,
            &dyn Fn() -> bool,
        ) -> Result<Option<Scan>, ScanError>
        + Send
        + Sync,
>;

impl Scans for Scanner {
    /// Read and analyse the line on a blocking thread with a connection of the read pool, then
    /// reconcile its changepoints in one short write. Nothing is written after shutdown.
    async fn scan(&mut self, log: &Logger, line: WatchedLine, shutdown: &CancellationToken) {
        let Some(mut conn) = shutdown.run_until_cancelled(self.read()).await else {
            return;
        };
        let read_line = Arc::clone(&self.read_line);
        let scan = tokio::task::spawn_blocking({
            let log = log.clone();
            let line = line.clone();
            let shutdown = shutdown.clone();
            move || read_line(&log, &mut conn, &line, &|| shutdown.is_cancelled())
        })
        .await;
        let scan = match scan {
            Ok(Ok(Some(scan))) => scan,
            Ok(Ok(None)) => return,
            Ok(Err(e)) => {
                slog::error!(log, "Failed to scan a line for changepoints"; "line" => ?line, "error" => %e);
                return;
            },
            Err(e) => {
                slog::error!(log, "Failed to scan a line for changepoints"; "line" => ?line, "error" => %e);
                return;
            },
        };
        let now = self.clock.now();
        let Some(mut writer) = shutdown.run_until_cancelled(self.connection.lock()).await else {
            return;
        };
        let written = writer.immediate_transaction(|conn| scan.write(conn, now));
        if let Err(e) = written {
            slog::error!(log, "Failed to write the changepoints of a line"; "line" => ?line, "error" => %e);
        }
    }
}

impl Scanner {
    /// A read connection, however long the pool stays busy.
    async fn read(&self) -> PooledConnection<ConnectionManager<DbConnection>> {
        loop {
            if let Some(conn) = self.reader.try_get() {
                return conn;
            }
            tokio::time::sleep(self.reader.connection_timeout()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };

    use bencher_json::{Clock, MetricName};
    use diesel::{
        Connection as _,
        connection::SimpleConnection as _,
        r2d2::{ConnectionManager, Pool},
    };
    use pretty_assertions::assert_eq;
    use slog::{Drain as _, Logger};
    use tokio::sync::{
        Semaphore,
        mpsc::{self, error::TryRecvError},
    };
    use tokio_util::sync::CancellationToken;

    use super::{Changepoints, QUEUE, ReadLine, Scanner, Scans, run};
    use crate::{
        context::DbConnection,
        model::project::{
            ProjectId,
            benchmark::BenchmarkId,
            branch::head::HeadId,
            measure::MeasureId,
            testbed::TestbedId,
            threshold::{
                ThresholdId,
                changepoint::{ChangepointLine, ChangepointState, QueryChangepoint},
                changepoint_scan::{
                    Scan, WatchedLine,
                    tests::{Capture, Line},
                },
            },
            variant::VariantId,
        },
        run_migrations,
    };

    fn line(benchmark: i32) -> WatchedLine {
        WatchedLine {
            line: ChangepointLine {
                project_id: ProjectId::from_raw(1),
                head_id: HeadId::from_raw(1),
                testbed_id: TestbedId::from_raw(1),
                spec_id: None,
                benchmark_id: BenchmarkId::from_raw(benchmark),
                variant_id: VariantId::from_raw(benchmark),
                measure_id: MeasureId::from_raw(1),
                metric: MetricName::value(),
            },
            threshold_id: ThresholdId::from_raw(1),
        }
    }

    fn log() -> Logger {
        Logger::root(slog::Discard, slog::o!())
    }

    /// Records each line it scans, and finishes a scan only once a permit is released for it or
    /// shutdown.
    #[derive(Clone)]
    struct Held {
        scanned: Arc<Mutex<Vec<WatchedLine>>>,
        permits: Arc<Semaphore>,
    }

    impl Held {
        fn new(permits: usize) -> Self {
            Self {
                scanned: Arc::new(Mutex::new(Vec::new())),
                permits: Arc::new(Semaphore::new(permits)),
            }
        }

        fn scanned(&self) -> Vec<WatchedLine> {
            self.scanned.lock().expect("a record").clone()
        }
    }

    impl Scans for Held {
        async fn scan(&mut self, _log: &Logger, line: WatchedLine, shutdown: &CancellationToken) {
            self.scanned.lock().expect("a record").push(line);
            if let Some(permit) = shutdown.run_until_cancelled(self.permits.acquire()).await {
                permit.expect("an open semaphore").forget();
            }
        }
    }

    /// Let the loop run until it waits again.
    async fn settle() {
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
    }

    // Kills a queue that scans a line once for each report that queued it before the task reached
    // it, or that never scans a line again after it was scanned.
    #[tokio::test(start_paused = true)]
    async fn a_line_queued_twice_is_scanned_once() {
        let held = Held::new(usize::MAX >> 4);
        let (sender, receiver) = mpsc::channel(QUEUE);
        sender
            .try_send(vec![line(1), line(2)])
            .expect("a queued report");
        sender
            .try_send(vec![line(1), line(3)])
            .expect("a queued report");
        let shutdown = CancellationToken::new();
        let task = tokio::spawn(run(held.clone(), receiver, shutdown.clone(), log()));

        settle().await;
        assert_eq!(held.scanned(), [line(1), line(2), line(3)]);

        // A later report scans the line again.
        sender.try_send(vec![line(1)]).expect("a queued report");
        settle().await;
        assert_eq!(held.scanned(), [line(1), line(2), line(3), line(1)]);

        shutdown.cancel();
        task.await.expect("a task");
    }

    // Kills a line that a report queues again while the task scans the lines before it, and that
    // is then scanned twice with no new data between, the second scan counted as confirming.
    #[tokio::test(start_paused = true)]
    async fn a_line_queued_again_during_a_batch_is_scanned_once() {
        let held = Held::new(0);
        let shutdown = CancellationToken::new();
        let changepoints = Changepoints::start(log(), held.clone(), shutdown.clone());

        changepoints.queue(vec![line(1), line(2)]);
        settle().await;
        assert_eq!(held.scanned(), [line(1)]);
        // The task holds line 1, and a second report queues line 2 again.
        changepoints.queue(vec![line(2)]);
        settle().await;

        held.permits.add_permits(4);
        settle().await;
        assert_eq!(held.scanned(), [line(1), line(2)]);

        shutdown.cancel();
        changepoints.drain(&log()).await;
    }

    // Kills a line that a report queues during its own scan, after the scan read the line, and that
    // is never scanned again for the new data.
    #[tokio::test(start_paused = true)]
    async fn a_line_queued_during_its_own_scan_is_scanned_again() {
        let held = Held::new(0);
        let shutdown = CancellationToken::new();
        let changepoints = Changepoints::start(log(), held.clone(), shutdown.clone());

        changepoints.queue(vec![line(1)]);
        settle().await;
        changepoints.queue(vec![line(1)]);
        settle().await;

        held.permits.add_permits(4);
        settle().await;
        assert_eq!(held.scanned(), [line(1), line(1)]);

        shutdown.cancel();
        changepoints.drain(&log()).await;
    }

    // Kills a report that waits on the scan of its lines, and lines handed over during a scan that
    // are never scanned.
    #[tokio::test(start_paused = true)]
    async fn a_report_hands_its_lines_over_without_waiting() {
        let held = Held::new(0);
        let shutdown = CancellationToken::new();
        let changepoints = Changepoints::start(log(), held.clone(), shutdown.clone());

        changepoints.queue(vec![line(1)]);
        settle().await;
        assert_eq!(held.scanned(), [line(1)]);
        // The scan of the first report's line is held, and the second report still hands its
        // lines over at once.
        changepoints.queue(vec![line(2)]);
        settle().await;
        assert_eq!(held.scanned(), [line(1)]);

        held.permits.add_permits(2);
        settle().await;
        assert_eq!(held.scanned(), [line(1), line(2)]);

        shutdown.cancel();
        changepoints.drain(&log()).await;
    }

    // Kills a task that goes on to the next line after shutdown, which would hold up the server's
    // shutdown, and a queue that fails after the task stops.
    #[tokio::test(start_paused = true)]
    async fn shutdown_drains_the_task() {
        let held = Held::new(0);
        let shutdown = CancellationToken::new();
        let changepoints = Changepoints::start(log(), held.clone(), shutdown.clone());
        changepoints.queue(vec![line(1), line(2)]);
        settle().await;
        assert_eq!(held.scanned(), [line(1)]);

        shutdown.cancel();
        tokio::time::timeout(Duration::from_secs(1), changepoints.drain(&log()))
            .await
            .expect("The drain waited out the scan");
        assert_eq!(held.scanned(), [line(1)]);
        changepoints.queue(vec![line(3)]);
        changepoints.drain(&log()).await;
    }

    /// A line with a step on a database file, so a scanner's own read pool sees it.
    struct Stepped {
        /// Holds the database, so removing it removes the WAL files too, whatever closes last.
        _dir: tempfile::TempDir,
        path: String,
        connection: Arc<tokio::sync::Mutex<DbConnection>>,
        watched: WatchedLine,
    }

    impl Stepped {
        /// A step from 100 to 110 at version 20 of 30, under a model that guards both sides, so a
        /// scan runs 2 passes and finds a candidate.
        fn new() -> Self {
            let dir = tempfile::TempDir::new().unwrap();
            let path = dir
                .path()
                .join("changepoints.db")
                .to_str()
                .unwrap()
                .to_owned();
            let mut conn = DbConnection::establish(&path).unwrap();
            conn.batch_execute(
                "PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL",
            )
            .unwrap();
            run_migrations(&mut conn).unwrap();
            let mut line = Line::on(conn, log());
            line.set_model(Some(0.05), Some(0.05), None);
            line.level(100.0, 20);
            line.level(110.0, 10);
            let watched = line.watched();
            Self {
                _dir: dir,
                path,
                connection: Arc::new(tokio::sync::Mutex::new(line.into_conn())),
                watched,
            }
        }

        fn scanner(&self, read_line: ReadLine) -> Scanner {
            Scanner {
                connection: Arc::clone(&self.connection),
                reader: Pool::builder()
                    .max_size(1)
                    .build(ConnectionManager::<DbConnection>::new(&self.path))
                    .unwrap(),
                clock: Clock::System,
                read_line,
            }
        }

        /// The statuses of the line's changepoints.
        fn statuses(&self) -> Vec<ChangepointState> {
            let mut conn = DbConnection::establish(&self.path).unwrap();
            QueryChangepoint::on_line(&mut conn, &self.watched.line)
                .unwrap()
                .into_iter()
                .map(|changepoint| changepoint.status)
                .collect()
        }
    }

    // Kills a scan that reads and analyses its line on the task's own thread, which holds up every
    // other task of the runtime for as long as a long line takes.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_slow_scan_leaves_the_runtime_to_its_other_tasks() {
        let stepped = Stepped::new();
        let ticks = Arc::new(AtomicUsize::new(0));
        let timer = tokio::spawn({
            let ticks = Arc::clone(&ticks);
            async move {
                let mut interval = tokio::time::interval(Duration::from_millis(5));
                loop {
                    interval.tick().await;
                    ticks.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        let (ticked, mut ticked_during) = mpsc::channel(1);
        // The read is slow until the timer ticks 3 times, or for 5 s if the read holds the runtime's
        // only thread, where no task can move the clock.
        let read_line: ReadLine = Arc::new(move |log, conn, line, stopped| {
            let start = ticks.load(Ordering::SeqCst);
            let deadline = Instant::now() + Duration::from_secs(5);
            while ticks.load(Ordering::SeqCst) < start.saturating_add(3)
                && Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(1));
            }
            let during = ticks.load(Ordering::SeqCst).saturating_sub(start);
            ticked.try_send(during).unwrap();
            Scan::read(log, conn, line, stopped)
        });
        let shutdown = CancellationToken::new();
        let changepoints = Changepoints::start(log(), stepped.scanner(read_line), shutdown.clone());

        changepoints.queue(vec![stepped.watched.clone()]);
        // A blocking task holds the paused clock, so the test moves it for the timer.
        let during = loop {
            match ticked_during.try_recv() {
                Ok(during) => break during,
                Err(TryRecvError::Empty) => tokio::time::advance(Duration::from_millis(5)).await,
                Err(TryRecvError::Disconnected) => panic!("the scan ended before its read"),
            }
        };
        shutdown.cancel();
        changepoints.drain(&log()).await;
        timer.abort();
        assert!(timer.await.unwrap_err().is_cancelled(), "the timer stopped");
        assert!(
            during >= 3,
            "the timer ticked {during} times during the scan"
        );
    }

    // Kills a shutdown that waits out the whole scan, or that cuts the task while its blocking read
    // goes on alone.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn shutdown_during_a_scan_ends_it_at_its_next_step_with_nothing_written() {
        let stepped = Stepped::new();
        let shutdown = CancellationToken::new();
        let checks = Arc::new(AtomicUsize::new(0));
        let (started, mut read_started) = mpsc::channel(1);
        // The read starts, waits for shutdown, then reads the line and counts each check of the
        // shutdown token.
        let read_line: ReadLine = Arc::new({
            let shutdown = shutdown.clone();
            let checks = Arc::clone(&checks);
            move |log, conn, line, stopped| {
                started.try_send(()).unwrap();
                while !shutdown.is_cancelled() {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Scan::read(log, conn, line, &|| {
                    checks.fetch_add(1, Ordering::SeqCst);
                    stopped()
                })
            }
        });
        let changepoints = Changepoints::start(log(), stepped.scanner(read_line), shutdown.clone());

        changepoints.queue(vec![stepped.watched.clone()]);
        read_started.recv().await.unwrap();
        shutdown.cancel();
        // The paused clock holds while the scan runs, so this bound counts only an idle drain.
        tokio::time::timeout(Duration::from_secs(5), changepoints.drain(&log()))
            .await
            .expect("The drain waited on an idle task");
        // The scan stopped at its first check, before its first pass.
        assert_eq!(checks.load(Ordering::SeqCst), 1);
        assert_eq!(stepped.statuses(), []);
    }

    // Kills a scan that waits out a busy read pool after shutdown, which holds up the server's
    // shutdown for as long as the pool stays busy.
    #[tokio::test(start_paused = true)]
    async fn shutdown_ends_a_scan_that_waits_for_a_read_connection() {
        let stepped = Stepped::new();
        let scanner = stepped.scanner(Arc::new(Scan::read));
        // The pool's only connection stays busy until the drain is done.
        let busy = scanner.reader.get().unwrap();
        let shutdown = CancellationToken::new();
        let changepoints = Changepoints::start(log(), scanner, shutdown.clone());

        changepoints.queue(vec![stepped.watched.clone()]);
        settle().await;
        shutdown.cancel();
        tokio::time::timeout(Duration::from_secs(1), changepoints.drain(&log()))
            .await
            .expect("The drain waited for a read connection");
        drop(busy);
    }

    // Kills a scan that writes what it found after a shutdown that came during its last step.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn shutdown_after_the_analysis_writes_nothing() {
        let stepped = Stepped::new();
        let shutdown = CancellationToken::new();
        let (read, mut scan_read) = mpsc::channel(1);
        let read_line: ReadLine = Arc::new({
            let shutdown = shutdown.clone();
            move |log, conn, line, stopped| {
                let scan = Scan::read(log, conn, line, stopped);
                shutdown.cancel();
                read.try_send(scan.as_ref().is_ok_and(Option::is_some))
                    .unwrap();
                scan
            }
        });
        let changepoints = Changepoints::start(log(), stepped.scanner(read_line), shutdown.clone());

        changepoints.queue(vec![stepped.watched.clone()]);
        assert!(scan_read.recv().await.unwrap(), "the scan found the step");
        changepoints.drain(&log()).await;
        assert_eq!(stepped.statuses(), []);
    }

    // Kills a panic in a scan that ends the task, so no later line is scanned, and one that is not
    // logged.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_scan_that_panics_is_logged_and_the_next_line_is_scanned() {
        let stepped = Stepped::new();
        let mut panics = stepped.watched.clone();
        panics.threshold_id = ThresholdId::from_raw(i32::MAX);
        let (read, mut scan_read) = mpsc::channel(1);
        let read_line: ReadLine = Arc::new({
            let panics = panics.clone();
            move |log, conn, line, stopped| {
                assert_ne!(*line, panics, "a scan that panics");
                let scan = Scan::read(log, conn, line, stopped);
                read.try_send(scan.as_ref().is_ok_and(Option::is_some))
                    .unwrap();
                scan
            }
        });
        let capture = Capture::default();
        let logger = Logger::root(capture.clone().fuse(), slog::o!());
        let shutdown = CancellationToken::new();
        let changepoints =
            Changepoints::start(logger, stepped.scanner(read_line), shutdown.clone());

        changepoints.queue(vec![panics, stepped.watched.clone()]);
        // The panic drops its read connection as broken, and the next scan waits for the pool to
        // open another on a thread that does not hold the paused clock, so this wait has no
        // deadline. Once the read is done, the write follows with no idle time.
        assert_eq!(
            scan_read.recv().await,
            Some(true),
            "the next line was scanned"
        );
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while stepped.statuses().is_empty() && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        shutdown.cancel();
        changepoints.drain(&log()).await;
        assert_eq!(stepped.statuses(), [ChangepointState::Candidate]);
        let lines = capture.lines();
        assert!(
            lines.iter().any(
                |line| line.starts_with("Failed to scan a line for changepoints")
                    && line.contains("a scan that panics")
            ),
            "{lines:?}"
        );
    }
}
