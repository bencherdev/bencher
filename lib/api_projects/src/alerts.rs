use bencher_endpoint::{CorsResponse, Endpoint, Get, Patch, ResponseOk, TotalCount};
use bencher_json::{
    AlertUuid, JsonAlert, JsonAlerts, JsonDirection, JsonPagination, ProjectResourceId,
    project::alert::{AlertStatus, JsonUpdateAlert},
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    auth_conn, conn_lock,
    context::ApiContext,
    error::{resource_conflict_err, resource_not_found_err},
    model::{
        project::{
            QueryProject,
            threshold::alert::{QueryAlert, UpdateAlert},
        },
        user::{
            auth::{AuthUser, BearerToken},
            public::{PubBearerToken, PublicUser},
        },
    },
    public_conn, schema,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use futures::stream::{FuturesOrdered, StreamExt as _};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct ProjAlertsParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjAlertsQuery {
    /// Filter alerts by their status.
    /// If not set, returns all alerts.
    pub status: Option<AlertStatus>,
    /// If set to `true`, only returns archived alerts.
    /// If not set or set to `false`, only returns alerts with non-archived branches, testbeds, or measures.
    pub archived: Option<bool>,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/alerts",
    tags = ["projects", "alerts"]
}]
pub async fn proj_alerts_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjAlertsParams>,
    _pagination_params: Query<ProjAlertsPagination>,
    _query_params: Query<ProjAlertsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List alerts for a project
///
/// List all alerts for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
/// By default, the alerts are sorted by status (active then dismissed) and modification date time in reverse chronological order.
/// The HTTP response header `X-Total-Count` contains the total number of alerts.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/alerts",
    tags = ["projects", "alerts"]
}]
pub async fn proj_alerts_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjAlertsParams>,
    pagination_params: Query<ProjAlertsPagination>,
    query_params: Query<ProjAlertsQuery>,
) -> Result<ResponseOk<JsonAlerts>, HttpError> {
    let public_user = PublicUser::new(&rqctx).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        &public_user,
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
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
    public_user: &PublicUser,
    path_params: ProjAlertsParams,
    pagination_params: ProjAlertsPagination,
    query_params: ProjAlertsQuery,
) -> Result<(JsonAlerts, TotalCount), HttpError> {
    let query_project = QueryProject::is_allowed_public(
        public_conn!(context, public_user),
        &context.rbac,
        &path_params.project,
        public_user,
    )?;

    let alerts = get_ls_query(&query_project, &pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load(public_conn!(context, public_user))
        .map_err(resource_not_found_err!(
            Alert,
            (&query_project, &pagination_params, &query_params)
        ))?;

    let json_alerts = alerts
        .into_iter()
        .map(|alert| async { alert.into_json(public_conn!(context, public_user)) })
        .collect::<FuturesOrdered<_>>()
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .filter_map(|alert| match alert {
            Ok(alert) => Some(alert),
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
        .get_result::<i64>(public_conn!(context, public_user))
        .map_err(resource_not_found_err!(
            Alert,
            (&query_project, &pagination_params, &query_params)
        ))?
        .try_into()?;

    Ok((json_alerts.into(), total_count))
}

fn get_ls_query<'q>(
    query_project: &'q QueryProject,
    pagination_params: &ProjAlertsPagination,
    query_params: &'q ProjAlertsQuery,
) -> BoxedQuery<'q> {
    let mut query = schema::alert::table
        .inner_join(
            schema::boundary::table
                .inner_join(
                    schema::threshold::table
                        .inner_join(schema::branch::table)
                        .inner_join(schema::testbed::table)
                        .inner_join(schema::measure::table),
                )
                .inner_join(
                    schema::metric::table.inner_join(
                        schema::report_benchmark::table
                            .inner_join(schema::report::table)
                            .inner_join(schema::benchmark::table),
                    ),
                ),
        )
        .filter(schema::benchmark::project_id.eq(query_project.id))
        .into_boxed();

    if let Some(status) = query_params.status {
        query = query.filter(schema::alert::status.eq(status));
    }

    if let Some(true) = query_params.archived {
        query = query.filter(
            schema::branch::archived
                .is_not_null()
                .or(schema::testbed::archived.is_not_null())
                .or(schema::measure::archived.is_not_null()),
        );
    } else {
        query = query.filter(
            schema::branch::archived
                .is_null()
                .and(schema::testbed::archived.is_null())
                .and(schema::measure::archived.is_null()),
        );
    }

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
            diesel::internal::table_macro::SelectStatement<
                diesel::internal::table_macro::FromClause<
                    diesel::helper_types::InnerJoinQuerySource<
                        diesel::helper_types::InnerJoinQuerySource<
                            schema::boundary::table,
                            diesel::internal::table_macro::SelectStatement<
                                diesel::internal::table_macro::FromClause<
                                    diesel::helper_types::InnerJoinQuerySource<
                                        diesel::helper_types::InnerJoinQuerySource<
                                            diesel::helper_types::InnerJoinQuerySource<
                                                schema::threshold::table,
                                                schema::branch::table,
                                            >,
                                            schema::testbed::table,
                                        >,
                                        schema::measure::table,
                                    >,
                                >,
                            >,
                        >,
                        diesel::internal::table_macro::SelectStatement<
                            diesel::internal::table_macro::FromClause<
                                diesel::helper_types::InnerJoinQuerySource<
                                    schema::metric::table,
                                    diesel::internal::table_macro::SelectStatement<
                                        diesel::internal::table_macro::FromClause<
                                            diesel::helper_types::InnerJoinQuerySource<
                                                diesel::helper_types::InnerJoinQuerySource<
                                                    schema::report_benchmark::table,
                                                    schema::report::table,
                                                >,
                                                schema::benchmark::table,
                                            >,
                                        >,
                                    >,
                                >,
                            >,
                        >,
                    >,
                >,
            >,
        >,
    >,
    diesel::sqlite::Sqlite,
>;

#[derive(Deserialize, JsonSchema)]
pub struct ProjAlertParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The UUID for an alert.
    pub alert: AlertUuid,
}

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
    let public_user = PublicUser::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &public_user).await?;
    Ok(Get::response_ok(json, public_user.is_auth()))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjAlertParams,
    public_user: &PublicUser,
) -> Result<JsonAlert, HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        public_user,
    )?;

    let alert = QueryAlert::from_uuid(conn_lock!(context), query_project.id, path_params.alert)?;

    alert.into_json(conn_lock!(context))
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

    auth_conn!(context, |conn| QueryAlert::get(conn, query_alert.id)?
        .into_json(conn))
}
