use std::sync::Arc;

use bencher_callback::{CallbackKey, SealedRequest};
use bencher_json::{
    BmfVersion, DateTime, ImageDigest, JobStatus, JobUuid, JsonJob, JsonJobConfig, Priority,
    ReportUuid, Timeout,
    project::report::JsonReportSettings,
    runner::{JsonIterationOutput, JsonNewCallback, job::JsonNewRunJob},
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    result::QueryResult,
};
use dropshot::HttpError;
use tokio::sync::Mutex;

use slog::Logger;

use crate::{
    auth_conn,
    context::{ApiContext, Callbacks, DbConnection},
    error::{bad_request_error, issue_error, resource_not_found_err},
    macros::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        sql::last_insert_rowid,
    },
    model::{
        organization::{OrganizationId, plan::PlanKind},
        project::{
            ProjectId, QueryProject,
            report::{QueryReport, ReportId},
        },
        runner::{InsertJobCallback, QueryJobCallbackView, QueryRunner, RunnerId, SourceIp},
        spec::{QuerySpec, SpecId},
        user::{actor::ApiActor, public::PublicUser},
    },
    schema::{self, job as job_table},
    write_conn, write_transaction,
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
    pub organization_id: OrganizationId,
    pub project_id: ProjectId,
    pub report_id: ReportId,
    pub source_ip: SourceIp,
    pub spec_id: SpecId,
    pub config: JsonJobConfig,
    pub timeout: Timeout,
    pub priority: Priority,
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

    /// When the job's deadline clock starts: when it started running,
    /// or when it was claimed if it never started.
    pub fn deadline_start(&self) -> Option<DateTime> {
        self.started.or(self.claimed)
    }

    #[cfg(feature = "plus")]
    pub fn runner_minutes_usage(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
        start_time: DateTime,
        end_time: DateTime,
    ) -> Result<u32, HttpError> {
        schema::job_duration_by_report::table
            .inner_join(schema::report::table.inner_join(schema::project::table))
            .filter(schema::project::organization_id.eq(organization_id))
            .filter(schema::report::end_time.ge(start_time))
            .filter(schema::report::end_time.le(end_time))
            .select(diesel::dsl::sum(
                (schema::job_duration_by_report::job_duration + 59) / 60,
            ))
            .get_result::<Option<i64>>(conn)
            .map_err(|e| {
                issue_error(
                    "Failed to query runner minutes usage",
                    &format!("Failed to query runner minutes usage for organization ({organization_id}) between {start_time} and {end_time}."),
                    e,
                )
            })?
            .unwrap_or_default()
            .try_into()
            .map_err(|e| {
                issue_error(
                    "Failed to count runner minutes usage",
                    &format!("Failed to count runner minutes usage for organization ({organization_id}) between {start_time} and {end_time}."),
                    e,
                )
            })
    }

    /// Process benchmark results from a completed job into the report.
    ///
    /// Looks up the report and branch, parses benchmark output via the adapter,
    /// creates metrics/alerts, checks plan usage, and updates the report timestamps.
    #[expect(clippy::too_many_lines, reason = "sequential job processing steps")]
    pub async fn process_results(
        &self,
        log: &Logger,
        context: &ApiContext,
        results: Vec<JsonIterationOutput>,
        now: DateTime,
    ) -> Result<(), HttpError> {
        // Look up the report
        let query_report: QueryReport = schema::report::table
            .filter(schema::report::id.eq(self.report_id))
            .first(auth_conn!(context))
            .map_err(resource_not_found_err!(Report, self.report_id))?;

        // Get branch_id from the head
        let branch_id = schema::head::table
            .filter(schema::head::id.eq(query_report.head_id))
            .select(schema::head::branch_id)
            .first(auth_conn!(context))
            .map_err(resource_not_found_err!(Head, query_report.head_id))?;

        // Look up the project for plan checks
        let query_project = QueryProject::get(auth_conn!(context), query_report.project_id)?;

        // TODO: Add a RunnerKey variant to ApiActor so runner-authenticated requests use auth_conn
        let api_actor = ApiActor::Public(PublicUser::Public(None));
        let plan_kind = PlanKind::new_for_project(
            context,
            context.biller.as_ref(),
            &context.licensor,
            &query_project,
            &api_actor,
        )
        .await?;

        // Build results array from per-iteration output.
        // File output takes precedence (mirrors CLI CommandToFile mode):
        // each file's contents = one result string for the adapter.
        // Otherwise fall back to stdout (mirrors CLI Command mode).
        //
        // All iterations are flattened into a single Vec<String>, matching
        // the CLI's local behavior (services/cli/src/bencher/sub/run/mod.rs).
        // Without fold: each string becomes a separate enumerated iteration.
        // With fold: all strings are combined via the fold operation.
        let results_strings: Vec<String> = results
            .into_iter()
            .flat_map(|r| {
                if let Some(output) = r.output {
                    output.into_values().collect::<Vec<_>>()
                } else if let Some(stdout) = r.stdout {
                    vec![stdout]
                } else {
                    Vec::new()
                }
            })
            .collect();
        let results_array: Vec<&str> = results_strings.iter().map(AsRef::as_ref).collect();

        // Build settings from job config
        let settings = JsonReportSettings {
            adapter: Some(query_report.adapter),
            average: self.config.average,
            fold: self.config.fold,
        };

        // Process results (adapter parsing, metrics, alerts, usage)
        //
        // The job parses at the version the run declared.
        query_report
            .process_results(
                log,
                context,
                branch_id,
                &results_array,
                query_report.adapter,
                settings,
                self.config.bmf_version.unwrap_or_default(),
                plan_kind,
                #[cfg(feature = "otel")]
                self.priority,
                &query_project,
            )
            .await?;

        // Compute report times and job duration before acquiring write lock
        let started = self.started.unwrap_or(now);
        let (start_time, end_time) = if let Some(backdate) = self.config.backdate {
            let elapsed = now.into_inner() - started.into_inner();
            (backdate, DateTime::from(backdate.into_inner() + elapsed))
        } else {
            (started, now)
        };
        let duration_secs = (now.timestamp() - started.timestamp()).max(0);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "clamped to i32::MAX before cast"
        )]
        let job_duration = duration_secs.min(i64::from(i32::MAX)) as i32;

        // Update report times and record job duration atomically
        let report_id = self.report_id;
        write_transaction!(context, |conn| {
            let updated =
                diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
                    .set((
                        schema::report::start_time.eq(start_time),
                        schema::report::end_time.eq(end_time),
                    ))
                    .execute(conn)?;
            if updated == 0 {
                return Err(diesel::result::Error::NotFound);
            }
            insert_job_duration(conn, report_id, job_duration)
        })
        .map_err(|e| {
            issue_error(
                "Failed to update report times and insert job duration",
                &format!(
                    "Failed to update report times / insert job duration for report {report_id}.",
                ),
                e,
            )
        })?;

        #[cfg(feature = "otel")]
        {
            #[expect(
                clippy::cast_precision_loss,
                reason = "duration seconds fits comfortably in f64"
            )]
            let duration_f64 = duration_secs as f64;
            bencher_otel::ApiMeter::record(
                bencher_otel::ApiHistogram::JobRunDuration(self.priority),
                duration_f64,
            );
        }

        Ok(())
    }

    /// Convert to JSON for public API (config is not included); the caller selects `report_uuid`
    /// and `callback` in the same query as the job, so a list needs no lookup per job.
    pub fn into_json(
        self,
        conn: &mut DbConnection,
        report_uuid: ReportUuid,
        callback: Option<QueryJobCallbackView>,
    ) -> Result<JsonJob, HttpError> {
        let runner_uuid = if let Some(runner_id) = self.runner_id {
            Some(QueryRunner::get(conn, runner_id)?.uuid)
        } else {
            None
        };

        let json_spec = QuerySpec::get(conn, self.spec_id)?.into_json();

        Ok(JsonJob {
            uuid: self.uuid,
            report: report_uuid,
            spec: json_spec,
            config: None,
            timeout: self.timeout,
            status: self.status,
            runner: runner_uuid,
            claimed: self.claimed,
            started: self.started,
            completed: self.completed,
            callback: callback.map(Into::into),
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
    pub organization_id: OrganizationId,
    pub project_id: ProjectId,
    pub report_id: ReportId,
    pub source_ip: SourceIp,
    pub spec_id: SpecId,
    pub config: JsonJobConfig,
    pub timeout: Timeout,
    pub priority: Priority,
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
        uuid: JobUuid,
        organization_id: OrganizationId,
        project_id: ProjectId,
        report_id: ReportId,
        source_ip: SourceIp,
        spec_id: SpecId,
        config: JsonJobConfig,
        timeout: Timeout,
        priority: Priority,
        now: DateTime,
    ) -> Self {
        Self {
            uuid,
            organization_id,
            project_id,
            report_id,
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
    uuid: JobUuid,
    organization_id: OrganizationId,
    project_id: ProjectId,
    source_ip: SourceIp,
    spec_id: SpecId,
    config: JsonJobConfig,
    timeout: Timeout,
    priority: Priority,
    callback: Option<PendingCallback>,
}

impl PendingInsertJob {
    #[expect(
        clippy::too_many_arguments,
        reason = "job creation has many dimensions"
    )]
    pub async fn from_run(
        context: &ApiContext,
        query_project: &QueryProject,
        source_ip: SourceIp,
        spec_id: SpecId,
        plan_kind: &PlanKind,
        is_claimed: bool,
        new_run_job: JsonNewRunJob,
        settings: &JsonReportSettings,
        bmf_version: BmfVersion,
    ) -> Result<Self, HttpError> {
        let JsonNewRunJob {
            image,
            // The spec was resolved to `spec_id` with the testbed.
            spec: _,
            entrypoint,
            cmd,
            env,
            timeout,
            file_paths,
            build_time,
            file_size,
            iter,
            allow_failure,
            backdate,
            callback,
        } = new_run_job;

        // 1. Validate registry and resolve image digest
        let registry_url = context.registry_url();
        let registry_host = registry_url.host_str().ok_or_else(|| {
            bad_request_error(format!("Registry URL has no host: {registry_url}"))
        })?;
        image
            .validate_registry(registry_host, registry_url.port_or_known_default())
            .map_err(|e| bad_request_error(e.to_string()))?;
        let registry_url: bencher_json::Url = registry_url.clone().into();
        let digest = resolve_digest(&image, &query_project.uuid, context.oci_storage()).await?;

        // 2. Determine priority
        let priority = plan_kind.priority(is_claimed);

        // 3. Resolve timeout (clamped by plan tier)
        let timeout = resolve_timeout(timeout, plan_kind, is_claimed);

        // 4. Build config
        let config = JsonJobConfig {
            registry: registry_url,
            project: query_project.uuid,
            digest,
            image: Some(image),
            entrypoint,
            cmd,
            env,
            timeout,
            file_paths,
            build_time,
            file_size,
            average: settings.average,
            iter,
            fold: settings.fold,
            bmf_version: Some(bmf_version),
            allow_failure,
            backdate,
        };

        // 5. Seal the callback to the job now, so no crypto runs while the writer lock is held
        let uuid = JobUuid::new();
        let callback = callback
            .map(|callback| {
                PendingCallback::new(
                    &context.callback_key,
                    uuid,
                    plan_kind,
                    is_claimed,
                    &callback,
                )
            })
            .transpose()?;

        Ok(Self {
            uuid,
            organization_id: query_project.organization_id,
            project_id: query_project.id,
            source_ip,
            spec_id,
            config,
            timeout,
            priority,
            callback,
        })
    }

    /// Whether the job has a callback, and if so, whether the callback was skipped.
    #[cfg(feature = "otel")]
    pub fn callback_skipped(&self) -> Option<bool> {
        self.callback
            .as_ref()
            .map(|callback| matches!(callback, PendingCallback::Skipped))
    }

    /// Finalize the pending job with a report ID and insert it into the database.
    ///
    /// Accepts a `&mut DbConnection` and `DateTime` directly so it can be called
    /// inside a diesel `transaction()` closure for atomicity with the report insert.
    pub fn insert(
        self,
        conn: &mut DbConnection,
        report_id: ReportId,
        now: DateTime,
    ) -> QueryResult<()> {
        let Self {
            uuid,
            organization_id,
            project_id,
            source_ip,
            spec_id,
            config,
            timeout,
            priority,
            callback,
        } = self;
        let insert_job = InsertJob::new(
            uuid,
            organization_id,
            project_id,
            report_id,
            source_ip,
            spec_id,
            config,
            timeout,
            priority,
            now,
        );
        diesel::insert_into(schema::job::table)
            .values(&insert_job)
            .execute(conn)?;

        let Some(callback) = callback else {
            return Ok(());
        };
        let job_id = diesel::select(last_insert_rowid()).get_result::<JobId>(conn)?;
        match callback {
            PendingCallback::Sealed(sealed) => InsertJobCallback::pending(job_id, sealed, now),
            PendingCallback::Skipped => InsertJobCallback::skipped(job_id, now),
        }
        .insert(conn)
    }
}

/// A run's callback: sealed to its job for a claimed project, on a paid plan or as the GitHub Actions
/// dispatch `bencher run` composes, and skipped otherwise.
enum PendingCallback {
    Sealed(SealedRequest),
    Skipped,
}

impl PendingCallback {
    fn new(
        key: &CallbackKey,
        job: JobUuid,
        plan_kind: &PlanKind,
        is_claimed: bool,
        callback: &JsonNewCallback,
    ) -> Result<Self, HttpError> {
        if is_claimed && (plan_kind.is_paid() || callback.is_bencher_run_dispatch()) {
            let plaintext = serde_json::to_vec(callback).map_err(|e| {
                issue_error(
                    "Failed to serialize job callback",
                    &format!("Failed to serialize the callback for job ({job})."),
                    e,
                )
            })?;
            key.seal(job, &plaintext).map(Self::Sealed).map_err(|e| {
                issue_error(
                    "Failed to seal job callback",
                    &format!("Failed to seal the callback for job ({job})."),
                    e,
                )
            })
        } else {
            Ok(Self::Skipped)
        }
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
                let msg = format!("Failed to resolve image tag for `{image}`: {e}");
                if e.status_code().is_server_error() {
                    issue_error(&msg, &msg, e)
                } else {
                    bad_request_error(msg)
                }
            })?;
        oci_digest.as_str().parse().map_err(|e| {
            bad_request_error(format!(
                "Failed to parse resolved digest for `{image}`: {e}"
            ))
        })
    }
}

