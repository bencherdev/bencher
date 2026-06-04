use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseCreated};
use bencher_json::{
    JsonNewRun, JsonReport, ProjectResourceId, ProjectSlug, ResourceName, RunContext,
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{bad_request_error, unauthorized_error, with_auth_hint},
    model::{
        project::{
            QueryProject,
            report::{NewRunReport, QueryReport},
        },
        user::{
            actor::{ApiActor, ProjectKeyActor, PubProjectBearerToken},
            public::PublicUser,
        },
    },
    public_conn,
};
#[cfg(feature = "plus")]
use bencher_schema::{
    context::RateLimiting,
    model::{
        project::{report::NewRunJob, testbed::RunTestbed},
        runner::SourceIp,
    },
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};
use slog::Logger;

#[endpoint {
    method = OPTIONS,
    path =  "/v0/run",
    tags = ["run", "reports"]
}]
pub async fn run_options(_rqctx: RequestContext<ApiContext>) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Create a run
///
/// Create a run.
/// The user does not need have an account yet or be authenticated.
/// The project may or may not exist yet.
#[endpoint {
    method = POST,
    path =  "/v0/run",
    tags = ["run", "reports"]
}]
pub async fn run_post(
    rqctx: RequestContext<ApiContext>,
    pub_project_bearer_token: PubProjectBearerToken,
    body: TypedBody<JsonNewRun>,
) -> Result<ResponseCreated<JsonReport>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        pub_project_bearer_token,
    )
    .await?;

    let json = match api_actor {
        ApiActor::ProjectKey(project_key_actor) => post_inner_project_key(
            &rqctx.log,
            rqctx.context(),
            project_key_actor,
            #[cfg(feature = "plus")]
            rqctx.request.headers(),
            body.into_inner(),
        )
        .await
        .map_err(with_auth_hint)?,
        ApiActor::Public(public_user) => post_inner(
            &rqctx.log,
            rqctx.context(),
            &public_user,
            #[cfg(feature = "plus")]
            rqctx.request.headers(),
            body.into_inner(),
        )
        .await
        .map_err(with_auth_hint)?,
    };

    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    public_user: &PublicUser,
    #[cfg(feature = "plus")] headers: &http::HeaderMap,
    json_run: JsonNewRun,
) -> Result<JsonReport, HttpError> {
    match public_user {
        PublicUser::Public(remote_ip) => {
            if let Some(remote_ip) = remote_ip {
                slog::info!(log, "Unclaimed run request from remote IP address"; "remote_ip" => ?remote_ip);
                #[cfg(feature = "plus")]
                context.rate_limiting.unclaimed_run(*remote_ip)?;
            }
        },
        PublicUser::Auth(auth_user) => {
            #[cfg(feature = "plus")]
            context.rate_limiting.claimed_run(auth_user.user.uuid)?;

            slog::info!(log, "Authenticated run request"; "user_uuid" => %auth_user.user.uuid);
        },
    }

    let target_project = TargetProject::find(&json_run);
    let project_name_fn = || project_name(&json_run);
    let project_slug_fn = || project_slug(&json_run);
    let query_project = if let Some(project) = &target_project {
        QueryProject::get_or_create(
            log,
            context,
            public_user,
            project.resource_id(),
            project_name_fn,
        )
        .await?
    } else {
        QueryProject::get_or_create_from_context(
            log,
            context,
            public_user,
            project_name_fn,
            project_slug_fn,
        )
        .await?
    };

    let query_organization = query_project.organization(public_conn!(context, public_user))?;
    let is_claimed = query_organization.is_claimed(public_conn!(context, public_user))?;
    // If the organization is claimed, check permissions
    if is_claimed {
        match public_user {
            PublicUser::Public(remote_ip) => {
                slog::info!(log, "Public user attempted to create a run for a claimed project"; "project" => ?query_project.uuid, "remote_ip" => ?remote_ip);

                return Err(unauthorized_error(format!(
                    "This project ({slug}) has already been claimed. Provide a valid API token (`--token`) or project key (`--key`) to authenticate.",
                    slug = query_project.slug
                )));
            },
            PublicUser::Auth(auth_user) => {
                let auth_user = auth_user.reload(public_conn!(context, public_user))?;
                query_project.try_allowed(&context.rbac, &auth_user, Permission::Create)?;
            },
        }
    // If the organization is not claimed and the user is authenticated, claim it
    } else if let PublicUser::Auth(auth_user) = public_user {
        query_organization
            .claim(log, context, &auth_user.user)
            .await?;
    }

    let api_actor = ApiActor::Public(public_user.clone());
    create_run_report(
        log,
        context,
        &query_project,
        is_claimed,
        &api_actor,
        #[cfg(feature = "plus")]
        headers,
        json_run,
    )
    .await
}

