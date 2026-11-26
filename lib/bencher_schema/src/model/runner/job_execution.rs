//! An execution is 1 run of a job's image that the server sent to a runner.
//!
//! An iteration of a job is 1 execution. The server sends a job's executions one at a time, and
//! the next only once none is in flight, so its iterations may run on different runners, one after
//! another, and between other jobs' executions.

use bencher_json::{DateTime, JobStatus, JobUuid, Timeout};
use diesel::{
    BoolExpressionMethods as _, Connection as _, ExpressionMethods as _, QueryDsl as _,
    RunQueryDsl as _, result::QueryResult,
};
use slog::Logger;

use crate::{
    context::{ApiContext, DbConnection},
    model::{
        project::report::ReportId,
        runner::{JobId, QueryJob, RunnerId, UpdateJob},
    },
    schema::{self, job_execution as job_execution_table},
    write_conn,
};

crate::macros::typed_id::typed_id!(JobExecutionId);

/// Where an execution stands.
///
/// An execution in flight is `Sent`, `Running`, or `Unknown`: its result may still arrive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, diesel::FromSqlRow, diesel::AsExpression)]
#[diesel(sql_type = diesel::sql_types::Integer)]
pub enum ExecutionStatus {
    /// Sent to its runner, which has not yet reported it running.
    Sent = 0,
    /// Its runner reported it running.
    Running = 1,
    /// It ran, and its output is stored.
    Completed = 2,
    /// It failed: a `Failed` message, a non-zero exit, or a run with no result.
    Failed = 3,
    /// Its job ended while it was in flight, or its runner acknowledged a cancel.
    Canceled = 4,
    /// Its runner went silent. Its result is still taken until its budget and the grace period
    /// run out.
    Unknown = 5,
}

impl ExecutionStatus {
    pub const IN_FLIGHT: [Self; 3] = [Self::Sent, Self::Running, Self::Unknown];

    pub fn is_in_flight(self) -> bool {
        Self::IN_FLIGHT.contains(&self)
    }
}

impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for ExecutionStatus
where
    DB: diesel::backend::Backend,
    i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        match self {
            Self::Sent => 0.to_sql(out),
            Self::Running => 1.to_sql(out),
            Self::Completed => 2.to_sql(out),
            Self::Failed => 3.to_sql(out),
            Self::Canceled => 4.to_sql(out),
            Self::Unknown => 5.to_sql(out),
        }
    }
}

impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for ExecutionStatus
where
    DB: diesel::backend::Backend,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        match i32::from_sql(bytes)? {
            0 => Ok(Self::Sent),
            1 => Ok(Self::Running),
            2 => Ok(Self::Completed),
            3 => Ok(Self::Failed),
            4 => Ok(Self::Canceled),
            5 => Ok(Self::Unknown),
            value => Err(format!("Invalid execution status: {value}").into()),
        }
    }
}

#[derive(Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Selectable)]
#[diesel(table_name = job_execution_table)]
pub struct QueryJobExecution {
    pub id: JobExecutionId,
    /// The UUID the runner echoes in its result, in the job's slot of the message.
    pub uuid: JobUuid,
    pub job_id: JobId,
    pub report_id: ReportId,
    pub iteration: i32,
    pub runner_id: RunnerId,
    /// The budget it was sent with: what was left of its job's timeout.
    pub timeout: Timeout,
    pub status: ExecutionStatus,
    pub sent: DateTime,
    pub started: Option<DateTime>,
    pub ended: Option<DateTime>,
    pub last_heartbeat: DateTime,
}

impl QueryJobExecution {
    pub fn get(conn: &mut DbConnection, id: JobExecutionId) -> QueryResult<Self> {
        schema::job_execution::table
            .filter(schema::job_execution::id.eq(id))
            .first(conn)
    }

    pub fn from_uuid(conn: &mut DbConnection, uuid: JobUuid) -> QueryResult<Option<Self>> {
        use diesel::OptionalExtension as _;
        schema::job_execution::table
            .filter(schema::job_execution::uuid.eq(uuid))
            .first(conn)
            .optional()
    }

