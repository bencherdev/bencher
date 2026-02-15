pub(crate) mod websocket;

use std::time::Duration;

use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseOk};
use bencher_json::{
    DateTime, JobPriority, JobStatus, JsonClaimJob, JsonClaimedJob, JsonSpec, RunnerResourceId,
};
use bencher_schema::{
    context::ApiContext,
    error::resource_conflict_err,
    model::{
        runner::{QueryJob, UpdateJob},
        spec::QuerySpec,
    },
    schema,
};
use bencher_token::OciAction;
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, OptionalExtension as _, QueryDsl as _,
    RunQueryDsl as _, dsl::exists, dsl::not,
};
use dropshot::{HttpError, Path, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::runner_token::RunnerToken;

// Table aliases for correlated subqueries in the claim query.
// job_org is used to check if an org already has a running job (Free tier limit).
// job_ip is used to check if a source IP already has a running job (Unclaimed tier limit).
diesel::alias!(schema::job as job_org: JobOrg);
diesel::alias!(schema::job as job_ip: JobIp);

/// Priority threshold for Enterprise/Team tiers (unlimited concurrency)
const PRIORITY_UNLIMITED: JobPriority = JobPriority::Team;
/// Priority threshold for Free tier (1 per org concurrency)
const PRIORITY_FREE: JobPriority = JobPriority::Free;

/// Poll interval for long-polling (1 second)
const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Default poll timeout (30 seconds)
const DEFAULT_POLL_TIMEOUT: u32 = 30;

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
) -> Result<ResponseOk<Option<JsonClaimedJob>>, HttpError> {
    let context = rqctx.context();
    let path_params = path_params.into_inner();
    let runner_token = RunnerToken::from_request(&rqctx, &path_params.runner).await?;

    // Per-runner rate limiting
    #[cfg(feature = "plus")]
    context
        .rate_limiting
        .runner_request(runner_token.runner_uuid)?;

    let json = claim_job_inner(context, runner_token, body.into_inner()).await?;
    Ok(Post::auth_response_ok(json))
}

