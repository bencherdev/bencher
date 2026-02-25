use std::sync::Arc;

use bencher_json::{
    DateTime, ImageDigest, JobPriority, JobStatus, JobUuid, JsonJob, JsonJobConfig, PlanLevel,
    Timeout, runner::job::JsonNewRunJob,
};
use diesel::{BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;
use tokio::sync::Mutex;

use crate::{
    context::{ApiContext, DbConnection},
    error::{bad_request_error, resource_conflict_err},
    macros::fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
    model::{
        organization::{OrganizationId, plan::PlanKind},
        project::{QueryProject, report::ReportId},
        runner::{QueryRunner, RunnerId, SourceIp},
        spec::{QuerySpec, SpecId},
    },
    schema::{self, job as job_table},
    write_conn,
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
    pub config: JsonJobConfig,
    pub timeout: Timeout,
    pub priority: JobPriority,
    pub status: JobStatus,
    pub runner_id: Option<RunnerId>,
    pub claimed: Option<DateTime>,
    pub started: Option<DateTime>,
    pub completed: Option<DateTime>,
    pub last_heartbeat: Option<DateTime>,
    pub last_billed_minute: Option<i32>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryJob {
    fn_get!(job, JobId);
    fn_get_id!(job, JobId, JobUuid);
    fn_get_uuid!(job, JobId, JobUuid);
    fn_from_uuid!(job, JobUuid, Job);

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
            created: self.created,
            modified: self.modified,
            output: None,
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
    pub config: JsonJobConfig,
    pub timeout: Timeout,
    pub priority: JobPriority,
    pub status: JobStatus,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertJob {
    #[expect(
        clippy::too_many_arguments,
        reason = "job creation has many dimensions"
    )]
    fn new(
        report_id: ReportId,
        organization_id: OrganizationId,
        source_ip: SourceIp,
        spec_id: SpecId,
        config: JsonJobConfig,
        timeout: Timeout,
        priority: JobPriority,
        now: DateTime,
    ) -> Self {
        Self {
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
        }
    }
}

/// Pre-validated job that is ready to be inserted once a report ID is available.
///
/// This separates async validation (registry checks, OCI digest resolution) from
/// the actual database insert, allowing callers to validate the job *before*
/// inserting the report — making report + job creation atomic.
pub struct PendingInsertJob {
    organization_id: OrganizationId,
    source_ip: SourceIp,
    spec_id: SpecId,
    config: JsonJobConfig,
    timeout: Timeout,
    priority: JobPriority,
}

impl PendingInsertJob {
    pub async fn from_run(
        context: &ApiContext,
        query_project: &QueryProject,
        source_ip: SourceIp,
        spec_id: SpecId,
        plan_kind: &PlanKind,
        is_claimed: bool,
        new_run_job: JsonNewRunJob,
    ) -> Result<Self, HttpError> {
        // 1. Validate registry and resolve image digest
        let registry_url = context.registry_url();
        let registry_host = registry_url.host_str().ok_or_else(|| {
            bad_request_error(format!("Registry URL has no host: {registry_url}"))
        })?;
        new_run_job
            .image
            .validate_registry(registry_host)
            .map_err(|e| bad_request_error(e.to_string()))?;
        let registry_url: bencher_json::Url = registry_url.clone().into();
        let digest = resolve_digest(
            &new_run_job.image,
            &query_project.uuid,
            context.oci_storage(),
        )
        .await?;

        // 2. Determine priority
        let priority = determine_priority(plan_kind, is_claimed);

        // 3. Resolve timeout (clamped by plan tier)
        let timeout = resolve_timeout(new_run_job.timeout, plan_kind, is_claimed);

        // 4. Build config
        let config = JsonJobConfig {
            registry: registry_url,
            project: query_project.uuid,
            digest,
            entrypoint: new_run_job.entrypoint,
            cmd: new_run_job.cmd,
            env: new_run_job.env,
            timeout,
            file_paths: new_run_job.file_paths,
        };

        Ok(Self {
            organization_id: query_project.organization_id,
            source_ip,
            spec_id,
            config,
            timeout,
            priority,
        })
    }

    /// Finalize the pending job with a report ID and insert it into the database.
    pub async fn insert(self, context: &ApiContext, report_id: ReportId) -> Result<(), HttpError> {
        let now = context.clock.now();
        let insert_job = InsertJob::new(
            report_id,
            self.organization_id,
            self.source_ip,
            self.spec_id,
            self.config,
            self.timeout,
            self.priority,
            now,
        );
        diesel::insert_into(schema::job::table)
            .values(&insert_job)
            .execute(write_conn!(context))
            .map_err(resource_conflict_err!(Job, insert_job))?;
        Ok(())
    }
}

async fn resolve_digest(
    image: &bencher_json::ImageReference,
    project_uuid: &bencher_json::ProjectUuid,
    oci_storage: &bencher_oci_storage::OciStorage,
) -> Result<ImageDigest, HttpError> {
    if image.is_digest() {
        image
            .reference()
            .parse()
            .map_err(|e| bad_request_error(format!("Invalid image digest for `{image}`: {e}")))
    } else {
        let tag: bencher_oci_storage::Tag = image
            .reference()
            .parse()
            .map_err(|e| bad_request_error(format!("Invalid image tag for `{image}`: {e}")))?;
        let oci_digest = oci_storage
            .resolve_tag(project_uuid, &tag)
            .await
            .map_err(|e| {
                bad_request_error(format!("Failed to resolve image tag for `{image}`: {e}"))
            })?;
        oci_digest.as_str().parse().map_err(|e| {
            bad_request_error(format!(
                "Failed to parse resolved digest for `{image}`: {e}"
            ))
        })
    }
}

/// Resolve the job timeout, clamping to plan-tier maximums.
/// - Unclaimed: max 5 min
/// - Free (`PlanKind::None`): max 15 min
/// - Paid (Metered/Licensed): default 1 hour, no upper bound
fn resolve_timeout(requested: Option<Timeout>, plan_kind: &PlanKind, is_claimed: bool) -> Timeout {
    if !is_claimed {
        return requested.map_or(Timeout::UNCLAIMED_MAX, |t| {
            t.clamp_max(Timeout::UNCLAIMED_MAX)
        });
    }
    match plan_kind {
        PlanKind::None => requested.map_or(Timeout::FREE_MAX, |t| t.clamp_max(Timeout::FREE_MAX)),
        PlanKind::Metered(_) | PlanKind::Licensed(_) => requested.unwrap_or(Timeout::PAID_DEFAULT),
    }
}

fn determine_priority(plan_kind: &PlanKind, is_claimed: bool) -> JobPriority {
    if !is_claimed {
        return JobPriority::Unclaimed;
    }
    match plan_kind {
        PlanKind::None => JobPriority::Free,
        // TODO: Check metered plan level to distinguish Team vs Enterprise
        PlanKind::Metered(_) => JobPriority::Team,
        PlanKind::Licensed(license_usage) => match license_usage.level {
            PlanLevel::Free => JobPriority::Free,
            PlanLevel::Team => JobPriority::Team,
            PlanLevel::Enterprise => JobPriority::Enterprise,
        },
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{Entitlements, MeteredPlanId};
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::model::organization::plan::LicenseUsage;

    fn metered_plan() -> PlanKind {
        PlanKind::Metered("test_plan".parse::<MeteredPlanId>().unwrap())
    }

    fn licensed_plan(level: PlanLevel) -> PlanKind {
        PlanKind::Licensed(LicenseUsage {
            entitlements: Entitlements::try_from(1000).unwrap(),
            usage: 0,
            level,
        })
    }

    // --- resolve_timeout tests ---

    #[test]
    fn timeout_unclaimed_default() {
        let timeout = resolve_timeout(None, &PlanKind::None, false);
        assert_eq!(u32::from(timeout), u32::from(Timeout::UNCLAIMED_MAX));
    }

    #[test]
    fn timeout_unclaimed_clamped() {
        let requested = Timeout::try_from(600).unwrap(); // 10 min > 5 min max
        let timeout = resolve_timeout(Some(requested), &PlanKind::None, false);
        assert_eq!(u32::from(timeout), u32::from(Timeout::UNCLAIMED_MAX));
    }

    #[test]
    fn timeout_unclaimed_below_max() {
        let requested = Timeout::try_from(60).unwrap(); // 1 min < 5 min max
        let timeout = resolve_timeout(Some(requested), &PlanKind::None, false);
        assert_eq!(u32::from(timeout), 60);
    }

    #[test]
    fn timeout_free_default() {
        let timeout = resolve_timeout(None, &PlanKind::None, true);
        assert_eq!(u32::from(timeout), u32::from(Timeout::FREE_MAX));
    }

    #[test]
    fn timeout_free_clamped() {
        let requested = Timeout::try_from(1800).unwrap(); // 30 min > 15 min max
        let timeout = resolve_timeout(Some(requested), &PlanKind::None, true);
        assert_eq!(u32::from(timeout), u32::from(Timeout::FREE_MAX));
    }

    #[test]
    fn timeout_free_below_max() {
        let requested = Timeout::try_from(120).unwrap();
        let timeout = resolve_timeout(Some(requested), &PlanKind::None, true);
        assert_eq!(u32::from(timeout), 120);
    }

    #[test]
    fn timeout_metered_default() {
        let timeout = resolve_timeout(None, &metered_plan(), true);
        assert_eq!(u32::from(timeout), u32::from(Timeout::PAID_DEFAULT));
    }

    #[test]
    fn timeout_metered_custom() {
        let requested = Timeout::try_from(7200).unwrap(); // 2 hours, no cap
        let timeout = resolve_timeout(Some(requested), &metered_plan(), true);
        assert_eq!(u32::from(timeout), 7200);
    }

    #[test]
    fn timeout_licensed_default() {
        let plan = licensed_plan(PlanLevel::Team);
        let timeout = resolve_timeout(None, &plan, true);
        assert_eq!(u32::from(timeout), u32::from(Timeout::PAID_DEFAULT));
    }

    #[test]
    fn timeout_licensed_custom() {
        let plan = licensed_plan(PlanLevel::Enterprise);
        let requested = Timeout::try_from(86400).unwrap(); // 24 hours, no cap
        let timeout = resolve_timeout(Some(requested), &plan, true);
        assert_eq!(u32::from(timeout), 86400);
    }

    // --- determine_priority tests ---

    #[test]
    fn priority_unclaimed() {
        assert_eq!(
            determine_priority(&PlanKind::None, false),
            JobPriority::Unclaimed
        );
    }

    #[test]
    fn priority_unclaimed_ignores_plan() {
        // Even with a paid plan, unclaimed is always Unclaimed
        assert_eq!(
            determine_priority(&metered_plan(), false),
            JobPriority::Unclaimed
        );
    }

    #[test]
    fn priority_free() {
        assert_eq!(determine_priority(&PlanKind::None, true), JobPriority::Free);
    }

    #[test]
    fn priority_metered() {
        assert_eq!(determine_priority(&metered_plan(), true), JobPriority::Team);
    }

    #[test]
    fn priority_licensed_free() {
        assert_eq!(
            determine_priority(&licensed_plan(PlanLevel::Free), true),
            JobPriority::Free
        );
    }

    #[test]
    fn priority_licensed_team() {
        assert_eq!(
            determine_priority(&licensed_plan(PlanLevel::Team), true),
            JobPriority::Team
        );
    }

    #[test]
    fn priority_licensed_enterprise() {
        assert_eq!(
            determine_priority(&licensed_plan(PlanLevel::Enterprise), true),
            JobPriority::Enterprise
        );
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
    clock: bencher_json::Clock,
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
            if check_job_timeout(&log, &job, job_timeout_grace_period, &mut conn, &clock) {
                return;
            }

            // If the runner reconnected and sent a recent heartbeat, don't fail the job
            if let Some(last_heartbeat) = job.last_heartbeat {
                let now = clock.now();
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
                        clock,
                    );
                    return;
                }
            }

            // Mark the job as failed
            slog::warn!(log, "Heartbeat timeout, marking job as failed"; "job_id" => ?job_id);
            let now = clock.now();
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
    clock: &bencher_json::Clock,
) -> usize {
    let heartbeat_timeout =
        chrono::Duration::from_std(heartbeat_timeout).unwrap_or(chrono::Duration::MAX);
    let cutoff = clock.now() - heartbeat_timeout;

    // Find claimed jobs where claimed (or created, if claimed is NULL) is older than the cutoff
    let orphaned_jobs: Vec<QueryJob> = match schema::job::table
        .filter(schema::job::status.eq(JobStatus::Claimed))
        .filter(
            schema::job::claimed.le(cutoff).or(schema::job::claimed
                .is_null()
                .and(schema::job::created.le(cutoff))),
        )
        .load(conn)
    {
        Ok(jobs) => jobs,
        Err(e) => {
            slog::error!(log, "Failed to query orphaned claimed jobs"; "error" => %e);
            return 0;
        },
    };

    let mut recovered = 0;
    for job in &orphaned_jobs {
        if job.claimed.is_none() {
            // Claimed but no timestamp — should not happen, fail it anyway
            slog::warn!(log, "Claimed job has no claimed timestamp"; "job_id" => ?job.id);
        }

        slog::warn!(log, "Recovering orphaned claimed job"; "job_id" => ?job.id);
        let now = clock.now();
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
        slog::info!(log, "Recovered orphaned claimed jobs"; "count" => recovered);
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
    clock: &bencher_json::Clock,
) -> bool {
    let Some(started) = job.started else {
        return false;
    };
    let now = clock.now();
    let elapsed = (now.timestamp() - started.timestamp()).max(0);
    #[expect(
        clippy::cast_possible_wrap,
        reason = "timeout max i32::MAX + grace period fits in i64"
    )]
    let limit =
        u64::from(u32::from(job.timeout)) as i64 + job_timeout_grace_period.as_secs() as i64;
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