    /// Every execution of a job, in iteration order.
    pub fn of_job(conn: &mut DbConnection, job_id: JobId) -> QueryResult<Vec<Self>> {
        schema::job_execution::table
            .filter(schema::job_execution::job_id.eq(job_id))
            .order(schema::job_execution::iteration.asc())
            .load(conn)
    }

    /// Seconds of its job's time it has used: from its `Running` to its result, or to `now`
    /// while it is in flight.
    pub fn elapsed_secs(&self, now: DateTime) -> i64 {
        self.started.map_or(0, |started| {
            (self.ended.unwrap_or(now).timestamp() - started.timestamp()).max(0)
        })
    }

    /// Whether it holds every iteration of its job: a job an older server sent whole, under its own
    /// UUID.
    pub fn is_whole_job(&self, job: &QueryJob) -> bool {
        self.uuid == job.uuid
    }

    /// Whether its budget and the grace period have run out, counted from its start, or from
    /// when it was sent if it never started.
    pub fn is_past_deadline(&self, now: DateTime, grace_period: std::time::Duration) -> bool {
        self.until_deadline(now, grace_period).is_none()
    }

    /// Time left until it is past its deadline, or `None` once it is.
    pub fn until_deadline(
        &self,
        now: DateTime,
        grace_period: std::time::Duration,
    ) -> Option<std::time::Duration> {
        let start = self.started.unwrap_or(self.sent);
        let elapsed = (now.timestamp() - start.timestamp()).max(0);
        let limit = timeout_limit(self.timeout, grace_period);
        u64::try_from(limit - elapsed)
            .ok()
            .map(std::time::Duration::from_secs)
    }
}

/// A timeout plus the grace period, in seconds.
pub fn timeout_limit(timeout: Timeout, grace_period: std::time::Duration) -> i64 {
    i64::from(u32::from(timeout))
        .saturating_add(i64::try_from(grace_period.as_secs()).unwrap_or(i64::MAX))
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = job_execution_table)]
pub struct InsertJobExecution {
    pub uuid: JobUuid,
    pub job_id: JobId,
    pub report_id: ReportId,
    pub iteration: i32,
    pub runner_id: RunnerId,
    pub timeout: Timeout,
    pub status: ExecutionStatus,
    pub sent: DateTime,
    pub started: Option<DateTime>,
    pub last_heartbeat: DateTime,
}

#[derive(Debug, Default, diesel::AsChangeset)]
#[diesel(table_name = job_execution_table)]
pub struct UpdateJobExecution {
    pub status: Option<ExecutionStatus>,
    pub started: Option<Option<DateTime>>,
    pub ended: Option<Option<DateTime>>,
    pub last_heartbeat: Option<DateTime>,
}

impl UpdateJobExecution {
    pub fn start(now: DateTime) -> Self {
        Self {
            status: Some(ExecutionStatus::Running),
            started: Some(Some(now)),
            last_heartbeat: Some(now),
            ..Default::default()
        }
    }

    pub fn heartbeat(now: DateTime) -> Self {
        Self {
            last_heartbeat: Some(now),
            ..Default::default()
        }
    }

    pub fn end(status: ExecutionStatus, now: DateTime) -> Self {
        debug_assert!(!status.is_in_flight(), "end() called with {status:?}");
        Self {
            status: Some(status),
            ended: Some(Some(now)),
            ..Default::default()
        }
    }

    pub fn set_status(status: ExecutionStatus) -> Self {
        Self {
            status: Some(status),
            ..Default::default()
        }
    }

    /// Apply this changeset if the execution is in one of `expected`, and return the rows updated.
    pub fn execute_if_any_status(
        &self,
        conn: &mut DbConnection,
        id: JobExecutionId,
        expected: &[ExecutionStatus],
    ) -> QueryResult<usize> {
        diesel::update(
            schema::job_execution::table
                .filter(schema::job_execution::id.eq(id))
                .filter(schema::job_execution::status.eq_any(expected)),
        )
        .set(self)
        .execute(conn)
    }
}

/// A job and its executions, read together.
#[derive(Debug, Clone)]
pub struct JobExecutions {
    pub job: QueryJob,
    pub executions: Vec<QueryJobExecution>,
}

