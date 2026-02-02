use std::time::{Duration, Instant};

use bencher_endpoint::{CorsResponse, Endpoint, Patch, Post, ResponseOk};
use bencher_json::{
    DateTime, JobStatus, JobUuid, JsonClaimJob, JsonJob, JsonUpdateJob, JsonUpdateJobResponse,
    RunnerResourceId,
};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{forbidden_error, resource_conflict_err, resource_not_found_err},
    model::runner::{QueryJob, UpdateJob},
    schema, write_conn,
};
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::runner_token::RunnerToken;

/// Poll interval for long-polling (1 second)
const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Default poll timeout (30 seconds)
const DEFAULT_POLL_TIMEOUT: u32 = 30;
/// Minimum poll timeout (1 second)
const MIN_POLL_TIMEOUT: u32 = 1;
/// Maximum poll timeout (60 seconds)
const MAX_POLL_TIMEOUT: u32 = 60;

#[derive(Deserialize, JsonSchema)]
pub struct RunnerJobsParams {
    /// The slug or UUID for a runner.
    pub runner: RunnerResourceId,
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/runners/{runner}/jobs",
    tags = ["runners"]
}]
pub async fn runner_jobs_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<RunnerJobsParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Claim a job
///
/// Long-poll to claim a pending job for execution.
/// Authenticated via runner token.
/// Returns a job if one is available, or empty response on timeout.
#[endpoint {
    method = POST,
    path = "/v0/runners/{runner}/jobs",
    tags = ["runners"]
}]
pub async fn runner_jobs_post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<RunnerJobsParams>,
    body: TypedBody<JsonClaimJob>,
) -> Result<ResponseOk<Option<JsonJob>>, HttpError> {
    let context = rqctx.context();

    // IP-based rate limiting
    #[cfg(feature = "plus")]
    if let Some(remote_ip) =
        bencher_schema::context::RateLimiting::remote_ip(&rqctx.log, rqctx.request.headers())
    {
        context.rate_limiting.public_request(remote_ip)?;
    }

    let path_params = path_params.into_inner();
    let runner_token = RunnerToken::from_request(&rqctx, &path_params.runner).await?;
    let json = claim_job_inner(context, runner_token, body.into_inner()).await?;
    Ok(Post::auth_response_ok(json))
}

async fn claim_job_inner(
    context: &ApiContext,
    runner_token: RunnerToken,
    claim_request: JsonClaimJob,
) -> Result<Option<JsonJob>, HttpError> {
    // Cap poll timeout at 60 seconds
    let poll_timeout = claim_request
        .poll_timeout
        .unwrap_or(DEFAULT_POLL_TIMEOUT)
        .clamp(MIN_POLL_TIMEOUT, MAX_POLL_TIMEOUT);
    let deadline = Instant::now() + Duration::from_secs(u64::from(poll_timeout));

    loop {
        // Try to claim a job (connection is released when function returns)
        if let Some(json_job) = try_claim_job(context, &runner_token).await? {
            return Ok(Some(json_job));
        }

        // Check if we've exceeded the timeout
        if Instant::now() >= deadline {
            return Ok(None);
        }

        // Wait before trying again
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Attempt to claim a pending job.
/// Returns `Ok(Some(job_id))` if a job was claimed, `Ok(None)` if no jobs available.
async fn try_claim_job(
    context: &ApiContext,
    runner_token: &RunnerToken,
) -> Result<Option<JsonJob>, HttpError> {
    let conn = write_conn!(context);

    // Try to claim a pending job (ordered by priority DESC, created ASC)
    let pending_job: Option<QueryJob> = schema::job::table
        .filter(schema::job::status.eq(JobStatus::Pending))
        .order((schema::job::priority.desc(), schema::job::created.asc()))
        .first(conn)
        .optional()
        .map_err(resource_not_found_err!(Job))?;

    let Some(query_job) = pending_job else {
        return Ok(None);
    };

    // Atomically claim the job
    let now = DateTime::now();
    let update_job = UpdateJob {
        status: Some(JobStatus::Claimed),
        runner_id: Some(Some(runner_token.runner_id)),
        claimed: Some(Some(now)),
        last_heartbeat: Some(Some(now)),
        modified: Some(now),
        ..Default::default()
    };

    let updated = diesel::update(
        schema::job::table
            .filter(schema::job::id.eq(query_job.id))
            .filter(schema::job::status.eq(JobStatus::Pending)),
    )
    .set(&update_job)
    .execute(conn)
    .map_err(resource_conflict_err!(Job, query_job))?;

    if updated > 0 {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobClaim);
        // Return JSON with updated status (query_job still has Pending status)
        Ok(Some(JsonJob {
            uuid: query_job.uuid,
            status: JobStatus::Claimed,
            runner: Some(runner_token.runner_uuid),
            claimed: Some(now),
            started: None,
            completed: None,
            exit_code: None,
            created: query_job.created,
            modified: now,
        }))
    } else {
        // Job was claimed by another runner
        Ok(None)
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct RunnerJobParams {
    /// The slug or UUID for a runner.
    pub runner: RunnerResourceId,
    /// The UUID for a job.
    pub job: JobUuid,
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/runners/{runner}/jobs/{job}",
    tags = ["runners"]
}]
pub async fn runner_job_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<RunnerJobParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Patch.into()]))
}