async fn post_inner_project_key(
    log: &Logger,
    context: &ApiContext,
    project_key_actor: ProjectKeyActor,
    #[cfg(feature = "plus")] headers: &http::HeaderMap,
    json_run: JsonNewRun,
) -> Result<JsonReport, HttpError> {
    let query_project = QueryProject::get(auth_conn!(context), project_key_actor.project_id)?;

    // Verify the run targets this project (if explicitly specified or derived from image)
    match TargetProject::find(&json_run) {
        Some(TargetProject::Explicit(project_rid)) => {
            let target = QueryProject::from_resource_id(auth_conn!(context), &project_rid)?;
            project_key_actor.verify_project(target.id)?;
        },
        #[cfg(feature = "plus")]
        Some(TargetProject::Derived(project_rid)) => {
            // A derived repository may not name a project at all, so only
            // verify when it resolves. A bogus image instead fails digest
            // resolution during report creation.
            if let Ok(target) = QueryProject::from_resource_id(auth_conn!(context), &project_rid) {
                project_key_actor.verify_project(target.id)?;
            }
        },
        None => {},
    }

    slog::info!(log, "New run via project key"; "project" => ?query_project.uuid);

    let api_actor = ApiActor::ProjectKey(project_key_actor);
    create_run_report(
        log,
        context,
        &query_project,
        true,
        &api_actor,
        #[cfg(feature = "plus")]
        headers,
        json_run,
    )
    .await
}

async fn create_run_report(
    log: &Logger,
    context: &ApiContext,
    query_project: &QueryProject,
    is_claimed: bool,
    api_actor: &ApiActor,
    #[cfg(feature = "plus")] headers: &http::HeaderMap,
    mut json_run: JsonNewRun,
) -> Result<JsonReport, HttpError> {
    #[cfg(not(feature = "plus"))]
    let _unused = is_claimed;
    #[cfg(feature = "plus")]
    let testbed = if json_run.testbed.is_some() {
        RunTestbed::Explicit
    } else {
        RunTestbed::Derived
    };

    #[cfg(feature = "plus")]
    let spec_reset = json_run.spec_reset.unwrap_or_default();

    #[cfg(feature = "plus")]
    let job = json_run.job.take().map(|run_job| {
        let source_ip = if let Some(ip) = RateLimiting::remote_ip(log, headers) {
            SourceIp::new(ip)
        } else {
            slog::warn!(
                log,
                "Failed to extract remote IP for job; falling back to LOCALHOST"
            );
            SourceIp::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
        };
        NewRunJob {
            is_claimed,
            run_job,
            source_ip,
        }
    });

    #[cfg(feature = "plus")]
    context.rate_limiting.project_run(query_project.uuid)?;

    slog::info!(log, "New run requested"; "project" => ?query_project, "run" => ?json_run);

    let idempotency_key = json_run.idempotency_key.take();

    let new_run_report = NewRunReport {
        report: json_run.into(),
        idempotency_key,
        #[cfg(feature = "plus")]
        is_claimed,
        #[cfg(feature = "plus")]
        testbed,
        #[cfg(feature = "plus")]
        spec_reset,
        #[cfg(feature = "plus")]
        job,
    };

    QueryReport::create(log, context, query_project, new_run_report, api_actor).await
}

/// The project that a run targets.
enum TargetProject {
    /// The `project` field was specified.
    Explicit(ProjectResourceId),
    /// The project was derived from the job image repository.
    #[cfg(feature = "plus")]
    Derived(ProjectResourceId),
}

impl TargetProject {
    /// Find the target project for a run, if any.
    ///
    /// Bencher registry images are named `[{registry}/]{project}:{tag}`,
    /// so a job image project repository that parses as a project
    /// resource ID derives the target project.
    /// Multi-segment repositories (e.g. `{user}/{image}`) are not
    /// supported by the Bencher registry, so no project is derived for them.
    fn find(json_run: &JsonNewRun) -> Option<Self> {
        if let Some(project) = json_run.project.clone() {
            return Some(Self::Explicit(project));
        }
        #[cfg(feature = "plus")]
        if let Some(job) = &json_run.job
            && let Some(project) = job
                .image
                .project_repository()
                .and_then(|repository| repository.parse().ok())
        {
            return Some(Self::Derived(project));
        }
        None
    }

    fn resource_id(&self) -> &ProjectResourceId {
        match self {
            Self::Explicit(project) => project,
            #[cfg(feature = "plus")]
            Self::Derived(project) => project,
        }
    }
}

fn project_name(json_run: &JsonNewRun) -> Result<ResourceName, HttpError> {
    json_run
        .context
        .as_ref()
        .and_then(RunContext::name)
        .ok_or_else(|| {
            bad_request_error(
            "The `project` field was not specified nor was a run `context` provided for the name",
        )
        })
}

fn project_slug(json_run: &JsonNewRun) -> Result<ProjectSlug, HttpError> {
    json_run
        .context
        .as_ref()
        .and_then(RunContext::slug)
        .map(Into::into)
        .ok_or_else(|| {
            bad_request_error(
            "The `project` field was not specified nor was a run `context` provided for the slug",
        )
        })
}
