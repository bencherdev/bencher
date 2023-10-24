#![cfg(feature = "plus")]

use bencher_json::{organization::usage::JsonUsage, DateTimeMillis, ResourceId};
use bencher_rbac::organization::Permission;
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    error::forbidden_error,
    model::{
        organization::QueryOrganization,
        project::metric::QueryMetric,
        user::auth::{AuthUser, BearerToken},
    },
};

#[derive(Deserialize, JsonSchema)]
pub struct OrgUsageParams {
    pub organization: ResourceId,
}

#[derive(Clone, Deserialize, JsonSchema)]
pub struct OrgUsageQuery {
    pub start: DateTimeMillis,
    pub end: DateTimeMillis,
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
    Ok(Endpoint::cors(&[Get.into()]))
}

#[endpoint {
    method = GET,
    path = "/v0/organizations/{organization}/usage",
    tags = ["organizations", "usage"]
}]
pub async fn org_usage_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgUsageParams>,
    query_params: Query<OrgUsageQuery>,
) -> Result<ResponseOk<JsonUsage>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_inner(
        rqctx.context(),
        path_params.into_inner(),
        query_params.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_inner(
    context: &ApiContext,
    path_params: OrgUsageParams,
    query_params: OrgUsageQuery,
    auth_user: &AuthUser,
) -> Result<JsonUsage, HttpError> {
    let conn = &mut *context.conn().await;

    // Get the organization
    let query_org = QueryOrganization::from_resource_id(conn, &path_params.organization)?;
    // Check to see if user has permission to manage a project within the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_org)
        .map_err(forbidden_error)?;

    let OrgUsageQuery { start, end } = query_params;
    let metrics_used = QueryMetric::usage(conn, query_org.id, start.into(), end.into())?;

    Ok(JsonUsage { metrics_used })
}