async fn claim_job_inner(
    context: &ApiContext,
    runner_token: RunnerToken,
    claim_request: JsonClaimJob,
) -> Result<Option<JsonClaimedJob>, HttpError> {
    let poll_timeout = claim_request
        .poll_timeout
        .map_or(DEFAULT_POLL_TIMEOUT, u32::from);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(u64::from(poll_timeout));

    loop {
        // Try to claim a job (connection is released when function returns)
        if let Some(json_job) = try_claim_job(context, &runner_token).await? {
            return Ok(Some(json_job));
        }

        // Check if we've exceeded the timeout
        if tokio::time::Instant::now() >= deadline {
            return Ok(None);
        }

        // Wait before trying again
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Attempt to claim a pending job with tier-based concurrency limits.
///
/// Priority tiers:
/// - Enterprise / Team: Unlimited concurrent jobs
/// - Free: 1 concurrent job per organization
/// - Unclaimed: 1 concurrent job per source IP
///
/// Returns `Ok(Some(job))` if a job was claimed, `Ok(None)` if no eligible jobs available.
/// OCI runner token TTL: 10 minutes (enough for image pull, short enough to limit exposure)
const OCI_RUNNER_TOKEN_TTL: u32 = 600;

async fn try_claim_job(
    context: &ApiContext,
    runner_token: &RunnerToken,
) -> Result<Option<JsonClaimedJob>, HttpError> {
    use schema::job::dsl::{created, id, organization_id, priority, source_ip, status};

    // Tier 1: Enterprise/Team (priority >= 200) - no concurrency limit
    let tier_unlimited = priority.ge(PRIORITY_UNLIMITED);

    // Tier 2: Free (priority 100-199) - one concurrent job per organization
    // Block if the same org already has a Claimed or Running job
    let tier_free_eligible = priority
        .ge(PRIORITY_FREE)
        .and(priority.lt(PRIORITY_UNLIMITED))
        .and(not(exists(
            job_org
                .filter(
                    job_org
                        .field(status)
                        .eq(JobStatus::Claimed)
                        .or(job_org.field(status).eq(JobStatus::Running)),
                )
                .filter(job_org.field(organization_id).eq(organization_id)),
        )));

    // Tier 3: Unclaimed (priority < 100) - one concurrent job per source IP
    // Block if the same source_ip already has a Claimed or Running job
    let tier_unclaimed_eligible = priority.lt(PRIORITY_FREE).and(not(exists(
        job_ip
            .filter(
                job_ip
                    .field(status)
                    .eq(JobStatus::Claimed)
                    .or(job_ip.field(status).eq(JobStatus::Running)),
            )
            .filter(job_ip.field(source_ip).eq(source_ip)),
    )));

    // Combined eligibility: any tier condition passes
    let eligible = tier_unlimited
        .or(tier_free_eligible)
        .or(tier_unclaimed_eligible);

    // Spec filter: only claim jobs whose spec_id matches one of the runner's specs
    let spec_filter = schema::job::spec_id.eq_any(
        schema::runner_spec::table
            .filter(schema::runner_spec::runner_id.eq(runner_token.runner_id))
            .select(schema::runner_spec::spec_id),
    );

    // Acquire write lock for the entire read-check-update to prevent TOCTOU races
    // where concurrent runners could bypass concurrency limits.
    let mut conn = context.database.connection.lock().await;

    // Find the highest-priority eligible pending job matching this runner's specs
    let pending_job: Option<QueryJob> = schema::job::table
        .filter(status.eq(JobStatus::Pending))
        .filter(eligible)
        .filter(spec_filter)
        .order((priority.desc(), created.asc(), id.asc()))
        .first(&mut *conn)
        .optional()
        .map_err(|e| HttpError::for_internal_error(format!("Failed to query pending jobs: {e}")))?;

    let Some(query_job) = pending_job else {
        return Ok(None);
    };

    // Claim the job under the same lock
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
            .filter(id.eq(query_job.id))
            .filter(status.eq(JobStatus::Pending)),
    )
    .set(&update_job)
    .execute(&mut *conn)
    .map_err(resource_conflict_err!(Job, query_job))?;

    // Look up the spec before releasing the lock
    let json_spec = (updated > 0)
        .then(|| QuerySpec::get(&mut conn, query_job.spec_id).map(QuerySpec::into_json))
        .transpose()?;

    // Release the lock before doing non-DB work
    drop(conn);

    if let Some(json_spec) = json_spec {
        Ok(Some(build_claimed_job(
            context,
            query_job,
            runner_token,
            json_spec,
        )?))
    } else {
        // Defensive: the UPDATE matched 0 rows despite SELECT finding a pending job.
        // Under the current single-writer lock this should not happen, but we
        // handle it gracefully by returning None (no job claimed this iteration).
        Ok(None)
    }
}

fn build_claimed_job(
    context: &ApiContext,
    query_job: QueryJob,
    runner_token: &RunnerToken,
    json_spec: JsonSpec,
) -> Result<JsonClaimedJob, HttpError> {
    #[cfg(feature = "otel")]
    {
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobClaim);

        // Record queue duration (time from creation to claim)
        let now = DateTime::now();
        #[expect(
            clippy::cast_precision_loss,
            reason = "queue duration in seconds fits in f64 mantissa"
        )]
        let queue_duration_secs =
            ((now.timestamp() - query_job.created.timestamp()) as f64).max(0.0);
        let tier = bencher_otel::PriorityTier::from_priority(query_job.priority.into());
        bencher_otel::ApiMeter::record(
            bencher_otel::ApiHistogram::JobQueueDuration(tier),
            queue_duration_secs,
        );
    }

    let timeout = query_job.config.timeout;
    let oci_token = context
        .token_key
        .new_oci_runner(
            runner_token.runner_uuid,
            OCI_RUNNER_TOKEN_TTL,
            Some(query_job.config.project.to_string()),
            vec![OciAction::Pull],
        )
        .map_err(|e| {
            HttpError::for_internal_error(format!("Failed to generate OCI runner token: {e}"))
        })?;

    Ok(JsonClaimedJob {
        uuid: query_job.uuid,
        spec: json_spec,
        config: query_job.config,
        oci_token,
        timeout,
        created: query_job.created,
    })
}
