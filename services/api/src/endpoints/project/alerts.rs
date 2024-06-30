use bencher_json::{
    project::alert::{AlertStatus, JsonAlertStats, JsonUpdateAlert},
    AlertUuid, JsonAlert, JsonAlerts, JsonDirection, JsonPagination, ResourceId,
};
use bencher_rbac::project::Permission;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    conn_lock,
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, Patch, ResponseOk},
        Endpoint,
    },
    error::{resource_conflict_err, resource_not_found_err},
    model::{
        project::{
            threshold::alert::{QueryAlert, UpdateAlert},
            QueryProject,
        },
        user::auth::{AuthUser, BearerToken, PubBearerToken},
    },
    schema,
    util::headers::TotalCount,
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjAlertsParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
}

pub type ProjAlertsPagination = JsonPagination<ProjAlertsSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjAlertsSort {
    /// Sort by alert creation date time.
    Created,
    /// Sort by alert modified date time.
    #[default]
    Modified,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/alerts",
    tags = ["projects", "alerts"]
}]
pub async fn proj_alerts_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjAlertsParams>,
    _pagination_params: Query<ProjAlertsPagination>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List alerts for a project
///
/// List all alerts for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
/// By default, the alerts are sorted by status (active then dismissed) and modification date time in reverse chronological order.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/alerts",
    tags = ["projects", "alerts"]
}]
pub async fn proj_alerts_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjAlertsParams>,
    pagination_params: Query<ProjAlertsPagination>,
) -> Result<ResponseOk<JsonAlerts>, HttpError> {
    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        pagination_params.into_inner(),
    )
    .await?;
    Ok(Get::response_ok_with_total_count(
        json,
        auth_user.is_some(),
        total_count,
    ))
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: Option<&AuthUser>,
    path_params: ProjAlertsParams,
    pagination_params: ProjAlertsPagination,
) -> Result<(JsonAlerts, TotalCount), HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    let mut query = schema::alert::table
        .inner_join(
            schema::boundary::table.inner_join(
                schema::metric::table.inner_join(
                    schema::report_benchmark::table
                        .inner_join(schema::report::table)
                        .inner_join(schema::benchmark::table),
                ),
            ),
        )
        .filter(schema::benchmark::project_id.eq(query_project.id))
        .select(QueryAlert::as_select())
        .into_boxed();

    query = match pagination_params.order() {
        ProjAlertsSort::Created => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order((
                schema::alert::status.asc(),
                schema::report::start_time.asc(),
                schema::benchmark::name.asc(),
                schema::report_benchmark::iteration.asc(),
            )),
            Some(JsonDirection::Desc) => query.order((
                schema::alert::status.asc(),
                schema::report::start_time.desc(),
                schema::benchmark::name.asc(),
                schema::report_benchmark::iteration.asc(),
            )),
        },
        ProjAlertsSort::Modified => match pagination_params.direction {
            Some(JsonDirection::Asc) => query.order((
                schema::alert::status.asc(),
                schema::alert::modified.asc(),
                schema::benchmark::name.asc(),
                schema::report_benchmark::iteration.asc(),
            )),
            Some(JsonDirection::Desc) | None => query.order((
                schema::alert::status.asc(),
                schema::alert::modified.desc(),
                schema::benchmark::name.asc(),
                schema::report_benchmark::iteration.asc(),
            )),
        },
    };

    // Separate out this query to prevent a deadlock when getting the conn_lock
    let alerts = query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load(conn_lock!(context))
        .map_err(resource_not_found_err!(Alert, &query_project))?;

    let mut json_alerts = Vec::with_capacity(alerts.len());
    for alert in alerts {
        match alert.into_json(context).await {
            Ok(alert) => json_alerts.push(alert),
            Err(err) => {
                debug_assert!(false, "{err}");
                #[cfg(feature = "sentry")]
                sentry::capture_error(&err);
            },
        }
    }

    Ok((json_alerts.into(), TotalCount::default()))
}

