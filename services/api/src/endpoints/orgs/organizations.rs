use std::sync::Arc;

use bencher_json::{JsonNewOrganization, JsonOrganization, ResourceId};
use bencher_rbac::organization::{Permission, Role};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{
    endpoint, HttpError, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk, Path,
    RequestContext, TypedBody,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    endpoints::{
        endpoint::{response_ok, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::{
        organization::{InsertOrganization, QueryOrganization},
        user::{auth::AuthUser, organization::InsertOrganizationRole, QueryUser},
    },
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        headers::CorsHeaders,
        map_http_error,
        resource_id::fn_resource_id,
        Context,
    },
    ApiError,
};

use super::Resource;

const ORGANIZATION_RESOURCE: Resource = Resource::Organization;

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations",
    tags = ["organizations"]
}]
pub async fn dir_options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path = "/v0/organizations",
    tags = ["organizations"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
) -> Result<ResponseOk<Vec<JsonOrganization>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(ORGANIZATION_RESOURCE, Method::GetLs);

    let context = rqctx.context();
    let json = get_ls_inner(&auth_user, context)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_ls_inner(
    auth_user: &AuthUser,
    context: &Context,
) -> Result<Vec<JsonOrganization>, ApiError> {
    let context = &mut *context.lock().await;
    let conn = &mut context.db_conn;

    let json: Vec<JsonOrganization> = schema::organization::table
        .order(schema::organization::name)
        .load::<QueryOrganization>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(|query| {
            context
                .rbac
                .is_allowed_organization(auth_user, Permission::View, query)
        })
        .filter_map(|query| query.into_json().ok())
        .collect();

    Ok(json)
}

#[endpoint {
    method = POST,
    path = "/v0/organizations",
    tags = ["organizations"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonNewOrganization>,
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonOrganization>, CorsHeaders>, HttpError> {
    let user_id = QueryUser::auth(&rqctx).await?;

    let json_organization = body.into_inner();

    let context = &mut *rqctx.context().lock().await;
    let conn = &mut context.db_conn;

    // Create the organization
    let insert_organization = InsertOrganization::from_json(conn, json_organization)?;
    diesel::insert_into(schema::organization::table)
        .values(&insert_organization)
        .execute(conn)
        .map_err(map_http_error!("Failed to create organization."))?;
    let query_organization = schema::organization::table
        .filter(schema::organization::uuid.eq(&insert_organization.uuid))
        .first::<QueryOrganization>(conn)
        .map_err(map_http_error!("Failed to create organization."))?;

    // Connect the user to the organization as a `Maintainer`
    let insert_org_role = InsertOrganizationRole {
        user_id,
        organization_id: query_organization.id,
        role: Role::Leader.to_string(),
    };
    diesel::insert_into(schema::organization_role::table)
        .values(&insert_org_role)
        .execute(conn)
        .map_err(map_http_error!("Failed to create organization."))?;

    let json = query_organization.into_json()?;

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(json),
        CorsHeaders::new_auth("POST".into()),
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct PathParams {
    pub organization: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}",
    tags = ["organizations"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<PathParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

fn_resource_id!(organization);

#[endpoint {
    method = GET,
    path = "/v0/organizations/{organization}",
    tags = ["organizations"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<PathParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonOrganization>, CorsHeaders>, HttpError> {
    let user_id = QueryUser::auth(&rqctx).await?;
    let path_params = path_params.into_inner();

    let context = &mut *rqctx.context().lock().await;
    let conn = &mut context.db_conn;

    let organization = path_params.organization;
    let query = schema::organization::table
        .filter(resource_id(&organization)?)
        .first::<QueryOrganization>(conn)
        .map_err(map_http_error!("Failed to get organization."))?;

    QueryUser::has_access(conn, user_id, query.id)?;
    let json = query.into_json()?;

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}
