use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Post, ResponseCreated, ResponseDeleted, ResponseOk,
    TotalCount,
};
use bencher_json::{
    JsonDirection, JsonNewReport, JsonPagination, JsonReport, JsonReports, ProjectResourceId,
    ReportUuid,
    project::{
        head::VersionNumber,
        report::{JsonReportQuery, JsonReportQueryParams},
    },
};
use bencher_rbac::project::Permission;
#[cfg(feature = "plus")]
use bencher_schema::model::project::testbed::RunTestbed;
use bencher_schema::{
    actor_conn, auth_conn,
    context::ApiContext,
    error::{
        bad_request_error, resource_conflict_err, resource_not_found_err, with_auth_hint,
        with_token_hint,
    },
    model::{
        project::{
            QueryProject,
            branch::{head::HeadId, version::VersionId},
            report::{
                NewRunReport, QueryReport, ReportId, ReportMode,
                report_benchmark::ReportBenchmarkId,
            },
        },
        user::{
            actor::{ApiActor, PubProjectBearerToken},
            auth::{AuthUser, BearerToken},
        },
    },
    schema, write_conn, write_transaction,
};
use diesel::{
    BelongingToDsl as _, BoolExpressionMethods as _, ExpressionMethods as _, JoinOnDsl as _,
    OptionalExtension as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use futures::{StreamExt as _, stream::FuturesOrdered};
use schemars::JsonSchema;
use serde::Deserialize;
use slog::Logger;

use crate::macros::{filter_branch_name_id, filter_testbed_name_id};

#[derive(Deserialize, JsonSchema)]
pub struct ProjReportsParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

pub type ProjReportsPagination = JsonPagination<ProjReportsSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjReportsSort {
    /// Sort by date time.
    #[default]
    DateTime,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/reports",
    tags = ["projects", "reports"]
}]
pub async fn proj_reports_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjReportsParams>,
    _pagination_params: Query<ProjReportsPagination>,
    _query_params: Query<JsonReportQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// List reports for a project
///
/// List all reports for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project,
/// or provide a valid project key for the project.
/// By default, the reports are sorted by date time in reverse chronological order.
/// By default, the results and alerts for each report are omitted and only their counts are included.
/// Set the `expand` query param to `true` to include the full results and alerts.
/// The HTTP response header `X-Total-Count` contains the total number of reports.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/reports",
    tags = ["projects", "reports"]
}]
pub async fn proj_reports_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjReportsParams>,
    pagination_params: Query<ProjReportsPagination>,
    query_params: Query<JsonReportQueryParams>,
) -> Result<ResponseOk<JsonReports>, HttpError> {
    // Second round of marshaling
    let json_report_query = query_params
        .into_inner()
        .try_into()
        .map_err(bad_request_error)?;

    let api_actor = ApiActor::new(&rqctx).await?;
    let (json, total_count) = get_ls_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        json_report_query,
        &api_actor,
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::response_ok_with_total_count(
        json,
        api_actor.is_auth(),
        total_count,
    ))
}