/// Update job status
///
/// Update the status of a job being executed.
/// Authenticated via runner token.
/// Used to mark jobs as running, completed, or failed.
#[endpoint {
    method = PATCH,
    path = "/v0/runners/{runner}/jobs/{job}",
    tags = ["runners"]
}]
pub async fn runner_job_patch(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<RunnerJobParams>,
    body: TypedBody<JsonUpdateJob>,
) -> Result<ResponseOk<JsonUpdateJobResponse>, HttpError> {
    let path_params = path_params.into_inner();
    let runner_token = RunnerToken::from_request(&rqctx, &path_params.runner).await?;
    let json = update_job_inner(
        rqctx.context(),
        runner_token,
        path_params.job,
        body.into_inner(),
    )
    .await?;
    Ok(Patch::auth_response_ok(json))
}

async fn update_job_inner(
    context: &ApiContext,
    runner_token: RunnerToken,
    job_uuid: JobUuid,
    update_request: JsonUpdateJob,
) -> Result<JsonUpdateJobResponse, HttpError> {
    let job = QueryJob::from_uuid(auth_conn!(context), job_uuid)?;

    // Verify this runner owns this job
    if job.runner_id != Some(runner_token.runner_id) {
        return Err(forbidden_error("Job is not assigned to this runner"));
    }

    // Verify valid state transition
    let valid_transition = matches!(
        (job.status, update_request.status),
        (JobStatus::Claimed, JobStatus::Running | JobStatus::Failed)
            | (JobStatus::Running, JobStatus::Completed | JobStatus::Failed)
    );

    if !valid_transition {
        return Err(forbidden_error(format!(
            "Invalid status transition from {:?} to {:?}",
            job.status, update_request.status
        )));
    }

    let now = DateTime::now();
    let job_update = UpdateJob {
        status: Some(update_request.status),
        started: (update_request.status == JobStatus::Running).then_some(Some(now)),
        completed: update_request.status.is_terminal().then_some(Some(now)),
        exit_code: update_request
            .status
            .is_terminal()
            .then_some(update_request.exit_code),
        modified: Some(now),
        ..Default::default()
    };

    diesel::update(schema::job::table.filter(schema::job::id.eq(job.id)))
        .set(&job_update)
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(Job, job))?;

    #[cfg(feature = "otel")]
    {
        let status_kind = match update_request.status {
            JobStatus::Running => bencher_otel::JobStatusKind::Running,
            JobStatus::Completed => bencher_otel::JobStatusKind::Completed,
            JobStatus::Failed => bencher_otel::JobStatusKind::Failed,
            // These statuses shouldn't reach here due to validation, but handle them
            JobStatus::Pending | JobStatus::Claimed | JobStatus::Canceled => {
                bencher_otel::JobStatusKind::Failed
            },
        };
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(status_kind));
    }

    // Check if job was canceled
    let refreshed_job = QueryJob::get(auth_conn!(context), job.id)?;
    let canceled = refreshed_job.status == JobStatus::Canceled;

    Ok(JsonUpdateJobResponse { canceled })
}