/// What a job with nothing in flight does next.
#[derive(Debug, PartialEq, Eq)]
pub enum JobStep {
    /// It waits: for a result, for a runner to take its next iteration, or for nothing at all.
    Wait,
    /// Every iteration has its outcome.
    Complete,
    /// It has iterations left and no time to run them.
    OutOfTime(String),
    /// An execution failed without `allow_failure`, and the job has not yet failed.
    Fail(String),
}

impl JobExecutions {
    pub fn get(conn: &mut DbConnection, job_id: JobId) -> QueryResult<Self> {
        Ok(Self {
            job: schema::job::table
                .filter(schema::job::id.eq(job_id))
                .first(conn)?,
            executions: QueryJobExecution::of_job(conn, job_id)?,
        })
    }

    /// How many iterations the job runs: its `iter`, and at least 1, so a job is always sent.
    pub fn iterations(&self) -> usize {
        self.job
            .config
            .iter
            .map_or(1, bencher_json::Iteration::as_usize)
            .max(1)
    }

    /// Seconds of the job's timeout its executions have used, finished and in flight.
    pub fn used_secs(&self, now: DateTime) -> i64 {
        self.executions
            .iter()
            .map(|execution| execution.elapsed_secs(now))
            .sum()
    }

    /// Seconds left of the job's timeout, which is the budget of its next execution.
    pub fn budget_secs(&self, now: DateTime) -> i64 {
        i64::from(u32::from(self.job.timeout)) - self.used_secs(now)
    }

    /// Whether the job's executions have run past its timeout and the grace period.
    pub fn is_past_deadline(&self, now: DateTime, grace_period: std::time::Duration) -> bool {
        self.used_secs(now) > timeout_limit(self.job.timeout, grace_period)
    }

    fn has_in_flight(&self) -> bool {
        self.executions
            .iter()
            .any(|execution| execution.status.is_in_flight())
    }

    fn allow_failure(&self) -> bool {
        self.job.config.allow_failure.unwrap_or_default()
    }

    pub fn step(&self, now: DateTime) -> JobStep {
        if !is_in_progress(self.job.status) || self.executions.is_empty() || self.has_in_flight() {
            return JobStep::Wait;
        }
        if !self.allow_failure()
            && let Some(failed) = self
                .executions
                .iter()
                .find(|execution| execution.status == ExecutionStatus::Failed)
        {
            return JobStep::Fail(format!("Iteration {} failed", failed.iteration + 1));
        }
        let iterations = self.iterations();
        let ran = self.executions.len();
        if ran >= iterations
            || self
                .executions
                .iter()
                .any(|execution| execution.is_whole_job(&self.job))
        {
            return JobStep::Complete;
        }
        if self.budget_secs(now) <= 0 {
            return JobStep::OutOfTime(format!(
                "Job timed out after {}s, with {ran} of {iterations} iterations run",
                u32::from(self.job.timeout)
            ));
        }
        JobStep::Wait
    }

    /// The status of a job in progress: `Unknown` while an execution is silent and none other is
    /// in flight, as a job of 1 execution has always been; `Running` once one has started; and
    /// `Claimed` before that.
    fn progress_status(&self) -> JobStatus {
        let unknown = self
            .executions
            .iter()
            .any(|execution| execution.status == ExecutionStatus::Unknown);
        let live = self.executions.iter().any(|execution| {
            matches!(
                execution.status,
                ExecutionStatus::Sent | ExecutionStatus::Running
            )
        });
        if unknown && !live {
            JobStatus::Unknown
        } else if self.job.started.is_some() {
            JobStatus::Running
        } else {
            JobStatus::Claimed
        }
    }

    /// The start and end of the job's report, and the seconds of its executions: the first
    /// execution's start, the last execution's end, and their sum. A job with no execution, which
    /// an older server ran whole, keeps its own start and ends at `now`.
    pub fn clock(&self, now: DateTime) -> (DateTime, DateTime, i64) {
        let start = self
            .executions
            .iter()
            .filter_map(|execution| execution.started)
            .min_by_key(DateTime::timestamp);
        let end = self
            .executions
            .iter()
            .filter_map(|execution| execution.ended)
            .max_by_key(DateTime::timestamp);
        if let (Some(start), Some(end)) = (start, end) {
            return (start, end, self.used_secs(now));
        }
        let start = self.job.started.unwrap_or(now);
        (start, now, (now.timestamp() - start.timestamp()).max(0))
    }
}