pub async fn get_ls_inner(
    log: &Logger,
    context: &ApiContext,
    path_params: ProjReportsParams,
    pagination_params: ProjReportsPagination,
    query_params: JsonReportQuery,
    api_actor: &ApiActor,
) -> Result<(JsonReports, TotalCount), HttpError> {
    let query_project = QueryProject::is_allowed_actor_pub(
        actor_conn!(context, api_actor),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        api_actor,
    )?;

    let reports = get_ls_query(&query_project, &pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load(actor_conn!(context, api_actor))
        .map_err(resource_not_found_err!(
            Report,
            (&query_project, &pagination_params, &query_params)
        ))?;

    let mode = if query_params.expand.unwrap_or_default() {
        ReportMode::Full
    } else {
        ReportMode::Collapsed
    };
    // Drop connection lock before iterating
    let json_reports = reports
        .into_iter()
        .map(|report| async { report.into_json(log, actor_conn!(context, api_actor), mode) })
        .collect::<FuturesOrdered<_>>()
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .filter_map(|report| match report {
            Ok(report) => Some(report),
            Err(err) => {
                debug_assert!(false, "{err}");
                #[cfg(feature = "sentry")]
                sentry::capture_error(&err);
                None
            },
        })
        .collect::<Vec<_>>();

    let total_count = get_ls_query(&query_project, &pagination_params, &query_params)
        .count()
        .get_result::<i64>(actor_conn!(context, api_actor))
        .map_err(resource_not_found_err!(
            Report,
            (&query_project, &pagination_params, &query_params)
        ))?
        .try_into()?;

    Ok((json_reports.into(), total_count))
}

fn get_ls_query<'q>(
    query_project: &'q QueryProject,
    pagination_params: &ProjReportsPagination,
    query_params: &'q JsonReportQuery,
) -> BoxedQuery<'q> {
    let mut query =
        QueryReport::belonging_to(query_project)
            .inner_join(schema::head::table.inner_join(
                schema::branch::table.on(schema::head::branch_id.eq(schema::branch::id)),
            ))
            .inner_join(schema::testbed::table)
            .into_boxed();

    if let Some(branch) = query_params.branch.as_ref() {
        filter_branch_name_id!(query, branch);
    }
    if let Some(testbed) = query_params.testbed.as_ref() {
        filter_testbed_name_id!(query, testbed);
    }

    if let Some(start_time) = query_params.start_time {
        query = query.filter(schema::report::start_time.ge(start_time));
    }
    if let Some(end_time) = query_params.end_time {
        query = query.filter(schema::report::end_time.le(end_time));
    }

    if let Some(true) = query_params.archived {
        query = query.filter(
            schema::branch::archived
                .is_not_null()
                .or(schema::testbed::archived.is_not_null()),
        );
    } else {
        query = query.filter(
            schema::branch::archived
                .is_null()
                .and(schema::testbed::archived.is_null()),
        );
    }

    match pagination_params.order() {
        ProjReportsSort::DateTime => match pagination_params.direction {
            Some(JsonDirection::Asc) => query.order((
                schema::report::start_time.asc(),
                schema::report::end_time.asc(),
                schema::report::created.asc(),
            )),
            Some(JsonDirection::Desc) | None => query.order((
                schema::report::start_time.desc(),
                schema::report::end_time.desc(),
                schema::report::created.desc(),
            )),
        },
    }
    .select(QueryReport::as_select())
}

// TODO refactor out internal types
type BoxedQuery<'q> = diesel::internal::table_macro::BoxedSelectStatement<
    'q,
    diesel::helper_types::AsSelect<QueryReport, diesel::sqlite::Sqlite>,
    diesel::internal::table_macro::FromClause<
        diesel::helper_types::InnerJoinQuerySource<
            diesel::helper_types::InnerJoinQuerySource<
                schema::report::table,
                diesel::internal::table_macro::SelectStatement<
                    diesel::internal::table_macro::FromClause<
                        diesel::helper_types::InnerJoinQuerySource<
                            schema::head::table,
                            schema::branch::table,
                            diesel::dsl::Eq<
                                schema::head::columns::branch_id,
                                schema::branch::columns::id,
                            >,
                        >,
                    >,
                >,
            >,
            schema::testbed::table,
        >,
    >,
    diesel::sqlite::Sqlite,
>;

/// Create a report
///
/// Create a report for a project.
/// The user must have `create` permissions for the project,
/// or provide a valid project key for the project.
/// If using the Bencher CLI, it is recommended to use the `bencher run` subcommand
/// instead of trying to create a report manually.
#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/reports",
    tags = ["projects", "reports"]
}]
// For simplicity, this query makes the assumption that all posts are perfectly
// chronological. That is, a report will never be posted for X after Y has
// already been submitted when X really happened before Y. For implementing git
// bisect more complex logic will be required.
pub async fn proj_report_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjReportsParams>,
    body: TypedBody<JsonNewReport>,
) -> Result<ResponseCreated<JsonReport>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = post_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        api_actor,
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Post::auth_response_created(json))
}

pub async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    path_params: ProjReportsParams,
    json_report: JsonNewReport,
    api_actor: ApiActor,
) -> Result<JsonReport, HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed_actor_auth(
        auth_conn!(context),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        &api_actor,
        Permission::Create,
    )?;

    let new_run_report = NewRunReport {
        report: json_report,
        idempotency_key: None,
        #[cfg(feature = "plus")]
        is_claimed: true,
        #[cfg(feature = "plus")]
        testbed: RunTestbed::Explicit,
        #[cfg(feature = "plus")]
        spec_reset: false,
        #[cfg(feature = "plus")]
        job: None,
    };

    QueryReport::create(log, context, &query_project, new_run_report, &api_actor).await
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjReportParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The UUID for a report.
    pub report: ReportUuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/reports/{report}",
    tags = ["projects", "reports"]
}]
pub async fn proj_report_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjReportParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Delete.into()]))
}

/// View a report
///
/// View a report for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/reports/{report}",
    tags = ["projects", "reports"]
}]
pub async fn proj_report_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjReportParams>,
) -> Result<ResponseOk<JsonReport>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_one_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        &api_actor,
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::response_ok(json, api_actor.is_auth()))
}