fn get_ls_query<'q>(
    query_project: &'q QueryProject,
    pagination_params: &ProjAlertsPagination,
) -> Next<'q> {
    let mut query = schema::alert::table
        .inner_join(
            schema::boundary::table.inner_join(
                schema::metric::table.inner_join(
                    schema::report_benchmark::table
                        .inner_join(schema::report::table)
                        .inner_join(schema::benchmark::table),
                ),
            ),
        )
        .filter(schema::benchmark::project_id.eq(query_project.id))
        .into_boxed();

    match pagination_params.order() {
        ProjAlertsSort::Created => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order((
                schema::alert::status.asc(),
                schema::report::start_time.asc(),
                schema::benchmark::name.asc(),
                schema::report_benchmark::iteration.asc(),
            )),
            Some(JsonDirection::Desc) => query.order((
                schema::alert::status.asc(),
                schema::report::start_time.desc(),
                schema::benchmark::name.asc(),
                schema::report_benchmark::iteration.asc(),
            )),
        },
        ProjAlertsSort::Modified => match pagination_params.direction {
            Some(JsonDirection::Asc) => query.order((
                schema::alert::status.asc(),
                schema::alert::modified.asc(),
                schema::benchmark::name.asc(),
                schema::report_benchmark::iteration.asc(),
            )),
            Some(JsonDirection::Desc) | None => query.order((
                schema::alert::status.asc(),
                schema::alert::modified.desc(),
                schema::benchmark::name.asc(),
                schema::report_benchmark::iteration.asc(),
            )),
        },
    }
    .select(QueryAlert::as_select())
}

// TODO refactor out internal types
type BoxedQuery<'q> = diesel::internal::table_macro::BoxedSelectStatement<
    'q,
    diesel::helper_types::AsSelect<QueryAlert, diesel::sqlite::Sqlite>,
    diesel::internal::table_macro::FromClause<
        diesel::helper_types::InnerJoinQuerySource<
            schema::alert::table,
            diesel::helper_types::InnerJoinQuerySource<
                schema::boundary::table,
                diesel::helper_types::InnerJoinQuerySource<
                    schema::metric::table,
                    diesel::helper_types::InnerJoinQuerySource<
                        diesel::helper_types::InnerJoin<
                            schema::report_benchmark::table,
                            schema::report::table,
                        >,
                        schema::benchmark::table,
                    >,
                >,
            >,
        >,
    >,
    diesel::sqlite::Sqlite,
>;

type Next<'q> = diesel::internal::table_macro::BoxedSelectStatement<
    'q,
    diesel::helper_types::AsSelect<QueryAlert, diesel::sqlite::Sqlite>,
    diesel::internal::table_macro::FromClause<
        diesel::internal::table_macro::JoinOn<
            diesel::internal::table_macro::Join<
                schema::alert::table,
                diesel::internal::table_macro::SelectStatement<
                    diesel::internal::table_macro::FromClause<
                        diesel::internal::table_macro::JoinOn<
                            diesel::internal::table_macro::Join<
                                schema::boundary::table,
                                diesel::internal::table_macro::SelectStatement<
                                    diesel::internal::table_macro::FromClause<
                                        diesel::internal::table_macro::JoinOn<
                                            diesel::internal::table_macro::Join<
                                                schema::metric::table,
                                                diesel::internal::table_macro::SelectStatement<
                                                    diesel::internal::table_macro::FromClause<
                                                        diesel::internal::table_macro::JoinOn<
                                                            diesel::internal::table_macro::Join<
                                                                diesel::internal::table_macro::JoinOn<
                                                                    diesel::internal::table_macro::Join<
                                                                        schema::report_benchmark::table,
                                                                        schema::report::table,
                                                                        diesel::internal::table_macro::Inner
                                                                    >,
                                                                    // diesel::dsl::Grouped<
                                                                        diesel::dsl::Eq<
                                                                            diesel::internal::table_macro::NullableExpression<
                                                                                schema::report_benchmark::columns::report_id
                                                                            >,
                                                                            diesel::internal::table_macro::NullableExpression<
                                                                                schema::report::columns::id
                                                                            >
                                                                        >
                                                                    // >
                                                                >,
                                                                schema::benchmark::table,
                                                                diesel::internal::table_macro::Inner
                                                            >,
                                                            // diesel::dsl::Grouped<
                                                                diesel::dsl::Eq<
                                                                    diesel::internal::table_macro::NullableExpression<
                                                                        schema::report_benchmark::columns::benchmark_id
                                                                    >,
                                                                    diesel::internal::table_macro::NullableExpression<
                                                                        schema::benchmark::columns::id
                                                                    >
                                                                >
                                                            // >
                                                        >
                                                    >
                                                >,
                                                diesel::internal::table_macro::Inner
                                            >,
                                            // diesel::dsl::Grouped<
                                                diesel::dsl::Eq<
                                                    diesel::internal::table_macro::NullableExpression<
                                                        schema::metric::columns::report_benchmark_id
                                                    >,
                                                    diesel::internal::table_macro::NullableExpression<
                                                        schema::report_benchmark::columns::id
                                                    >
                                                >
                                            // >
                                        >
                                    >
                                >,
                                diesel::internal::table_macro::Inner
                            >,
                            // diesel::dsl::Grouped<
                                diesel::dsl::Eq<
                                    diesel::internal::table_macro::NullableExpression<
                                        schema::boundary::columns::metric_id
                                    >,
                                    diesel::internal::table_macro::NullableExpression<
                                        schema::metric::columns::id
                                    >
                                >
                            // >
                        >
                    >
                >,
                diesel::internal::table_macro::Inner
            >,
            // diesel::dsl::Grouped<
                diesel::dsl::Eq<
                    diesel::internal::table_macro::NullableExpression<
                        schema::alert::columns::boundary_id
                    >,
                    diesel::internal::table_macro::NullableExpression<
                        schema::boundary::columns::id
                    >
                >
            // >
        >
    >,
    diesel::sqlite::Sqlite,