/// A job that has been sent and has no outcome yet.
pub fn is_in_progress(status: JobStatus) -> bool {
    matches!(
        status,
        JobStatus::Claimed | JobStatus::Running | JobStatus::Unknown
    )
}

/// [`end_job`] under the write lock, at the time the lock is taken, so a heartbeat that held the
/// lock never billed the job to a later time than its executions end.
pub async fn end_job_now(
    context: &ApiContext,
    job_id: JobId,
    status: JobStatus,
) -> QueryResult<bool> {
    let conn = write_conn!(context);
    end_job(conn, job_id, status, context.clock.now())
}

/// Give a job its outcome, if it has none yet, and end its executions in flight, whose results
/// are then discarded as late. Returns whether this call ended the job.
pub fn end_job(
    conn: &mut DbConnection,
    job_id: JobId,
    status: JobStatus,
    now: DateTime,
) -> QueryResult<bool> {
    conn.transaction(|conn| {
        let updated = UpdateJob::terminate(status, now).execute_if_any_status(
            conn,
            job_id,
            &[JobStatus::Claimed, JobStatus::Running, JobStatus::Unknown],
        )?;
        diesel::update(
            schema::job_execution::table
                .filter(schema::job_execution::job_id.eq(job_id))
                .filter(schema::job_execution::status.eq_any(ExecutionStatus::IN_FLIGHT)),
        )
        .set(UpdateJobExecution::end(ExecutionStatus::Canceled, now))
        .execute(conn)?;
        Ok(updated > 0)
    })
}

/// Bring a job in progress to the status its executions give it.
pub fn sync_job_status(conn: &mut DbConnection, job_id: JobId, now: DateTime) -> QueryResult<()> {
    let job_executions = JobExecutions::get(conn, job_id)?;
    let current = job_executions.job.status;
    if !is_in_progress(current) {
        return Ok(());
    }
    let target = job_executions.progress_status();
    if target != current {
        UpdateJob::set_status(target, now).execute_if_status(conn, job_id, current)?;
    }
    Ok(())
}

/// Every execution in flight, which startup recovery arms a heartbeat timeout for.
///
/// Written as `OR`, not `eq_any`: a bound `IN` list never matches a partial index.
pub fn in_flight_executions(conn: &mut DbConnection) -> QueryResult<Vec<QueryJobExecution>> {
    schema::job_execution::table
        .filter(
            schema::job_execution::status
                .eq(ExecutionStatus::Sent)
                .or(schema::job_execution::status.eq(ExecutionStatus::Running))
                .or(schema::job_execution::status.eq(ExecutionStatus::Unknown)),
        )
        .load(conn)
}

/// Every job in progress with nothing in flight: between its executions, or done with them.
pub fn jobs_between_executions(conn: &mut DbConnection) -> QueryResult<Vec<JobId>> {
    use diesel::dsl::{exists, not};

    schema::job::table
        .filter(
            schema::job::status
                .eq(JobStatus::Claimed)
                .or(schema::job::status.eq(JobStatus::Running)),
        )
        .filter(exists(
            schema::job_execution::table.filter(schema::job_execution::job_id.eq(schema::job::id)),
        ))
        .filter(not(exists(
            schema::job_execution::table
                .filter(schema::job_execution::job_id.eq(schema::job::id))
                .filter(
                    schema::job_execution::status
                        .eq(ExecutionStatus::Sent)
                        .or(schema::job_execution::status.eq(ExecutionStatus::Running))
                        .or(schema::job_execution::status.eq(ExecutionStatus::Unknown)),
                ),
        )))
        .select(schema::job::id)
        .load(conn)
}

