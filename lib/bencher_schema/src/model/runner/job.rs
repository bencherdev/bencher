use std::sync::Arc;

use bencher_json::{DateTime, JobPriority, JobStatus, JobUuid, JsonJob, JsonJobConfig};
use diesel::{BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;
use tokio::sync::Mutex;

use crate::{
    context::DbConnection,
    error::issue_error,
    macros::fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
    model::{
        organization::OrganizationId,
        project::report::ReportId,
        runner::{QueryRunner, RunnerId, SourceIp},
        spec::{QuerySpec, SpecId},
    },
    schema::{self, job as job_table},
};

crate::macros::typed_id::typed_id!(JobId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = job_table)]
#[diesel(belongs_to(QueryRunner, foreign_key = runner_id))]
pub struct QueryJob {
    pub id: JobId,
    pub uuid: JobUuid,
    pub report_id: ReportId,
    pub organization_id: OrganizationId,
    pub source_ip: SourceIp,
    pub spec_id: SpecId,
    pub config: String,
    pub timeout: i32,
    pub priority: JobPriority,
    pub status: JobStatus,
    pub runner_id: Option<RunnerId>,
    pub claimed: Option<DateTime>,
    pub started: Option<DateTime>,
    pub completed: Option<DateTime>,
    pub last_heartbeat: Option<DateTime>,
    pub last_billed_minute: Option<i32>,
    pub exit_code: Option<i32>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryJob {
    fn_get!(job, JobId);
    fn_get_id!(job, JobId, JobUuid);
    fn_get_uuid!(job, JobId, JobUuid);
    fn_from_uuid!(job, JobUuid, Job);

    /// Parse the job config from JSON string.
    pub fn parse_config(&self) -> Result<JsonJobConfig, HttpError> {
        serde_json::from_str(&self.config).map_err(|e| {
            issue_error(
                "Invalid job config",
                "Job config stored in database could not be parsed",
                e,
            )
        })
    }

    /// Convert to JSON for public API (config is not included).
    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonJob, HttpError> {
        let runner_uuid = if let Some(runner_id) = self.runner_id {
            Some(QueryRunner::get(conn, runner_id)?.uuid)
        } else {
            None
        };

        let json_spec = QuerySpec::get(conn, self.spec_id)?.into_json();

        Ok(JsonJob {
            uuid: self.uuid,
            status: self.status,
            spec: json_spec,
            config: None,
            runner: runner_uuid,
            claimed: self.claimed,
            started: self.started,
            completed: self.completed,
            exit_code: self.exit_code,
            created: self.created,
            modified: self.modified,
        })
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = job_table)]
pub struct InsertJob {
    pub uuid: JobUuid,
    pub report_id: ReportId,
    pub organization_id: OrganizationId,
    pub source_ip: SourceIp,
    pub spec_id: SpecId,
    pub config: String,
    pub timeout: i32,
    pub priority: JobPriority,
    pub status: JobStatus,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertJob {
    pub fn new(
        report_id: ReportId,
        organization_id: OrganizationId,
        source_ip: SourceIp,
        spec_id: SpecId,
        config: &JsonJobConfig,
        timeout: i32,
        priority: JobPriority,
    ) -> Result<Self, HttpError> {
        let config = serde_json::to_string(config)
            .map_err(|e| issue_error("Invalid job config", "Failed to serialize job config", e))?;
        let now = DateTime::now();
        Ok(Self {
            uuid: JobUuid::new(),
            report_id,
            organization_id,
            source_ip,
            spec_id,
            config,
            timeout,
            priority,
            status: JobStatus::default(),
            created: now,
            modified: now,
        })
    }
}

#[derive(Debug, Default, diesel::AsChangeset)]
#[diesel(table_name = job_table)]
pub struct UpdateJob {
    pub status: Option<JobStatus>,
    pub runner_id: Option<Option<RunnerId>>,
    pub claimed: Option<Option<DateTime>>,
    pub started: Option<Option<DateTime>>,
    pub completed: Option<Option<DateTime>>,
    pub last_heartbeat: Option<Option<DateTime>>,
    pub last_billed_minute: Option<Option<i32>>,
    pub exit_code: Option<Option<i32>>,
    pub modified: Option<DateTime>,
}

/// Spawn a background task that marks a job as failed if no heartbeat is received
/// within the timeout period. This handles both "disconnected runner" recovery
/// and startup recovery for in-flight jobs.
///
/// Also enforces job timeout: if the job has been running longer than its configured
/// `timeout` plus `job_timeout_grace_period`, it is marked as Canceled so the runner
/// receives a Cancel event on its next heartbeat.
pub fn spawn_heartbeat_timeout(
    log: slog::Logger,
    timeout: std::time::Duration,
    connection: Arc<Mutex<DbConnection>>,
    job_id: JobId,
    heartbeat_tasks: &crate::context::HeartbeatTasks,
    job_timeout_grace_period: std::time::Duration,
) {
    let join_handle = tokio::spawn({
        let heartbeat_tasks = heartbeat_tasks.clone();
        async move {
            tokio::time::sleep(timeout).await;

            let mut conn = connection.lock().await;

            // Read the current job state
            let job: QueryJob = match schema::job::table
                .filter(schema::job::id.eq(job_id))
                .first(&mut *conn)
            {
                Ok(job) => job,
                Err(e) => {
                    slog::error!(log, "Failed to read job for heartbeat timeout"; "job_id" => ?job_id, "error" => %e);
                    return;
                },
            };

            // If the job is already in a terminal state, nothing to do
            if job.status.is_terminal() {
                return;
            }

            // Check job timeout: if running longer than timeout + grace period, cancel it
            if check_job_timeout(&log, &job, job_timeout_grace_period, &mut conn) {
                return;
            }

            // If the runner reconnected and sent a recent heartbeat, don't fail the job
            if let Some(last_heartbeat) = job.last_heartbeat {
                let now = DateTime::now();
                let elapsed = (now.timestamp() - last_heartbeat.timestamp()).max(0);
                if elapsed < i64::try_from(timeout.as_secs()).unwrap_or(i64::MAX) {
                    // Heartbeat is recent, runner reconnected — schedule another timeout
                    let remaining = std::cmp::max(
                        std::time::Duration::from_secs(
                            u64::try_from(
                                i64::try_from(timeout.as_secs()).unwrap_or(i64::MAX) - elapsed,
                            )
                            .unwrap_or(0),
                        ),
                        std::time::Duration::from_secs(1),
                    );
                    drop(conn);
                    let connection_clone = connection.clone();
                    spawn_heartbeat_timeout(
                        log,
                        remaining,
                        connection_clone,
                        job_id,
                        &heartbeat_tasks,
                        job_timeout_grace_period,
                    );
                    return;
                }
            }

            // Mark the job as failed
            slog::warn!(log, "Heartbeat timeout, marking job as failed"; "job_id" => ?job_id);
            let now = DateTime::now();
            let update = UpdateJob {
                status: Some(JobStatus::Failed),
                completed: Some(Some(now)),
                modified: Some(now),
                ..Default::default()
            };

            match diesel::update(
                schema::job::table
                    .filter(schema::job::id.eq(job_id))
                    .filter(
                        schema::job::status
                            .eq(JobStatus::Claimed)
                            .or(schema::job::status.eq(JobStatus::Running)),
                    ),
            )
            .set(&update)
            .execute(&mut *conn)
            {
                Ok(0) => {
                    slog::info!(log, "Heartbeat timeout: job already in terminal state"; "job_id" => ?job_id);
                },
                Ok(_) => {
                    #[cfg(feature = "otel")]
                    bencher_otel::ApiMeter::increment(
                        bencher_otel::ApiCounter::RunnerHeartbeatTimeout,
                    );
                },
                Err(e) => {
                    slog::error!(log, "Failed to mark job as failed"; "job_id" => ?job_id, "error" => %e);
                },
            }
        }
    });

    heartbeat_tasks.insert(job_id, join_handle.abort_handle());
}

/// Recover jobs stuck in `Claimed` status that were claimed longer ago than the
/// heartbeat timeout. These are orphaned: the runner claimed them but never
/// transitioned them to `Running` (e.g., the runner crashed after claiming).
///
/// Returns the number of jobs recovered (transitioned to `Failed`).
pub fn recover_orphaned_claimed_jobs(
    log: &slog::Logger,
    conn: &mut DbConnection,
    heartbeat_timeout: std::time::Duration,
) -> usize {
    let now = DateTime::now();
    #[expect(clippy::cast_possible_wrap, reason = "heartbeat timeout fits in i64")]
    let cutoff_timestamp = now.timestamp() - heartbeat_timeout.as_secs() as i64;

    // Find claimed jobs where claimed_at is older than the heartbeat timeout
    let orphaned_jobs: Vec<QueryJob> = match schema::job::table
        .filter(schema::job::status.eq(JobStatus::Claimed))
        .load(conn)
    {
        Ok(jobs) => jobs,
        Err(e) => {
            slog::error!(log, "Failed to query orphaned claimed jobs: {e}");
            return 0;
        },
    };

    let mut recovered = 0;
    for job in orphaned_jobs {
        let claimed_at = if let Some(claimed) = job.claimed {
            claimed
        } else {
            // Claimed but no timestamp — should not happen, fail it anyway
            slog::warn!(log, "Claimed job has no claimed timestamp"; "job_id" => ?job.id);
            job.created
        };

        if claimed_at.timestamp() > cutoff_timestamp {
            // Not yet orphaned
            continue;
        }

        slog::warn!(log, "Recovering orphaned claimed job"; "job_id" => ?job.id);
        let update = UpdateJob {
            status: Some(JobStatus::Failed),
            completed: Some(Some(now)),
            modified: Some(now),
            ..Default::default()
        };

        match diesel::update(
            schema::job::table
                .filter(schema::job::id.eq(job.id))
                .filter(schema::job::status.eq(JobStatus::Claimed)),
        )
        .set(&update)
        .execute(conn)
        {
            Ok(0) => {
                slog::info!(log, "Orphaned job already changed state"; "job_id" => ?job.id);
            },
            Ok(_) => {
                recovered += 1;
            },
            Err(e) => {
                slog::error!(log, "Failed to recover orphaned job"; "job_id" => ?job.id, "error" => %e);
            },
        }
    }

    if recovered > 0 {
        slog::info!(log, "Recovered {recovered} orphaned claimed job(s)");
    }

    recovered
}

/// Check if a job has exceeded its timeout + grace period.
/// If so, mark it as canceled and return `true` to indicate the caller should stop.
fn check_job_timeout(
    log: &slog::Logger,
    job: &QueryJob,
    job_timeout_grace_period: std::time::Duration,
    conn: &mut DbConnection,
) -> bool {
    let Some(started) = job.started else {
        return false;
    };
    let now = DateTime::now();
    let elapsed = (now.timestamp() - started.timestamp()).max(0);
    #[expect(
        clippy::cast_possible_wrap,
        reason = "job timeout and grace period fit in i64"
    )]
    let limit = i64::from(job.timeout) + job_timeout_grace_period.as_secs() as i64;
    if elapsed <= limit {
        return false;
    }
    slog::warn!(log, "Job timeout exceeded, marking as canceled"; "job_id" => ?job.id, "elapsed" => elapsed, "limit" => limit);
    let cancel_update = UpdateJob {
        status: Some(JobStatus::Canceled),
        completed: Some(Some(now)),
        modified: Some(now),
        ..Default::default()
    };
    // Use status filter to avoid TOCTOU race
    match diesel::update(
        schema::job::table
            .filter(schema::job::id.eq(job.id))
            .filter(
                schema::job::status
                    .eq(JobStatus::Claimed)
                    .or(schema::job::status.eq(JobStatus::Running)),
            ),
    )
    .set(&cancel_update)
    .execute(conn)
    {
        Ok(updated) if updated > 0 => {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobTimeout);
        },
        Ok(_) => {},
        Err(e) => {
            slog::error!(log, "Failed to cancel timed-out job"; "job_id" => ?job.id, "error" => %e);
        },
    }
    true
}