>;

// type Verbose<'q> = diesel::internal::table_macro::BoxedSelectStatement<
// 'q,
// diesel::helper_types::AsSelect<QueryAlert, diesel::sqlite::Sqlite>,
// diesel::internal::table_macro::FromClause<
//     diesel::query_source::joins::JoinOn<
//         diesel::query_source::joins::Join<
//             schema::alert::table,
//             diesel::internal::table_macro::SelectStatement<
//                 diesel::query_builder::from_clause::FromClause<
//                     diesel::query_source::joins::JoinOn<
//                         diesel::query_source::joins::Join<
//                             schema::boundary::table,
//                             diesel::internal::table_macro::SelectStatement<
//                                 diesel::query_builder::from_clause::FromClause<
//                                     diesel::query_source::joins::JoinOn<
//                                     diesel::query_source::joins::Join<
//                                         schema::metric::table,
//                                         diesel::internal::table_macro::SelectStatement<
//                                             diesel::query_builder::from_clause::FromClause<
//                                                 diesel::query_source::joins::JoinOn<
//                                                     diesel::query_source::joins::Join<
//                                                         diesel::query_source::joins::JoinOn<
//                                                             diesel::query_source::joins::Join<
//                                                                 schema::report_benchmark::table,
//                                                                 schema::report::table,
//                                                                 diesel::query_source::joins::Inner
//                                                         >,
//                                                         diesel::expression::grouped::Grouped<
//                                                             diesel::expression::operators::Eq<
//                                                                 diesel::expression::nullable::Nullable<
//                                                                     schema::report_benchmark::columns::report_id
//                                                                 >,
//                                                                 diesel::expression::nullable::Nullable<
//                                                                     schema::report::columns::id
//                                                                 >
//                                                             >
//                                                         >
//                                                     >,
//                                                     schema::benchmark::table,
//                                                     diesel::query_source::joins::Inner
//                                                 >,
//                                                 diesel::expression::grouped::Grouped<
//                                                     diesel::expression::operators::Eq<
//                                                         diesel::expression::nullable::Nullable<
//                                                             schema::report_benchmark::columns::benchmark_id
//                                                         >,
//                                                         diesel::expression::nullable::Nullable<
//                                                             schema::benchmark::columns::id
//                                                         >
//                                                     >
//                                                 >
//                                             >
//                                         >
//                                     >,
//                                     diesel::query_source::joins::Inner
//                                 >,
//                                 diesel::expression::grouped::Grouped<
//                                     diesel::expression::operators::Eq<
//                                         diesel::expression::nullable::Nullable<
//                                             schema::metric::columns::report_benchmark_id
//                                         >,
//                                         diesel::expression::nullable::Nullable<
//                                             schema::report_benchmark::columns::id
//                                         >
//                                     >
//                                 >
//                             >
//                         >
//                     >,
//                     diesel::query_source::joins::Inner
//                 >,
//                 diesel::expression::grouped::Grouped<
//                     diesel::expression::operators::Eq<
//                         diesel::expression::nullable::Nullable<
//                             schema::boundary::columns::metric_id
//                         >,
//                         diesel::expression::nullable::Nullable<
//                             schema::metric::columns::id
//                         >
//                     >
//                 >
//             >
//         >
//     >,
//     diesel::query_source::joins::Inner
// >, diesel::expression::grouped::Grouped<diesel::expression::operators::Eq<diesel::expression::nullable::Nullable<schema::alert::columns::boundary_id>, diesel::expression::nullable::Nullable<schema::boundary::columns::id>>>>>, diesel::sqlite::Sqlite>;