/// Mark executions that were sent and never reported running as `Unknown`.
///
/// Such an execution was sent longer ago than the heartbeat timeout: its runner took it and went
/// silent. An `Unknown` execution still takes its runner's result, and its deadline ends its job
/// otherwise.
///
/// Returns the number of executions marked `Unknown`.
pub fn mark_orphaned_sent_executions_unknown(
    log: &Logger,
    conn: &mut DbConnection,
    heartbeat_timeout: std::time::Duration,
    clock: &bencher_json::Clock,
) -> usize {
    let now = clock.now();
    let cutoff =
        now - chrono::Duration::from_std(heartbeat_timeout).unwrap_or(chrono::Duration::MAX);

    let orphaned: Vec<QueryJobExecution> = match schema::job_execution::table
        .filter(schema::job_execution::status.eq(ExecutionStatus::Sent))
        .filter(schema::job_execution::sent.le(cutoff))
        .load(conn)
    {
        Ok(executions) => executions,
        Err(e) => {
            slog::error!(log, "Failed to query orphaned sent executions"; "error" => %e);
            return 0;
        },
    };

    let mut marked = 0;
    for execution in &orphaned {
        slog::warn!(log, "Marking orphaned sent execution as unknown"; "execution_id" => ?execution.id, "job_id" => ?execution.job_id);
        let updated = UpdateJobExecution::set_status(ExecutionStatus::Unknown)
            .execute_if_any_status(conn, execution.id, &[ExecutionStatus::Sent])
            .and_then(|updated| {
                sync_job_status(conn, execution.job_id, now)?;
                Ok(updated)
            });
        match updated {
            Ok(0) => {
                slog::info!(log, "Orphaned execution already changed state"; "execution_id" => ?execution.id);
            },
            Ok(_) => {
                marked += 1;
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
                    bencher_otel::JobStatusKind::Unknown,
                ));
            },
            Err(e) => {
                slog::error!(log, "Failed to mark orphaned execution as unknown"; "execution_id" => ?execution.id, "error" => %e);
            },
        }
    }

    if marked > 0 {
        slog::info!(log, "Marked orphaned sent executions as unknown"; "count" => marked);
    }
    marked
}

