#![cfg(feature = "plus")]

use bencher_json::{organization::usage::JsonUsage, ResourceId};
use bencher_rbac::organization::Permission;
use diesel::{dsl::count, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{response_ok, ResponseOk},
        organization::Resource,
        Endpoint, Method,
    },
    model::{organization::QueryOrganization, user::auth::AuthUser},
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        to_date_time,
    },
    ApiError,
};

const USAGE_RESOURCE: Resource = Resource::Usage;

#[derive(Deserialize, JsonSchema)]
pub struct OrgUsageParams {
    pub organization: ResourceId,
}

#[derive(Clone, Deserialize, JsonSchema)]
pub struct OrgUsageQuery {
    pub start: i64,
    pub end: i64,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/usage",
    tags = ["organizations", "usage"]
}]
pub async fn org_usage_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgUsageParams>,
    _query_params: Query<OrgUsageQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path = "/v0/organizations/{organization}/usage",
    tags = ["organizations", "usage"]
}]
pub async fn org_usage_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgUsageParams>,
    query_params: Query<OrgUsageQuery>,
) -> Result<ResponseOk<JsonUsage>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(USAGE_RESOURCE, Method::GetOne);

    let json = get_inner(
        rqctx.context(),
        path_params.into_inner(),
        query_params.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

    response_ok!(endpoint, json)
}

async fn get_inner(
    context: &ApiContext,
    path_params: OrgUsageParams,
    query_params: OrgUsageQuery,
    auth_user: &AuthUser,
) -> Result<JsonUsage, ApiError> {
    let conn = &mut *context.conn().await;

    // Get the organization
    let query_org = QueryOrganization::from_resource_id(conn, &path_params.organization)?;
    // Check to see if user has permission to manage a project within the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_org)?;

    let OrgUsageQuery { start, end } = query_params;
    let start_time = to_date_time(start)?.timestamp();
    let end_time = to_date_time(end)?.timestamp();

    let metrics_used = schema::metric::table
        .inner_join(
            schema::perf::table
                .inner_join(schema::benchmark::table.inner_join(schema::project::table))
                .inner_join(schema::report::table),
        )
        .filter(schema::project::organization_id.eq(query_org.id))
        .filter(schema::report::end_time.ge(start_time))
        .filter(schema::report::end_time.le(end_time))
        .select(count(schema::metric::value))
        .first::<i64>(conn)
        .map_err(ApiError::from)?;

    Ok(JsonUsage {
        metrics_used: u64::try_from(metrics_used)?.into(),
    })
}