#[derive(Deserialize, JsonSchema)]
pub struct ProjAlertParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
    /// The UUID for an alert.
    pub alert: AlertUuid,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/alerts/{alert}",
    tags = ["projects", "alerts"]
}]
pub async fn proj_alert_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjAlertParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into()]))
}

/// View an alert
///
/// View an alert for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/alerts/{alert}",
    tags = ["projects", "alerts"]
}]
pub async fn proj_alert_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjAlertParams>,
) -> Result<ResponseOk<JsonAlert>, HttpError> {
    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        auth_user.as_ref(),
    )
    .await?;
    Ok(Get::response_ok(json, auth_user.is_some()))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjAlertParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonAlert, HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    // Separate out this query to prevent a deadlock when getting the conn_lock
    let alert = QueryAlert::from_uuid(conn_lock!(context), query_project.id, path_params.alert)?;

    alert.into_json(context).await
}

/// Update an alert
///
/// Update an alert for a project.
/// The user must have `edit` permissions for the project.
/// Use this endpoint to dismiss an alert.
#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/alerts/{alert}",
    tags = ["projects", "alerts"]
}]
pub async fn proj_alert_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjAlertParams>,
    body: TypedBody<JsonUpdateAlert>,
) -> Result<ResponseOk<JsonAlert>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = patch_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Patch::auth_response_ok(json))
}

async fn patch_inner(
    context: &ApiContext,
    path_params: ProjAlertParams,
    json_alert: JsonUpdateAlert,
    auth_user: &AuthUser,
) -> Result<JsonAlert, HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?;

    let query_alert =
        QueryAlert::from_uuid(conn_lock!(context), query_project.id, path_params.alert)?;
    let update_alert = UpdateAlert::from(json_alert.clone());
    diesel::update(schema::alert::table.filter(schema::alert::id.eq(query_alert.id)))
        .set(&update_alert)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Alert, (&query_alert, &json_alert)))?;

    // Separate out this query to prevent a deadlock when getting the conn_lock
    let alert = QueryAlert::get(conn_lock!(context), query_alert.id)?;

    alert.into_json(context).await
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/stats/alerts",
    tags = ["projects", "alerts"]
}]
pub async fn proj_alert_stats_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjAlertsParams>,
    _pagination_params: Query<ProjAlertsPagination>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View the total number of active alerts for a project
///
/// View the total number of active alerts for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
/// Use this endpoint to monitor the number of active alerts for a project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/stats/alerts",
    tags = ["projects", "alerts"]
}]
pub async fn proj_alert_stats_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjAlertsParams>,
) -> Result<ResponseOk<JsonAlertStats>, HttpError> {
    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    let json = get_stats_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
    )
    .await?;
    Ok(Get::response_ok(json, auth_user.is_some()))
}

async fn get_stats_inner(
    context: &ApiContext,
    auth_user: Option<&AuthUser>,
    path_params: ProjAlertsParams,
) -> Result<JsonAlertStats, HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    let active =
        schema::alert::table
            .filter(schema::alert::status.eq(AlertStatus::Active))
            .inner_join(schema::boundary::table.inner_join(
                schema::metric::table.inner_join(
                    schema::report_benchmark::table.inner_join(schema::benchmark::table),
                ),
            ))
            .filter(schema::benchmark::project_id.eq(query_project.id))
            .count()
            .get_result::<i64>(conn_lock!(context))
            .map_err(resource_not_found_err!(Alert, query_project))?;

    Ok(JsonAlertStats {
        active: u64::try_from(active).unwrap_or_default().into(),
    })
}
