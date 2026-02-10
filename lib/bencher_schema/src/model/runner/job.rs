use std::sync::Arc;

use bencher_json::{DateTime, JobStatus, JobUuid, JsonJob, JsonJobSpec};
use diesel::{BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;
use tokio::sync::Mutex;

use crate::{
    context::DbConnection,
    error::issue_error,
    model::{
        organization::OrganizationId,
        project::report::ReportId,
        runner::{QueryRunner, RunnerId, SourceIp},
    },
    resource_not_found_err,
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
    pub status: JobStatus,
    pub spec: String,
    pub timeout: i32,
    pub priority: i32,
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
    pub fn get(conn: &mut DbConnection, id: JobId) -> Result<Self, HttpError> {
        schema::job::table
            .filter(schema::job::id.eq(id))
            .first(conn)
            .map_err(resource_not_found_err!(Job, id))
    }

    pub fn from_uuid(conn: &mut DbConnection, uuid: JobUuid) -> Result<Self, HttpError> {
        schema::job::table
            .filter(schema::job::uuid.eq(uuid))
            .first(conn)
            .map_err(resource_not_found_err!(Job, uuid))
    }

    /// Parse the job spec from JSON string.
    pub fn parse_spec(&self) -> Result<JsonJobSpec, HttpError> {
        serde_json::from_str(&self.spec).map_err(|e| {
            issue_error(
                "Invalid job spec",
                "Job spec stored in database could not be parsed",
                e,
            )
        })
    }

    /// Convert to JSON for public API (spec is not included).
    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonJob, HttpError> {
        let runner_uuid = if let Some(runner_id) = self.runner_id {
            QueryRunner::get(conn, runner_id).ok().map(|r| r.uuid)
        } else {
            None
        };

        Ok(JsonJob {
            uuid: self.uuid,
            status: self.status,
            spec: None,
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
    pub status: JobStatus,
    pub spec: String,
    pub timeout: i32,
    pub priority: i32,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertJob {
    pub fn new(
        report_id: ReportId,
        organization_id: OrganizationId,
        source_ip: SourceIp,
        spec: String,
        timeout: i32,
        priority: i32,
    ) -> Self {
        let now = DateTime::now();
        Self {
            uuid: JobUuid::new(),
            report_id,
            organization_id,
            source_ip,
            status: JobStatus::default(),
            spec,
            timeout,
            priority,
            created: now,
            modified: now,
        }
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
            if let Some(started) = job.started {
                let now = DateTime::now();
                let elapsed = (now.timestamp() - started.timestamp()).max(0);
                #[expect(
                    clippy::cast_possible_wrap,
                    reason = "job timeout and grace period fit in i64"
                )]
                let limit = i64::from(job.timeout) + job_timeout_grace_period.as_secs() as i64;
                if elapsed > limit {
                    slog::warn!(log, "Job timeout exceeded, marking as canceled"; "job_id" => ?job_id, "elapsed" => elapsed, "limit" => limit);
                    let cancel_update = UpdateJob {
                        status: Some(JobStatus::Canceled),
                        completed: Some(Some(now)),
                        modified: Some(now),
                        ..Default::default()
                    };
                    // Use status filter to avoid TOCTOU race
                    if let Err(e) = diesel::update(
                        schema::job::table
                            .filter(schema::job::id.eq(job_id))
                            .filter(
                                schema::job::status
                                    .eq(JobStatus::Claimed)
                                    .or(schema::job::status.eq(JobStatus::Running)),
                            ),
                    )
                    .set(&cancel_update)
                    .execute(&mut *conn)
                    {
                        slog::error!(log, "Failed to cancel timed-out job"; "job_id" => ?job_id, "error" => %e);
                    }
                    return;
                }
            }

            // If the runner reconnected and sent a recent heartbeat, don't fail the job
            if let Some(last_heartbeat) = job.last_heartbeat {
                let now = DateTime::now();
                let elapsed = (now.timestamp() - last_heartbeat.timestamp()).max(0);
                if elapsed < i64::try_from(timeout.as_secs()).unwrap_or(i64::MAX) {
                    // Heartbeat is recent, runner reconnected — schedule another timeout
                    let remaining = std::time::Duration::from_secs(
                        u64::try_from(
                            i64::try_from(timeout.as_secs()).unwrap_or(i64::MAX) - elapsed,
                        )
                        .unwrap_or(0),
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

            if let Err(e) = diesel::update(schema::job::table.filter(schema::job::id.eq(job_id)))
                .set(&update)
                .execute(&mut *conn)
            {
                slog::error!(log, "Failed to mark job as failed"; "job_id" => ?job_id, "error" => %e);
            }
        }
    });

    heartbeat_tasks.insert(job_id, join_handle.abort_handle());
}
