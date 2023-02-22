#![cfg(feature = "plus")]

use bencher_json::{organization::entitlements::JsonEntitlements, ResourceId};
use bencher_rbac::organization::Permission;
use chrono::NaiveDate;
use diesel::{dsl::count, ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::Context,
    endpoints::{
        endpoint::{response_ok, ResponseOk},
        organization::Resource,
        Endpoint, Method,
    },
    error::api_error,
    model::{organization::QueryOrganization, user::auth::AuthUser},
    schema,
    util::cors::{get_cors, CorsResponse},
    ApiError,
};

const USAGE_RESOURCE: Resource = Resource::Usage;

#[derive(Deserialize, JsonSchema)]
pub struct GetParams {
    pub organization: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/usage",
    tags = ["organizations", "usage"]
}]
pub async fn options(
    _rqctx: RequestContext<Context>,
    _path_params: Path<GetParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path = "/v0/organizations/{organization}/usage",
    tags = ["organizations", "usage"]
}]
pub async fn get(
    rqctx: RequestContext<Context>,
    path_params: Path<GetParams>,
) -> Result<ResponseOk<JsonEntitlements>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(USAGE_RESOURCE, Method::GetOne);

    let json = get_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_inner(
    context: &Context,
    path_params: GetParams,
    auth_user: &AuthUser,
) -> Result<JsonEntitlements, ApiError> {
    let api_context = &mut *context.lock().await;
    let conn = &mut api_context.database.connection;

    // Get the organization
    let query_org = QueryOrganization::from_resource_id(conn, &path_params.organization)?;

    // Check to see if user has permission to manage a project within the organization
    api_context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_org)?;

    // TODO this should actually be the current billing period
    let year = 2022;
    let month = 1;
    let day = 1;

    let hours = 0;
    let min = 0;
    let sec = 0;

    let start_billing_period = NaiveDate::from_ymd_opt(year, month, day)
        .ok_or(ApiError::BadDate(year, month, day))?
        .and_hms_opt(hours, min, sec)
        .ok_or(ApiError::BadTime(hours, min, sec))?
        .timestamp();

    let metrics_used = schema::metric::table
        .left_join(schema::perf::table.on(schema::metric::perf_id.eq(schema::perf::id)))
        .left_join(
            schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
        )
        .left_join(schema::project::table.on(schema::benchmark::project_id.eq(schema::project::id)))
        .filter(schema::project::organization_id.eq(query_org.id))
        .inner_join(schema::report::table.on(schema::perf::report_id.eq(schema::report::id)))
        .filter(schema::report::start_time.ge(start_billing_period))
        .select(count(schema::metric::value))
        .first::<i64>(conn)
        .map_err(api_error!())?;

    Ok(JsonEntitlements {
        metrics_used: u64::try_from(metrics_used)?,
    })
}