#[cfg(test)]
mod tests {
    use bencher_json::{JobUuid, Priority};
    use diesel::connection::SimpleConnection as _;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        model::{organization::OrganizationId, project::ProjectId, runner::SourceIp, spec::SpecId},
        test_util::{create_job, create_job_fixture, setup_test_db},
    };

    /// `DateTime::TEST` plus `secs`.
    fn at(secs: i64) -> DateTime {
        DateTime::try_from(DateTime::TEST.timestamp() + secs).unwrap()
    }

    fn job(timeout: u32, iter: u32, allow_failure: bool) -> QueryJob {
        QueryJob {
            id: JobId::try_from_raw(1).unwrap(),
            uuid: JobUuid::new(),
            organization_id: OrganizationId::try_from_raw(1).unwrap(),
            project_id: ProjectId::try_from_raw(1).unwrap(),
            report_id: ReportId::try_from_raw(1).unwrap(),
            source_ip: SourceIp::new(std::net::Ipv4Addr::LOCALHOST.into()),
            spec_id: SpecId::try_from_raw(1).unwrap(),
            config: serde_json::from_value(serde_json::json!({
                "registry": "https://registry.example.com",
                "project": "00000000-0000-0000-0000-000000000000",
                "digest": format!("sha256:{}", "0".repeat(64)),
                "timeout": timeout,
                "iter": iter,
                "allow_failure": allow_failure,
            }))
            .unwrap(),
            timeout: Timeout::try_from(timeout).unwrap(),
            priority: Priority::Plus,
            status: JobStatus::Running,
            runner_id: Some(RunnerId::try_from_raw(1).unwrap()),
            claimed: Some(DateTime::TEST),
            started: Some(DateTime::TEST),
            completed: None,
            last_heartbeat: Some(DateTime::TEST),
            last_billed_minute: None,
            created: DateTime::TEST,
            modified: DateTime::TEST,
        }
    }

    /// An execution of `iteration` that started at `started` and ended at `ended`, in seconds.
    fn execution(
        iteration: i32,
        status: ExecutionStatus,
        started: Option<i64>,
        ended: Option<i64>,
    ) -> QueryJobExecution {
        QueryJobExecution {
            id: JobExecutionId::try_from_raw(iteration + 1).unwrap(),
            uuid: JobUuid::new(),
            job_id: JobId::try_from_raw(1).unwrap(),
            report_id: ReportId::try_from_raw(1).unwrap(),
            iteration,
            runner_id: RunnerId::try_from_raw(1).unwrap(),
            timeout: Timeout::try_from(100).unwrap(),
            status,
            sent: at(started.unwrap_or_default()),
            started: started.map(at),
            ended: ended.map(at),
            last_heartbeat: at(started.unwrap_or_default()),
        }
    }

    fn job_executions(job: QueryJob, executions: Vec<QueryJobExecution>) -> JobExecutions {
        JobExecutions { job, executions }
    }

    #[test]
    fn the_job_counts_only_the_time_inside_its_executions() {
        let job_executions = job_executions(
            job(100, 3, false),
            vec![
                execution(0, ExecutionStatus::Completed, Some(0), Some(20)),
                // 100 seconds of another job's executions between them
                execution(1, ExecutionStatus::Completed, Some(120), Some(140)),
                execution(2, ExecutionStatus::Running, Some(200), None),
            ],
        );
        assert_eq!(job_executions.used_secs(at(205)), 45);
        assert_eq!(job_executions.budget_secs(at(205)), 55);
        assert!(!job_executions.is_past_deadline(at(205), std::time::Duration::from_mins(1)));
        // 40 seconds of finished executions, and 121 of the one in flight, pass 100 plus 60.
        assert!(job_executions.is_past_deadline(at(321), std::time::Duration::from_mins(1)));
    }

    #[test]
    fn a_job_with_nothing_in_flight_takes_its_next_step() {
        let completed =
            |iteration| execution(iteration, ExecutionStatus::Completed, Some(0), Some(10));
        let failed = |iteration| execution(iteration, ExecutionStatus::Failed, Some(0), Some(10));
        for (job, executions, now, step) in [
            (job(100, 2, false), vec![completed(0)], 10, JobStep::Wait),
            (
                job(100, 2, false),
                vec![
                    completed(0),
                    execution(1, ExecutionStatus::Running, Some(10), None),
                ],
                10,
                JobStep::Wait,
            ),
            (
                job(100, 2, false),
                vec![completed(0), completed(1)],
                20,
                JobStep::Complete,
            ),
            (
                job(15, 3, false),
                vec![completed(0), completed(1)],
                20,
                JobStep::OutOfTime(
                    "Job timed out after 15s, with 2 of 3 iterations run".to_owned(),
                ),
            ),
            (
                job(100, 2, false),
                vec![failed(0)],
                10,
                JobStep::Fail("Iteration 1 failed".to_owned()),
            ),
            (job(100, 2, true), vec![failed(0)], 10, JobStep::Wait),
            (
                job(100, 2, true),
                vec![failed(0), completed(1)],
                20,
                JobStep::Complete,
            ),
        ] {
            let job_executions = job_executions(job, executions);
            assert_eq!(job_executions.step(at(now)), step, "{job_executions:?}");
        }
    }

    /// A job an older server sent whole completes with its 1 execution, whatever its `iter`.
    #[test]
    fn a_job_sent_whole_completes_with_its_one_execution() {
        let job = job(100, 3, false);
        let mut whole = execution(0, ExecutionStatus::Completed, Some(0), Some(10));
        whole.uuid = job.uuid;
        assert_eq!(
            job_executions(job, vec![whole]).step(at(10)),
            JobStep::Complete
        );
    }

    #[test]
    fn a_job_is_unknown_only_while_nothing_else_is_live() {
        let unknown = execution(0, ExecutionStatus::Unknown, Some(0), None);
        let running = execution(1, ExecutionStatus::Running, Some(0), None);
        let completed = execution(1, ExecutionStatus::Completed, Some(0), Some(10));
        assert_eq!(
            job_executions(job(100, 2, false), vec![unknown.clone(), running]).progress_status(),
            JobStatus::Running
        );
        assert_eq!(
            job_executions(job(100, 2, false), vec![unknown, completed.clone()]).progress_status(),
            JobStatus::Unknown
        );
        assert_eq!(
            job_executions(job(100, 2, false), vec![completed]).progress_status(),
            JobStatus::Running
        );
    }

    #[test]
    fn the_report_spans_the_first_start_to_the_last_end() {
        let job_executions = job_executions(
            job(100, 2, false),
            vec![
                execution(0, ExecutionStatus::Completed, Some(30), Some(50)),
                execution(1, ExecutionStatus::Completed, Some(10), Some(40)),
            ],
        );
        let (start, end, secs) = job_executions.clock(at(1000));
        assert_eq!(
            (start.timestamp(), end.timestamp(), secs),
            (at(10).timestamp(), at(50).timestamp(), 50)
        );
    }

    #[test]
    fn a_silent_execution_has_its_own_budget_and_the_grace_period() {
        let grace_period = std::time::Duration::from_mins(1);
        let started = execution(0, ExecutionStatus::Unknown, Some(10), None);
        assert_eq!(
            started.until_deadline(at(20), grace_period),
            Some(std::time::Duration::from_secs(150))
        );
        assert!(!started.is_past_deadline(at(170), grace_period));
        assert!(started.is_past_deadline(at(171), grace_period));
        // One that never started counts from when it was sent.
        let mut sent = execution(0, ExecutionStatus::Unknown, None, None);
        sent.sent = at(5);
        assert!(sent.is_past_deadline(at(166), grace_period));
    }

    /// The migration gives each job in flight 1 execution under the job's own UUID, which its
    /// runner echoes, and which holds every iteration of the job.
    #[test]
    fn migration_gives_each_job_in_flight_its_whole_job_execution() {
        use diesel_migrations::MigrationHarness as _;

        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        diesel::sql_query(
            "INSERT INTO runner (uuid, name, slug, key_hash, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000070', 'Runner', 'runner', 'hash', 0, 0)",
        )
        .execute(&mut conn)
        .unwrap();

        conn.batch_execute("PRAGMA foreign_keys = OFF").unwrap();
        let version = conn.revert_last_migration(crate::MIGRATIONS).unwrap();
        assert_eq!(version.to_string(), "20261124120000");
        let jobs: Vec<(JobStatus, JobId)> = [
            JobStatus::Pending,
            JobStatus::Claimed,
            JobStatus::Running,
            JobStatus::Unknown,
            JobStatus::Processed,
        ]
        .into_iter()
        .map(|status| (status, create_job(&mut conn, fixture, status)))
        .collect();
        diesel::sql_query(
            "UPDATE job SET config = json_set(config, '$.iter', 3), claimed = 1000, last_heartbeat = 1010,
                runner_id = (SELECT id FROM runner), started = CASE status WHEN 1 THEN NULL ELSE 1005 END
                WHERE status != 0",
        )
        .execute(&mut conn)
        .unwrap();
        conn.run_pending_migrations(crate::MIGRATIONS).unwrap();
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();

        for (status, job_id) in jobs {
            let job = QueryJob::get(&mut conn, job_id).unwrap();
            let executions = QueryJobExecution::of_job(&mut conn, job_id).unwrap();
            let expected = match status {
                JobStatus::Claimed => Some((ExecutionStatus::Sent, None)),
                JobStatus::Running => Some((ExecutionStatus::Running, Some(1005))),
                JobStatus::Unknown => Some((ExecutionStatus::Unknown, Some(1005))),
                JobStatus::Pending
                | JobStatus::Completed
                | JobStatus::Processed
                | JobStatus::Failed
                | JobStatus::Canceled => None,
            };
            let Some((execution_status, started)) = expected else {
                assert!(executions.is_empty(), "{status}");
                continue;
            };
            let [execution] = executions.as_slice() else {
                panic!("{status}: {executions:?}");
            };
            assert_eq!(
                (
                    execution.uuid,
                    execution.iteration,
                    execution.status,
                    execution.sent.timestamp(),
                    execution.started.map(|started| started.timestamp()),
                    execution.last_heartbeat.timestamp(),
                    u32::from(execution.timeout),
                ),
                (job.uuid, 0, execution_status, 1000, started, 1010, 3600),
                "{status}"
            );
            assert!(execution.is_whole_job(&job), "{status}");
        }
    }
}
