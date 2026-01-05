use std::collections::HashMap;

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
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::{bad_request_error, resource_conflict_err, resource_not_found_err},
    model::{
        project::{
            QueryProject,
            branch::{
                head::HeadId,
                version::{QueryVersion, VersionId},
            },
            report::{QueryReport, ReportId},
        },
        user::{
            auth::{AuthUser, BearerToken},
            public::{PubBearerToken, PublicUser},
        },
    },
    schema,
};
use diesel::{
    BelongingToDsl as _, BoolExpressionMethods as _, ExpressionMethods as _, JoinOnDsl as _,
    QueryDsl as _, RunQueryDsl as _, SelectableHelper as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
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
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
/// By default, the reports are sorted by date time in reverse chronological order.
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

    let public_user = PublicUser::new(&rqctx).await?;
    let (json, total_count) = get_ls_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        json_report_query,
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
    log: &Logger,
    context: &ApiContext,
    path_params: ProjReportsParams,
    pagination_params: ProjReportsPagination,
    query_params: JsonReportQuery,
    public_user: &PublicUser,
) -> Result<(JsonReports, TotalCount), HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        public_user,
    )?;

    let reports = get_ls_query(&query_project, &pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Report,
            (&query_project, &pagination_params, &query_params)
        ))?;

    // Separate out these queries to prevent a deadlock when getting the conn_lock
    let mut json_reports = Vec::with_capacity(reports.len());
    for report in reports {
        match report.into_json(log, context).await {
            Ok(report) => json_reports.push(report),
            Err(err) => {
                debug_assert!(false, "{err}");
                #[cfg(feature = "sentry")]
                sentry::capture_error(&err);
            },
        }
    }

    let total_count = get_ls_query(&query_project, &pagination_params, &query_params)
        .count()
        .get_result::<i64>(conn_lock!(context))
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
/// The user must have `create` permissions for the project.
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
    bearer_token: BearerToken,
    path_params: Path<ProjReportsParams>,
    body: TypedBody<JsonNewReport>,
) -> Result<ResponseCreated<JsonReport>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    path_params: ProjReportsParams,
    json_report: JsonNewReport,
    auth_user: &AuthUser,
) -> Result<JsonReport, HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Create,
    )?;
    QueryReport::create(
        log,
        context,
        &query_project,
        json_report,
        Some(auth_user.id),
    )
    .await
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
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/reports/{report}",
    tags = ["projects", "reports"]
}]
pub async fn proj_report_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjReportParams>,
) -> Result<ResponseOk<JsonReport>, HttpError> {
    let public_user = PublicUser::from_token(
        &rqctx.log,
        rqctx.context(),
        &rqctx.request_id,
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_one_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        &public_user,
    )
    .await?;
    Ok(Get::response_ok(json, public_user.is_auth()))
}

async fn get_one_inner(
    log: &Logger,
    context: &ApiContext,
    path_params: ProjReportParams,
    public_user: &PublicUser,
) -> Result<JsonReport, HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        public_user,
    )?;

    let report = QueryReport::belonging_to(&query_project)
        .filter(schema::report::uuid.eq(path_params.report.to_string()))
        .first::<QueryReport>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Report,
            (&query_project, path_params.report)
        ))?;

    // Separate out this query to prevent a deadlock when getting the conn_lock
    report.into_json(log, context).await
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
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjReportParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?;

    let (report_id, version_id) = QueryReport::belonging_to(&query_project)
        .filter(schema::report::uuid.eq(path_params.report.to_string()))
        .select((schema::report::id, schema::report::version_id))
        .first::<(ReportId, VersionId)>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Report,
            (&query_project, path_params.report)
        ))?;
    diesel::delete(schema::report::table.filter(schema::report::id.eq(report_id)))
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Report, report_id))?;

    // If there are no more reports for this version, delete the version
    // This is necessary because multiple reports can use the same version via a git hash
    // This will cascade and delete all head versions for this version
    // Before doing so, decrement all greater versions
    // Otherwise, just return since the version is still in use
    if schema::report::table
        .filter(schema::report::version_id.eq(version_id))
        .count()
        .first::<i64>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Version,
            (&query_project, report_id, version_id)
        ))?
        != 0
    {
        return Ok(());
    }

    let query_version = QueryVersion::get(conn_lock!(context), version_id)?;
    // Get all heads that use this version
    let heads = schema::head::table
        .inner_join(
            schema::head_version::table.on(schema::head_version::head_id.eq(schema::head::id)),
        )
        .filter(schema::head_version::version_id.eq(version_id))
        .select(schema::head::id)
        .load::<HeadId>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Head,
            (&query_project, report_id, version_id)
        ))?;

    let mut version_map = HashMap::new();
    // Get all versions greater than this one for each of the heads
    for head_id in heads {
        schema::version::table
            .inner_join(schema::head_version::table)
            .filter(schema::version::number.gt(query_version.number))
            .filter(schema::head_version::head_id.eq(head_id))
            .select((schema::version::id, schema::version::number))
            .load::<(VersionId, VersionNumber)>(conn_lock!(context))
            .map_err(resource_not_found_err!(
                Version,
                (&query_project, report_id, head_id, &query_version)
            ))?
            .into_iter()
            .for_each(|(version_id, version_number)| {
                version_map.insert(version_id, version_number);
            });
    }

    // For each version greater than this one, decrement the version number
    for (version_id, version_number) in version_map {
        if let Err(e) =
            diesel::update(schema::version::table.filter(schema::version::id.eq(version_id)))
                .set(schema::version::number.eq(version_number.decrement()))
                .execute(conn_lock!(context))
        {
            debug_assert!(
                false,
                "Failed to decrement version ({version_id}) number ({version_number}): {e}"
            );
            #[cfg(feature = "sentry")]
            sentry::capture_error(&e);
        }
    }

    // Finally delete the dangling version
    diesel::delete(schema::version::table.filter(schema::version::id.eq(version_id)))
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(
            Version,
            (&query_project, report_id, &query_version)
        ))?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReportDelete);

    Ok(())
}