/// Resolve the job timeout, clamping to plan-tier maximums.
/// - Unclaimed: max 1 min
/// - Free (`PlanKind::None`): max 5 min
/// - Plus (Metered/Licensed): default 1 hour, no upper bound
fn resolve_timeout(requested: Option<Timeout>, plan_kind: &PlanKind, is_claimed: bool) -> Timeout {
    if !is_claimed {
        return requested.map_or(Timeout::UNCLAIMED_MAX, |t| {
            t.clamp_max(Timeout::UNCLAIMED_MAX)
        });
    }
    match plan_kind {
        PlanKind::None => requested.map_or(Timeout::FREE_MAX, |t| t.clamp_max(Timeout::FREE_MAX)),
        PlanKind::Metered(_) | PlanKind::Licensed(_) => requested.unwrap_or(Timeout::PLUS_DEFAULT),
    }
}

/// Insert the job duration summary for a report.
///
/// Uses `on_conflict.do_nothing()` for idempotency — the first write wins.
/// This handles the `reprocess_completed_jobs` path where a report may be
/// processed more than once.
fn insert_job_duration(
    conn: &mut DbConnection,
    report_id: ReportId,
    job_duration: i32,
) -> QueryResult<()> {
    diesel::insert_into(schema::job_duration_by_report::table)
        .values((
            schema::job_duration_by_report::report_id.eq(report_id),
            schema::job_duration_by_report::job_duration.eq(job_duration),
        ))
        .on_conflict(schema::job_duration_by_report::report_id)
        .do_nothing()
        .execute(conn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, Entitlements, PlanLevel};
    use diesel::QueryDsl as _;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        macros::sql::last_insert_rowid,
        model::organization::plan::{LicenseUsage, MeteredPlan},
        test_util::{
            create_base_entities, create_branch_with_head, create_head_version, create_testbed,
            create_version, setup_test_db,
        },
    };

    fn metered_plan() -> PlanKind {
        PlanKind::Metered(MeteredPlan {
            customer_id: "cus_test".into(),
            level: PlanLevel::Pro,
            current_period_start: DateTime::TEST,
            current_period_end: DateTime::TEST,
        })
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
        let requested = Timeout::try_from(120).unwrap(); // 2 min > 1 min max
        let timeout = resolve_timeout(Some(requested), &PlanKind::None, false);
        assert_eq!(u32::from(timeout), u32::from(Timeout::UNCLAIMED_MAX));
    }

    #[test]
    fn timeout_unclaimed_below_max() {
        let requested = Timeout::try_from(30).unwrap(); // 30 sec < 1 min max
        let timeout = resolve_timeout(Some(requested), &PlanKind::None, false);
        assert_eq!(u32::from(timeout), 30);
    }

    #[test]
    fn timeout_free_default() {
        let timeout = resolve_timeout(None, &PlanKind::None, true);
        assert_eq!(u32::from(timeout), u32::from(Timeout::FREE_MAX));
    }

    #[test]
    fn timeout_free_clamped() {
        let requested = Timeout::try_from(600).unwrap(); // 10 min > 5 min max
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
        assert_eq!(u32::from(timeout), u32::from(Timeout::PLUS_DEFAULT));
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
        assert_eq!(u32::from(timeout), u32::from(Timeout::PLUS_DEFAULT));
    }

    #[test]
    fn timeout_licensed_custom() {
        let plan = licensed_plan(PlanLevel::Enterprise);
        let requested = Timeout::try_from(86400).unwrap(); // 24 hours, no cap
        let timeout = resolve_timeout(Some(requested), &plan, true);
        assert_eq!(u32::from(timeout), 86400);
    }

    // --- PlanKind::priority tests ---

    #[test]
    fn priority_unclaimed() {
        assert_eq!(PlanKind::None.priority(false), Priority::Unclaimed);
    }

    #[test]
    fn priority_unclaimed_ignores_plan() {
        // Even with a Plus plan, unclaimed is always Unclaimed
        assert_eq!(metered_plan().priority(false), Priority::Unclaimed);
    }

    #[test]
    fn priority_free() {
        assert_eq!(PlanKind::None.priority(true), Priority::Free);
    }

    #[test]
    fn priority_metered() {
        assert_eq!(metered_plan().priority(true), Priority::Plus);
    }

    #[test]
    fn priority_licensed_free() {
        assert_eq!(
            licensed_plan(PlanLevel::Free).priority(true),
            Priority::Free
        );
    }

    #[test]
    fn priority_licensed_team() {
        assert_eq!(
            licensed_plan(PlanLevel::Team).priority(true),
            Priority::Plus
        );
    }

    #[test]
    fn priority_licensed_enterprise() {
        assert_eq!(
            licensed_plan(PlanLevel::Enterprise).priority(true),
            Priority::Plus
        );
    }

    // --- runner_minutes_usage tests ---

    #[test]
    fn runner_minutes_no_jobs() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let result = QueryJob::runner_minutes_usage(
            &mut conn,
            base.organization_id,
            DateTime::TEST,
            DateTime::TEST,
        )
        .unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn runner_minutes_exact_boundary() {
        let mut conn = setup_test_db();
        let (report_id, org_id) = create_report_for_job_duration_test(&mut conn);
        insert_job_duration(&mut conn, report_id, 60).unwrap();
        let result =
            QueryJob::runner_minutes_usage(&mut conn, org_id, DateTime::TEST, DateTime::TEST)
                .unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn runner_minutes_partial_minute() {
        let mut conn = setup_test_db();
        let (report_id, org_id) = create_report_for_job_duration_test(&mut conn);
        insert_job_duration(&mut conn, report_id, 61).unwrap();
        let result =
            QueryJob::runner_minutes_usage(&mut conn, org_id, DateTime::TEST, DateTime::TEST)
                .unwrap();
        assert_eq!(result, 2);
    }

    #[test]
    fn runner_minutes_one_second() {
        let mut conn = setup_test_db();
        let (report_id, org_id) = create_report_for_job_duration_test(&mut conn);
        insert_job_duration(&mut conn, report_id, 1).unwrap();
        let result =
            QueryJob::runner_minutes_usage(&mut conn, org_id, DateTime::TEST, DateTime::TEST)
                .unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn runner_minutes_zero_seconds() {
        let mut conn = setup_test_db();
        let (report_id, org_id) = create_report_for_job_duration_test(&mut conn);
        insert_job_duration(&mut conn, report_id, 0).unwrap();
        let result =
            QueryJob::runner_minutes_usage(&mut conn, org_id, DateTime::TEST, DateTime::TEST)
                .unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn runner_minutes_outside_range() {
        let mut conn = setup_test_db();
        let (report_id, org_id) = create_report_for_job_duration_test(&mut conn);
        insert_job_duration(&mut conn, report_id, 120).unwrap();
        let start = DateTime::from(DateTime::TEST.into_inner() + chrono::Duration::seconds(100));
        let end = DateTime::from(DateTime::TEST.into_inner() + chrono::Duration::seconds(200));
        let result = QueryJob::runner_minutes_usage(&mut conn, org_id, start, end).unwrap();
        assert_eq!(result, 0);
    }

    // --- insert_job_duration tests ---

    fn create_report_for_job_duration_test(conn: &mut DbConnection) -> (ReportId, OrganizationId) {
        let base = create_base_entities(conn);
        let branch = create_branch_with_head(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000020",
        );
        let testbed_id = create_testbed(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "localhost",
            "localhost",
        );
        let version_id = create_version(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000040",
            0,
            None,
        );
        create_head_version(conn, branch.head_id, version_id);

        conn.immediate_transaction(|conn| {
            diesel::insert_into(schema::report::table)
                .values((
                    schema::report::uuid.eq("00000000-0000-0000-0000-000000000050"),
                    schema::report::project_id.eq(base.project_id),
                    schema::report::head_id.eq(branch.head_id),
                    schema::report::version_id.eq(version_id),
                    schema::report::testbed_id.eq(testbed_id),
                    schema::report::adapter.eq(0),
                    schema::report::start_time.eq(DateTime::TEST),
                    schema::report::end_time.eq(DateTime::TEST),
                    schema::report::created.eq(DateTime::TEST),
                ))
                .execute(conn)?;

            diesel::select(last_insert_rowid()).get_result::<ReportId>(conn)
        })
        .map(|report_id| (report_id, base.organization_id))
        .expect("Failed to insert report")
    }

    #[test]
    fn insert_job_duration_basic() {
        let mut conn = setup_test_db();
        let (report_id, _) = create_report_for_job_duration_test(&mut conn);

        insert_job_duration(&mut conn, report_id, 42).expect("Insert failed");

        let duration: i32 = schema::job_duration_by_report::table
            .filter(schema::job_duration_by_report::report_id.eq(report_id))
            .select(schema::job_duration_by_report::job_duration)
            .first(&mut conn)
            .expect("Failed to read job duration");
        assert_eq!(duration, 42);
    }

    #[test]
    fn insert_job_duration_idempotent() {
        let mut conn = setup_test_db();
        let (report_id, _) = create_report_for_job_duration_test(&mut conn);

        // First insert
        insert_job_duration(&mut conn, report_id, 100).expect("First insert failed");

        // Second insert with different value — should be ignored (do_nothing)
        insert_job_duration(&mut conn, report_id, 999).expect("Second insert failed");

        let duration: i32 = schema::job_duration_by_report::table
            .filter(schema::job_duration_by_report::report_id.eq(report_id))
            .select(schema::job_duration_by_report::job_duration)
            .first(&mut conn)
            .expect("Failed to read job duration");
        // First write wins
        assert_eq!(duration, 100);
    }

    // --- callback tests ---

    fn callback() -> JsonNewCallback {
        JsonNewCallback::new(
            "https://receiver.example/hooks",
            [("Authorization".to_owned(), "Bearer token".to_owned())],
            None,
        )
        .unwrap()
    }

    /// The GitHub Actions dispatch `bencher run` composes, but with `authorization` and `event_type`.
    fn dispatch(authorization: &str, event_type: &str) -> JsonNewCallback {
        JsonNewCallback::new(
            "https://api.github.com/repos/owner/repo/dispatches",
            [
                ("Accept", "application/vnd.github+json"),
                ("Authorization", authorization),
                ("X-GitHub-Api-Version", "2022-11-28"),
            ]
            .map(|(name, value)| (name.to_owned(), value.to_owned())),
            Some(serde_json::json!({
                "event_type": event_type,
                "client_payload": { "bencher": { "job": "{{ job.uuid }}" } },
            })),
        )
        .unwrap()
    }

    fn bencher_run_dispatch() -> JsonNewCallback {
        dispatch("Bearer token", "bencher_run")
    }

    fn unpaid_plans() -> [PlanKind; 2] {
        [PlanKind::None, licensed_plan(PlanLevel::Free)]
    }

    fn callback_key() -> CallbackKey {
        CallbackKey::new(&"callback-test-secret".parse().unwrap()).unwrap()
    }

    #[test]
    fn a_paid_plan_seals_the_callback_to_its_job() {
        let key = callback_key();
        let job = JobUuid::new();
        for plan_kind in [
            metered_plan(),
            licensed_plan(PlanLevel::Pro),
            licensed_plan(PlanLevel::Team),
            licensed_plan(PlanLevel::Enterprise),
        ] {
            let PendingCallback::Sealed(sealed) =
                PendingCallback::new(&key, job, &plan_kind, true, &callback()).unwrap()
            else {
                panic!("a paid plan seals its callback");
            };
            assert_eq!(
                key.open(job, &sealed).unwrap(),
                serde_json::to_vec(&callback()).unwrap(),
                "the sealed plaintext is the request as JSON"
            );
            assert!(
                key.open(JobUuid::new(), &sealed).is_err(),
                "the seal binds the job"
            );
        }
    }

    #[test]
    fn an_unpaid_plan_seals_the_bencher_run_dispatch() {
        let key = callback_key();
        let job = JobUuid::new();
        let dispatch = bencher_run_dispatch();
        for plan_kind in unpaid_plans() {
            let PendingCallback::Sealed(sealed) =
                PendingCallback::new(&key, job, &plan_kind, true, &dispatch).unwrap()
            else {
                panic!("a claimed project seals the bencher_run dispatch on any plan");
            };
            assert_eq!(
                key.open(job, &sealed).unwrap(),
                serde_json::to_vec(&dispatch).unwrap()
            );
        }
    }

    #[test]
    fn an_unpaid_plan_skips_any_other_callback() {
        for plan_kind in unpaid_plans() {
            for (callback, what) in [
                (callback(), "another URL"),
                (
                    dispatch("Basic dXNlcjpwYXNz", "bencher_run"),
                    "a dispatch without a bearer token",
                ),
                (
                    dispatch("Bearer token", "deploy"),
                    "a dispatch of another event",
                ),
            ] {
                let pending = PendingCallback::new(
                    &callback_key(),
                    JobUuid::new(),
                    &plan_kind,
                    true,
                    &callback,
                )
                .unwrap();
                assert!(matches!(pending, PendingCallback::Skipped), "{what}");
            }
        }
    }

    #[test]
    fn an_unclaimed_project_skips_even_the_bencher_run_dispatch() {
        for plan_kind in [
            PlanKind::None,
            licensed_plan(PlanLevel::Free),
            metered_plan(),
        ] {
            for (callback, what) in [
                (bencher_run_dispatch(), "the bencher_run dispatch"),
                (callback(), "another URL"),
            ] {
                let pending = PendingCallback::new(
                    &callback_key(),
                    JobUuid::new(),
                    &plan_kind,
                    false,
                    &callback,
                )
                .unwrap();
                assert!(matches!(pending, PendingCallback::Skipped), "{what}");
            }
        }
    }

    #[test]
    fn a_callback_is_stored_for_the_job_inserted_beside_it() {
        use crate::{
            model::runner::QueryJobCallback,
            test_util::{create_job, create_job_fixture},
        };
        use bencher_json::runner::JobCallbackState;

        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        // An earlier job, so a new job's ID differs from its report's.
        create_job(&mut conn, fixture, JobStatus::Pending);
        let key = callback_key();
        let config: JsonJobConfig = serde_json::from_value(serde_json::json!({
            "registry": "https://registry.bencher.dev",
            "project": "00000000-0000-0000-0000-000000000002",
            "digest": format!("sha256:{}", "0".repeat(64)),
            "timeout": 3600
        }))
        .unwrap();
        for (plan_kind, state) in [
            (metered_plan(), JobCallbackState::Pending),
            (PlanKind::None, JobCallbackState::Skipped),
        ] {
            let uuid = JobUuid::new();
            let pending_job = PendingInsertJob {
                uuid,
                organization_id: fixture.organization_id,
                project_id: fixture.project_id,
                source_ip: SourceIp::new(std::net::Ipv4Addr::LOCALHOST.into()),
                spec_id: fixture.spec_id,
                config: config.clone(),
                timeout: Timeout::PLUS_DEFAULT,
                priority: Priority::Plus,
                callback: Some(
                    PendingCallback::new(&key, uuid, &plan_kind, true, &callback()).unwrap(),
                ),
            };
            conn.immediate_transaction(|conn| {
                pending_job.insert(conn, fixture.report_id, DateTime::TEST)
            })
            .unwrap();

            let job = QueryJob::from_uuid(&mut conn, uuid).unwrap();
            assert_ne!(
                i32::from(job.id),
                i32::from(fixture.report_id),
                "the job and report IDs differ"
            );
            let stored = QueryJobCallback::get(&mut conn, job.id).unwrap();
            assert_eq!(stored.state, state, "the callback row is the job's");
        }
        assert_eq!(
            schema::job_callback::table
                .count()
                .get_result::<i64>(&mut conn)
                .unwrap(),
            2,
            "one row per callback"
        );
    }

    /// Revert every migration down to and including the job project migration, so the
    /// migrations above it are reverted first and `run_pending_migrations` restores them all.
    fn revert_to_job_project_migration(conn: &mut DbConnection) {
        use diesel_migrations::MigrationHarness as _;

        const JOB_PROJECT_MIGRATION: &str = "20260927130000";

        loop {
            let version = conn
                .revert_last_migration(crate::MIGRATIONS)
                .expect("Failed to revert a migration");
            if version.to_string() == JOB_PROJECT_MIGRATION {
                break;
            }
        }
    }

    /// Revert to before the job project migration, run `between`, then run every migration again.
    fn around_job_project_migration(
        conn: &mut DbConnection,
        between: impl FnOnce(&mut DbConnection),
    ) {
        use diesel::connection::SimpleConnection as _;
        use diesel_migrations::MigrationHarness as _;

        // Foreign keys cannot be toggled inside a transaction, and Diesel runs each
        // migration in one, so they are disabled around the revert and re-apply.
        conn.batch_execute("PRAGMA foreign_keys = OFF")
            .expect("Failed to disable foreign keys");
        revert_to_job_project_migration(conn);
        between(conn);
        conn.run_pending_migrations(crate::MIGRATIONS)
            .expect("Failed to re-apply the migrations");
        conn.batch_execute("PRAGMA foreign_keys = ON")
            .expect("Failed to enable foreign keys");
    }

    /// A second project in the fixture's organization with one report, so that its ID
    /// differs from the organization's and the report's differs from the job's.
    fn create_second_project_report(conn: &mut DbConnection) -> (ProjectId, ReportId) {
        use crate::test_util::{create_report, create_testbed};

        diesel::sql_query(
            "INSERT INTO project (uuid, organization_id, name, slug, visibility, bmf_version, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000060', 1, 'Second', 'second', 0, 0, 0, 0)",
        )
        .execute(conn)
        .expect("Failed to insert the second project");
        let project_id: ProjectId = diesel::select(last_insert_rowid())
            .get_result(conn)
            .expect("Failed to get the second project's ID");
        let branch = create_branch_with_head(
            conn,
            project_id,
            "00000000-0000-0000-0000-000000000061",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000062",
        );
        let testbed_id = create_testbed(
            conn,
            project_id,
            "00000000-0000-0000-0000-000000000063",
            "localhost",
            "localhost",
        );
        let version_id = create_version(
            conn,
            project_id,
            "00000000-0000-0000-0000-000000000064",
            0,
            None,
        );
        create_head_version(conn, branch.head_id, version_id);
        let report_id = create_report(
            conn,
            "00000000-0000-0000-0000-000000000065",
            project_id,
            branch.head_id,
            version_id,
            testbed_id,
        );
        (project_id, report_id)
    }

    /// Another report of the same project, head, version, and testbed as `report_id`.
    fn create_sibling_report(conn: &mut DbConnection, report_id: ReportId) -> ReportId {
        diesel::sql_query(
            "INSERT INTO report (uuid, project_id, head_id, version_id, testbed_id, adapter,
                    start_time, end_time, created)
                SELECT lower(hex(randomblob(16))), project_id, head_id, version_id, testbed_id,
                    adapter, start_time, end_time, created
                FROM report WHERE id = ?",
        )
        .bind::<diesel::sql_types::Integer, _>(report_id)
        .execute(conn)
        .expect("Failed to insert a sibling report");
        diesel::select(last_insert_rowid())
            .get_result(conn)
            .expect("Failed to get the sibling report's ID")
    }

    /// Every job's ID, report, project, and status, in ID order.
    fn job_rows(conn: &mut DbConnection) -> Vec<(JobId, ReportId, ProjectId, JobStatus)> {
        schema::job::table
            .order(schema::job::id.asc())
            .select((
                schema::job::id,
                schema::job::report_id,
                schema::job::project_id,
                schema::job::status,
            ))
            .load(conn)
            .expect("Failed to load jobs")
    }

    /// Every column of every job, in ID order, as one JSON array per row.
    fn job_rows_whole(conn: &mut DbConnection) -> Vec<String> {
        #[derive(diesel::QueryableByName)]
        struct Row {
            #[diesel(sql_type = diesel::sql_types::Text)]
            row: String,
        }

        diesel::sql_query(
            "SELECT json_array(id, uuid, organization_id, project_id, report_id, source_ip, spec_id,
                config, timeout, priority, status, runner_id, claimed, started, completed,
                last_heartbeat, last_billed_minute, created, modified) AS row
                FROM job ORDER BY id",
        )
        .load::<Row>(conn)
        .expect("Failed to load jobs")
        .into_iter()
        .map(|row| row.row)
        .collect()
    }

    #[test]
    fn migration_backfills_each_jobs_project_from_its_report() {
        use crate::test_util::create_job_fixture;

        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        let (second_project_id, second_project_report_id) = create_second_project_report(&mut conn);
        // Siblings, so that no job's report ID matches its project's ID or its own ID.
        let second_report_id = create_sibling_report(&mut conn, second_project_report_id);
        let first_report_id = create_sibling_report(&mut conn, fixture.report_id);

        around_job_project_migration(&mut conn, |conn| {
            // The job table as it stood before the migration, without `project_id`.
            for report_id in [second_report_id, first_report_id] {
                diesel::sql_query(
                    "INSERT INTO job (uuid, report_id, organization_id, source_ip, spec_id, config, created, modified)
                        VALUES (lower(hex(randomblob(16))), ?, ?, '127.0.0.1', ?, '{}', 0, 0)",
                )
                .bind::<diesel::sql_types::Integer, _>(report_id)
                .bind::<diesel::sql_types::Integer, _>(fixture.organization_id)
                .bind::<diesel::sql_types::Integer, _>(fixture.spec_id)
                .execute(conn)
                .expect("Failed to insert a job without a project");
            }
        });

        let expected = vec![
            (second_report_id, second_project_id),
            (first_report_id, fixture.project_id),
        ];
        let rows = job_rows(&mut conn);
        for ((job_id, _, _, _), (report_id, project_id)) in rows.iter().zip(&expected) {
            let report_id = i32::from(*report_id);
            assert_ne!(
                report_id,
                i32::from(*project_id),
                "a report's ID is not its project's"
            );
            assert_ne!(
                report_id,
                i32::from(*job_id),
                "a report's ID is not its job's"
            );
        }
        let projects: Vec<(ReportId, ProjectId)> = rows
            .into_iter()
            .map(|(_, report_id, project_id, _)| (report_id, project_id))
            .collect();
        assert_eq!(projects, expected, "every job carries its report's project");
    }

    #[test]
    fn migration_down_and_up_keeps_every_job() {
        use crate::test_util::{JobFixture, create_job, create_job_fixture};

        let mut conn = setup_test_db();
        let fixture = create_job_fixture(&mut conn);
        // A report whose ID is neither its project's nor either job's.
        create_sibling_report(&mut conn, fixture.report_id);
        let report_id = create_sibling_report(&mut conn, fixture.report_id);
        let fixture = JobFixture {
            report_id,
            ..fixture
        };
        create_job(&mut conn, fixture, JobStatus::Pending);
        create_job(&mut conn, fixture, JobStatus::Running);
        // Every nullable column holds a value on one of the jobs.
        diesel::sql_query(
            "INSERT INTO runner (uuid, name, slug, key_hash, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000070', 'Runner', 'runner', 'hash', 0, 0)",
        )
        .execute(&mut conn)
        .expect("Failed to insert a runner");
        diesel::sql_query(
            "UPDATE job SET runner_id = last_insert_rowid(), claimed = 1, started = 2,
                completed = 3, last_heartbeat = 4, last_billed_minute = 5 WHERE status = 2",
        )
        .execute(&mut conn)
        .expect("Failed to fill the running job");
        let before = job_rows_whole(&mut conn);

        around_job_project_migration(&mut conn, |_| {});

        assert_eq!(
            job_rows_whole(&mut conn),
            before,
            "down and up keep every column of every job"
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

impl UpdateJob {
    /// Terminal status + completed timestamp.
    pub fn terminate(status: JobStatus, now: DateTime) -> Self {
        debug_assert!(
            status.has_run(),
            "terminate() called with non-terminal status: {status:?}"
        );
        Self {
            status: Some(status),
            completed: Some(Some(now)),
            modified: Some(now),
            ..Default::default()
        }
    }

    /// Status-only transition (no completed timestamp).
    pub fn set_status(status: JobStatus, now: DateTime) -> Self {
        Self {
            status: Some(status),
            modified: Some(now),
            ..Default::default()
        }
    }

    /// Update heartbeat timestamp only.
    pub fn heartbeat(now: DateTime) -> Self {
        Self {
            last_heartbeat: Some(Some(now)),
            modified: Some(now),
            ..Default::default()
        }
    }

    /// Update heartbeat timestamp and billing minute together.
    pub fn heartbeat_with_billing(now: DateTime, billed_minute: i32) -> Self {
        Self {
            last_heartbeat: Some(Some(now)),
            last_billed_minute: Some(Some(billed_minute)),
            modified: Some(now),
            ..Default::default()
        }
    }

    /// Update billing minute only (no heartbeat timestamp).
    ///
    /// Used by final billing when the job is already in a terminal state.
    pub fn final_billing(billed_minute: i32, now: DateTime) -> Self {
        Self {
            last_billed_minute: Some(Some(billed_minute)),
            modified: Some(now),
            ..Default::default()
        }
    }

    /// Transition to Running (Claimed -> Running).
    pub fn start(now: DateTime) -> Self {
        Self {
            status: Some(JobStatus::Running),
            started: Some(Some(now)),
            last_heartbeat: Some(Some(now)),
            modified: Some(now),
            ..Default::default()
        }
    }

    /// Claim a pending job.
    pub fn claim(runner_id: RunnerId, now: DateTime) -> Self {
        Self {
            status: Some(JobStatus::Claimed),
            runner_id: Some(Some(runner_id)),
            claimed: Some(Some(now)),
            last_heartbeat: Some(Some(now)),
            modified: Some(now),
            ..Default::default()
        }
    }

    /// Apply this changeset to a job unconditionally (no status filter).
    pub fn execute(&self, conn: &mut DbConnection, job_id: JobId) -> QueryResult<usize> {
        diesel::update(schema::job::table.filter(schema::job::id.eq(job_id)))
            .set(self)
            .execute(conn)
    }

    /// Apply this changeset to a job, filtering on the expected current status.
    ///
    /// Returns the number of rows updated (0 if the job was not in the expected status).
    pub fn execute_if_status(
        &self,
        conn: &mut DbConnection,
        job_id: JobId,
        expected_status: JobStatus,
    ) -> QueryResult<usize> {
        diesel::update(
            schema::job::table
                .filter(schema::job::id.eq(job_id))
                .filter(schema::job::status.eq(expected_status)),
        )
        .set(self)
        .execute(conn)
    }

    /// Apply this changeset to a job, filtering on either of two expected statuses.
    ///
    /// Returns the number of rows updated (0 if the job was not in either expected status).
    pub fn execute_if_either_status(
        &self,
        conn: &mut DbConnection,
        job_id: JobId,
        status_a: JobStatus,
        status_b: JobStatus,
    ) -> QueryResult<usize> {
        diesel::update(
            schema::job::table
                .filter(schema::job::id.eq(job_id))
                .filter(
                    schema::job::status
                        .eq(status_a)
                        .or(schema::job::status.eq(status_b)),
                ),
        )
        .set(self)
        .execute(conn)
    }

    /// Apply this changeset to a job, filtering on any of the expected statuses.
    ///
    /// Returns the number of rows updated (0 if the job was not in any expected status).
    pub fn execute_if_any_status(
        &self,
        conn: &mut DbConnection,
        job_id: JobId,
        expected_statuses: &[JobStatus],
    ) -> QueryResult<usize> {
        diesel::update(
            schema::job::table
                .filter(schema::job::id.eq(job_id))
                .filter(schema::job::status.eq_any(expected_statuses)),
        )
        .set(self)
        .execute(conn)
    }
}

/// How long a job may go without a heartbeat, and how long past its own timeout it may run.
#[derive(Debug, Clone, Copy)]
pub struct JobTimeout {
    pub heartbeat: std::time::Duration,
    pub grace_period: std::time::Duration,
}

impl JobTimeout {
    /// Spawn a background task that marks a job as unknown if no heartbeat is received
    /// within the heartbeat timeout. This handles both "disconnected runner" recovery
    /// and startup recovery for in-flight jobs.
    ///
    /// Also enforces job timeout: if the job has been running longer than its configured
    /// `timeout` plus `job_timeout_grace_period`, it is marked as Canceled so the runner
    /// receives a Cancel event on its next heartbeat. An unknown job hears no heartbeats,
    /// so the task then waits for the job's deadline and checks again.
    pub fn spawn(
        self,
        log: Logger,
        connection: Arc<Mutex<DbConnection>>,
        job_id: JobId,
        heartbeat_tasks: &crate::context::HeartbeatTasks,
        clock: bencher_json::Clock,
        callbacks: Callbacks,
    ) {
        let Self {
            heartbeat: heartbeat_timeout,
            grace_period: job_timeout_grace_period,
        } = self;
        let join_handle = tokio::spawn(async move {
            let heartbeat_timeout_secs =
                i64::try_from(heartbeat_timeout.as_secs()).unwrap_or(i64::MAX);
            let mut wait = heartbeat_timeout;
            loop {
                tokio::time::sleep(wait).await;

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
                if job.status.has_run() {
                    return;
                }

                // Check job timeout: if running longer than timeout + grace period, cancel it
                if check_job_timeout(
                    &log,
                    &job,
                    job_timeout_grace_period,
                    &mut conn,
                    &clock,
                    &callbacks,
                ) {
                    return;
                }

                let now = clock.now();

                // If the runner reconnected and sent a recent heartbeat, check again once it could be stale
                if let Some(last_heartbeat) = job.last_heartbeat {
                    let elapsed = (now.timestamp() - last_heartbeat.timestamp()).max(0);
                    if elapsed < heartbeat_timeout_secs {
                        let secs = u64::try_from(heartbeat_timeout_secs - elapsed).unwrap_or(0);
                        wait = std::time::Duration::from_secs(secs.max(1));
                        continue;
                    }
                }

                if matches!(job.status, JobStatus::Claimed | JobStatus::Running) {
                    slog::warn!(log, "Heartbeat timeout, marking job as unknown"; "job_id" => ?job_id);
                    let update = UpdateJob::set_status(JobStatus::Unknown, now);
                    match update.execute_if_either_status(
                        &mut conn,
                        job_id,
                        JobStatus::Claimed,
                        JobStatus::Running,
                    ) {
                        Ok(0) => {
                            slog::info!(log, "Heartbeat timeout: job already changed state"; "job_id" => ?job_id);
                        },
                        Ok(_) => {
                            #[cfg(feature = "otel")]
                            {
                                bencher_otel::ApiMeter::increment(
                                    bencher_otel::ApiCounter::RunnerHeartbeatTimeout,
                                );
                                bencher_otel::ApiMeter::increment(
                                    bencher_otel::ApiCounter::RunnerJobUpdate(
                                        bencher_otel::JobStatusKind::Unknown,
                                    ),
                                );
                            }
                        },
                        Err(e) => {
                            slog::error!(log, "Failed to mark job as unknown"; "job_id" => ?job_id, "error" => %e);
                        },
                    }
                }

                let Some(until) = until_deadline(&job, job_timeout_grace_period, now) else {
                    slog::warn!(log, "Job has neither a claimed nor a started timestamp, not scheduling its deadline"; "job_id" => ?job_id);
                    return;
                };
                wait = until;
            }
        });

        heartbeat_tasks.insert(job_id, join_handle.abort_handle());
    }
}

/// Load the jobs startup recovery arms heartbeat timeouts for.
///
/// Written as `OR`, not `eq_any`: a bound `IN` list never matches a partial index,
/// so `eq_any` scans `job` instead of searching `index_job_in_flight`.
pub fn in_flight_jobs(conn: &mut DbConnection) -> QueryResult<Vec<QueryJob>> {
    schema::job::table
        .filter(
            schema::job::status
                .eq(JobStatus::Claimed)
                .or(schema::job::status.eq(JobStatus::Running))
                .or(schema::job::status.eq(JobStatus::Unknown)),
        )
        .load(conn)
}

/// Mark jobs stuck in `Claimed` status that were claimed longer ago than the
/// heartbeat timeout as `Unknown`. These are orphaned: the runner claimed them but never
/// transitioned them to `Running` (e.g., the runner crashed after claiming).
/// An `Unknown` job still accepts the runner's result, and its deadline ends it otherwise.
///
/// Returns the number of jobs marked `Unknown`.
pub fn mark_orphaned_claimed_jobs_unknown(
    log: &Logger,
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

    let mut marked = 0;
    for job in &orphaned_jobs {
        if job.claimed.is_none() {
            // Claimed but no timestamp should not happen; mark it anyway
            slog::warn!(log, "Claimed job has no claimed timestamp"; "job_id" => ?job.id);
        }

        slog::warn!(log, "Marking orphaned claimed job as unknown"; "job_id" => ?job.id);
        let now = clock.now();
        let update = UpdateJob::set_status(JobStatus::Unknown, now);

        match update.execute_if_status(conn, job.id, JobStatus::Claimed) {
            Ok(0) => {
                slog::info!(log, "Orphaned job already changed state"; "job_id" => ?job.id);
            },
            Ok(_) => {
                marked += 1;
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
                    bencher_otel::JobStatusKind::Unknown,
                ));
            },
            Err(e) => {
                slog::error!(log, "Failed to mark orphaned job as unknown"; "job_id" => ?job.id, "error" => %e);
            },
        }
    }

    if marked > 0 {
        slog::info!(log, "Marked orphaned claimed jobs as unknown"; "count" => marked);
    }

    marked
}

/// Check if a job has exceeded its timeout + grace period.
/// If so, mark it as canceled and return `true` to indicate the caller should stop.
fn check_job_timeout(
    log: &Logger,
    job: &QueryJob,
    job_timeout_grace_period: std::time::Duration,
    conn: &mut DbConnection,
    clock: &bencher_json::Clock,
    callbacks: &Callbacks,
) -> bool {
    let now = clock.now();
    let Some((elapsed, limit)) = deadline_elapsed(job, job_timeout_grace_period, now) else {
        return false;
    };
    if elapsed <= limit {
        return false;
    }
    slog::warn!(log, "Job timeout exceeded, marking as canceled"; "job_id" => ?job.id, "elapsed" => elapsed, "limit" => limit);
    let cancel_update = UpdateJob::terminate(JobStatus::Canceled, now);
    // Use status filter to avoid TOCTOU race
    match cancel_update.execute_if_any_status(
        conn,
        job.id,
        &[JobStatus::Claimed, JobStatus::Running, JobStatus::Unknown],
    ) {
        Ok(updated) if updated > 0 => {
            callbacks.fire(log, job.id);
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

/// Seconds elapsed since the job's deadline clock started, and the limit: its timeout plus
/// the grace period. `None` for a job with neither a claimed nor a started timestamp.
fn deadline_elapsed(
    job: &QueryJob,
    job_timeout_grace_period: std::time::Duration,
    now: DateTime,
) -> Option<(i64, i64)> {
    let start = job.deadline_start()?;
    let elapsed = (now.timestamp() - start.timestamp()).max(0);
    #[expect(
        clippy::cast_possible_wrap,
        reason = "timeout max i32::MAX + grace period fits in i64"
    )]
    let limit =
        u64::from(u32::from(job.timeout)) as i64 + job_timeout_grace_period.as_secs() as i64;
    Some((elapsed, limit))
}

/// Time left until the job is past its deadline, and at least one second.
fn until_deadline(
    job: &QueryJob,
    job_timeout_grace_period: std::time::Duration,
    now: DateTime,
) -> Option<std::time::Duration> {
    let (elapsed, limit) = deadline_elapsed(job, job_timeout_grace_period, now)?;
    let secs = u64::try_from(limit.saturating_sub(elapsed).saturating_add(1)).unwrap_or(1);
    Some(std::time::Duration::from_secs(secs.max(1)))
}

/// Mark an orphaned Completed job as Unknown and arm its heartbeat timeout.
///
/// Called when a Completed job has no stored output in OCI storage,
/// meaning its results were lost. Transitions to Unknown, preserving the original completed timestamp.
///
/// **Why Unknown instead of leaving as Completed?**
/// If the job stays in Completed, the runner's retry of the Completed message
/// is silently dropped by `handle_completed()` as an "idempotent duplicate"
/// (it sees Completed status and returns `Ok`). By marking as Unknown, the
/// runner's retry triggers an Unknown→Completed transition, which goes through
/// the full processing path (store output → process results → Processed).
/// If no retry arrives, the job's deadline ends it.
async fn mark_orphaned_completed_unknown(log: &Logger, context: &ApiContext, job: &QueryJob) {
    let now = context.clock.now();
    let unknown_update = UpdateJob::set_status(JobStatus::Unknown, now);
    match unknown_update.execute_if_status(write_conn!(context), job.id, JobStatus::Completed) {
        Ok(updated) if updated > 0 => {
            slog::info!(log, "Marked orphaned completed job as Unknown"; "job_id" => ?job.id);
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
                bencher_otel::JobStatusKind::Unknown,
            ));
            JobTimeout {
                heartbeat: context.heartbeat_timeout,
                grace_period: context.job_timeout_grace_period,
            }
            .spawn(
                log.clone(),
                context.database.connection.clone(),
                job.id,
                &context.heartbeat_tasks,
                context.clock.clone(),
                context.callbacks.clone(),
            );
        },
        Ok(_) => {
            slog::info!(log, "Job already changed state during orphan recovery"; "job_id" => ?job.id);
        },
        Err(e) => {
            slog::error!(log, "Failed to mark orphaned completed job as Unknown"; "job_id" => ?job.id, "error" => %e);
        },
    }
}