pub async fn get_one_inner(
    log: &Logger,
    context: &ApiContext,
    path_params: ProjReportParams,
    api_actor: &ApiActor,
) -> Result<JsonReport, HttpError> {
    let query_project = QueryProject::is_allowed_actor_pub(
        actor_conn!(context, api_actor),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        api_actor,
    )?;

    actor_conn!(context, api_actor, |conn| {
        QueryReport::belonging_to(&query_project)
            .filter(schema::report::uuid.eq(path_params.report.to_string()))
            .first::<QueryReport>(conn)
            .map_err(resource_not_found_err!(
                Report,
                (&query_project, path_params.report)
            ))?
            .into_json(log, conn, ReportMode::Full)
    })
}

/// Delete a report
///
/// Delete a report for a project.
/// The user must have `delete` permissions for the project.
/// If there are no more reports for a branch version, then that version will be deleted.
/// All later branch versions will have their version numbers decremented.
#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/reports/{report}",
    tags = ["projects", "reports"]
}]
pub async fn proj_report_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjReportParams>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(with_token_hint)?;
    Ok(Delete::auth_response_deleted())
}

/// Number of `report_benchmark` rows deleted per write statement when
/// deleting a report's results. Bounds how long each delete holds the single
/// writer connection (validated at ~1s per chunk against a production-scale
/// report).
pub const DELETE_CHUNK_SIZE: i64 = 1024;

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjReportParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        auth_conn!(context),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?;

    let (report_id, version_id) = QueryReport::belonging_to(&query_project)
        .filter(schema::report::uuid.eq(path_params.report.to_string()))
        .select((schema::report::id, schema::report::version_id))
        .first::<(ReportId, VersionId)>(auth_conn!(context))
        .map_err(resource_not_found_err!(
            Report,
            (&query_project, path_params.report)
        ))?;
    delete_report_results(context, &query_project, report_id).await?;

    diesel::delete(schema::report::table.filter(schema::report::id.eq(report_id)))
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(Report, report_id))?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReportDelete);

    // Several reports can share a version via a git hash, so the version and its
    // renumbering only go once no report uses it.
    write_transaction!(context, |conn| {
        let remaining = schema::report::table
            .filter(schema::report::version_id.eq(version_id))
            .count()
            .get_result::<i64>(conn)?;
        if remaining != 0 {
            return Ok(());
        }
        let Some(number) = schema::version::table
            .filter(schema::version::id.eq(version_id))
            .select(schema::version::number)
            .first::<VersionNumber>(conn)
            .optional()?
        else {
            return Ok(());
        };
        let heads = schema::head_version::table
            .filter(schema::head_version::version_id.eq(version_id))
            .select(schema::head_version::head_id)
            .load::<HeadId>(conn)?;
        let head_versions = schema::head_version::table
            .filter(schema::head_version::head_id.eq_any(heads))
            .select(schema::head_version::version_id);
        diesel::update(
            schema::version::table
                .filter(schema::version::id.eq_any(head_versions))
                .filter(schema::version::number.gt(number)),
        )
        .set(schema::version::number.eq(schema::version::number - 1))
        .execute(conn)?;
        diesel::delete(schema::version::table.filter(schema::version::id.eq(version_id)))
            .execute(conn)
            .map(|_| ())
    })
    .map_err(resource_conflict_err!(
        Version,
        (&query_project, report_id, version_id)
    ))
}

/// Delete a report's results in bounded chunks, each in its own write
/// statement, so a large report does not hold the single writer connection
/// for its entire cascade (`report_benchmark` -> metric -> boundary -> alert).
/// Other writers interleave between chunks. Any rows that land between the
/// final chunk and the report row delete are cleaned up by the report's own
/// cascade.
///
/// The deletion is deliberately not atomic: if the server fails mid-loop, the
/// report remains visible with a partial set of results (and a stale
/// `metric_count_by_report` rollup) until the delete is retried. This is
/// acceptable because reports are immutable after creation, the delete is
/// idempotent, and the report was already condemned by the caller.
async fn delete_report_results(
    context: &ApiContext,
    query_project: &QueryProject,
    report_id: ReportId,
) -> Result<(), HttpError> {
    loop {
        let report_benchmark_ids = schema::report_benchmark::table
            .filter(schema::report_benchmark::report_id.eq(report_id))
            .select(schema::report_benchmark::id)
            .limit(DELETE_CHUNK_SIZE)
            .load::<ReportBenchmarkId>(auth_conn!(context))
            .map_err(resource_not_found_err!(Report, (query_project, report_id)))?;
        if report_benchmark_ids.is_empty() {
            return Ok(());
        }
        diesel::delete(
            schema::report_benchmark::table
                .filter(schema::report_benchmark::id.eq_any(report_benchmark_ids)),
        )
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(Report, report_id))?;
    }
}
