#![cfg(feature = "plus")]

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk, TotalCount};
use bencher_json::{
    JobStatus, JobUuid, JsonDirection, JsonJob, JsonPagination, ProjectResourceId, runner::JsonJobs,
};
use bencher_schema::{
    context::ApiContext,
    error::resource_not_found_err,
    model::{project::QueryProject, runner::QueryJob, user::public::PublicUser},
    public_conn, schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _};
use dropshot::{HttpError, Path, Query, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct ProjJobsParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

pub type ProjJobsPagination = JsonPagination<ProjJobsSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjJobsSort {
    /// Sort by creation date time.
    #[default]
    Created,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct ProjJobsQuery {
    /// Filter by job status.
    pub status: Option<JobStatus>,
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/projects/{project}/jobs",
    tags = ["projects", "jobs"]
}]
pub async fn proj_jobs_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjJobsParams>,
    _pagination_params: Query<ProjJobsPagination>,
    _query_params: Query<ProjJobsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List jobs for a project
///
/// List all jobs for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
/// By default, the jobs are sorted by creation date time in reverse chronological order.
/// The HTTP response header `X-Total-Count` contains the total number of jobs.
#[endpoint {
    method = GET,
    path = "/v0/projects/{project}/jobs",
    tags = ["projects", "jobs"]
}]
pub async fn proj_jobs_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjJobsParams>,
    pagination_params: Query<ProjJobsPagination>,
    query_params: Query<ProjJobsQuery>,
) -> Result<ResponseOk<JsonJobs>, HttpError> {
    let public_user = PublicUser::new(&rqctx).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
        &public_user,
    )
    .await?;
    Ok(Get::response_ok_with_total_count(
        json,
        public_user.is_auth(),
        total_count,
    ))
}

async fn get_ls_inner(
    context: &ApiContext,
    path_params: ProjJobsParams,
    pagination_params: ProjJobsPagination,
    query_params: ProjJobsQuery,
    public_user: &PublicUser,
) -> Result<(JsonJobs, TotalCount), HttpError> {
    let query_project = QueryProject::is_allowed_public(
        public_conn!(context, public_user),
        &context.rbac,
        &path_params.project,
        public_user,
    )?;

    let jobs = get_ls_query(&query_project, &pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryJob>(public_conn!(context, public_user))
        .map_err(resource_not_found_err!(
            Job,
            (&query_project, &pagination_params, &query_params)
        ))?;

    let json_jobs = public_conn!(context, public_user, |conn| {
        jobs.into_iter()
            .map(|job| job.into_json(conn))
            .collect::<Result<Vec<_>, _>>()?
    });

    let total_count = get_ls_query(&query_project, &pagination_params, &query_params)
        .count()
        .get_result::<i64>(public_conn!(context, public_user))
        .map_err(resource_not_found_err!(
            Job,
            (&query_project, &pagination_params, &query_params)
        ))?
        .try_into()?;

    Ok((json_jobs.into(), total_count))
}

fn get_ls_query<'q>(
    query_project: &'q QueryProject,
    pagination_params: &ProjJobsPagination,
    query_params: &'q ProjJobsQuery,
) -> BoxedQuery<'q> {
    let mut query = schema::job::table
        .inner_join(schema::report::table)
        .filter(schema::report::project_id.eq(query_project.id))
        .select(QueryJob::as_select())
        .into_boxed();

    if let Some(status) = query_params.status {
        query = query.filter(schema::job::status.eq(status));
    }

    match pagination_params.order() {
        ProjJobsSort::Created => match pagination_params.direction {
            Some(JsonDirection::Asc) => query.order(schema::job::created.asc()),
            Some(JsonDirection::Desc) | None => query.order(schema::job::created.desc()),
        },
    }
}

// TODO refactor out internal types
type BoxedQuery<'q> = diesel::internal::table_macro::BoxedSelectStatement<
    'q,
    diesel::helper_types::AsSelect<QueryJob, diesel::sqlite::Sqlite>,
    diesel::internal::table_macro::FromClause<
        diesel::helper_types::InnerJoinQuerySource<schema::job::table, schema::report::table>,
    >,
    diesel::sqlite::Sqlite,
>;

#[derive(Deserialize, JsonSchema)]
pub struct ProjJobParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The UUID for a job.
    pub job: JobUuid,
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/projects/{project}/jobs/{job}",
    tags = ["projects", "jobs"]
}]
pub async fn proj_job_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjJobParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View a job
///
/// View a job for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
#[endpoint {
    method = GET,
    path = "/v0/projects/{project}/jobs/{job}",
    tags = ["projects", "jobs"]
}]
pub async fn proj_job_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjJobParams>,
) -> Result<ResponseOk<JsonJob>, HttpError> {
    let public_user = PublicUser::new(&rqctx).await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &public_user).await?;
    Ok(Get::response_ok(json, public_user.is_auth()))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjJobParams,
    public_user: &PublicUser,
) -> Result<JsonJob, HttpError> {
    let query_project = QueryProject::is_allowed_public(
        public_conn!(context, public_user),
        &context.rbac,
        &path_params.project,
        public_user,
    )?;

    let job: QueryJob = schema::job::table
        .inner_join(schema::report::table)
        .filter(schema::report::project_id.eq(query_project.id))
        .filter(schema::job::uuid.eq(path_params.job))
        .select(QueryJob::as_select())
        .first(public_conn!(context, public_user))
        .map_err(resource_not_found_err!(
            Job,
            (&query_project, path_params.job)
        ))?;

    job.into_json(public_conn!(context, public_user))
}