/// Reprocess jobs stuck in `Completed` status on startup.
///
/// These are jobs where output was stored but result processing (adapter parsing,
/// metrics, alerts) failed or was interrupted. Fetches stored output from OCI
/// storage, runs `process_results`, and transitions to `Processed` on success.
pub async fn reprocess_completed_jobs(log: &Logger, context: &ApiContext) {
    let completed_jobs: Vec<QueryJob> = {
        match schema::job::table
            .filter(schema::job::status.eq(JobStatus::Completed))
            .load(write_conn!(context))
        {
            Ok(jobs) => jobs,
            Err(e) => {
                slog::error!(log, "Failed to query completed jobs for reprocessing"; "error" => %e);
                return;
            },
        }
    };

    let count = completed_jobs.len();
    if count == 0 {
        return;
    }
    slog::info!(log, "Reprocessing {count} completed job(s)");

    for job in &completed_jobs {
        reprocess_single_completed_job(log, context, job).await;
    }
}

/// Attempt to reprocess a single Completed job.
///
/// Fetches stored output, runs result processing, and transitions state accordingly.
async fn reprocess_single_completed_job(log: &Logger, context: &ApiContext, job: &QueryJob) {
    let output = match context
        .oci_storage()
        .job_output()
        .get(job.config.project, job.uuid)
        .await
    {
        Ok(Some(output)) => output,
        Ok(None) => {
            // See mark_orphaned_completed_unknown doc comment.
            slog::warn!(log, "No stored output for completed job, marking as Unknown"; "job_id" => ?job.id);
            mark_orphaned_completed_unknown(log, context, job).await;
            return;
        },
        Err(e) => {
            // See mark_orphaned_completed_unknown doc comment.
            slog::error!(log, "Failed to fetch job output for reprocessing, marking as Unknown"; "job_id" => ?job.id, "error" => %e);
            mark_orphaned_completed_unknown(log, context, job).await;
            return;
        },
    };

    let now = context.clock.now();
    if let Err(e) = job.process_results(log, context, output.results, now).await {
        slog::warn!(log, "Failed to reprocess job results, marking as Failed"; "job_id" => ?job.id, "error" => %e);
        let failed_update = UpdateJob::set_status(JobStatus::Failed, now);
        match failed_update.execute_if_status(write_conn!(context), job.id, JobStatus::Completed) {
            Ok(updated) if updated > 0 => {
                slog::info!(log, "Marked failed reprocessing job as Failed"; "job_id" => ?job.id);
                context.callbacks.fire(log, job.id);
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
                    bencher_otel::JobStatusKind::Failed,
                ));
            },
            Ok(_) => {
                slog::info!(log, "Job already changed state during reprocessing failure"; "job_id" => ?job.id);
            },
            Err(e) => {
                slog::error!(log, "Failed to mark reprocessing job as Failed"; "job_id" => ?job.id, "error" => %e);
            },
        }
        return;
    }

    let processed_update = UpdateJob::set_status(JobStatus::Processed, now);
    match processed_update.execute_if_status(write_conn!(context), job.id, JobStatus::Completed) {
        Ok(updated) if updated > 0 => {
            slog::info!(log, "Reprocessed completed job"; "job_id" => ?job.id);
            context.callbacks.fire(log, job.id);
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
                bencher_otel::JobStatusKind::Processed,
            ));
        },
        Ok(_) => {
            slog::info!(log, "Job already changed state during reprocessing"; "job_id" => ?job.id);
        },
        Err(e) => {
            slog::error!(log, "Failed to mark reprocessed job as processed"; "job_id" => ?job.id, "error" => %e);
        },
    }
}
