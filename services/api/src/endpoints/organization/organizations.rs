use bencher_json::{
    organization::JsonUpdateOrganization, JsonDirection, JsonNewOrganization, JsonOrganization,
    JsonOrganizations, JsonPagination, NonEmpty, ResourceId,
};
use bencher_rbac::organization::{Permission, Role};
use chrono::Utc;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::{
        organization::{
            organization_role::InsertOrganizationRole, InsertOrganization, QueryOrganization,
        },
        user::auth::AuthUser,
    },
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
    },
    ApiError,
};

use super::Resource;

const ORGANIZATION_RESOURCE: Resource = Resource::Organization;

pub type OrganizationsQuery = JsonPagination<OrganizationsSort, OrganizationsQueryParams>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationsSort {
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct OrganizationsQueryParams {
    pub name: Option<NonEmpty>,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations",
    tags = ["organizations"]
}]
pub async fn organizations_options(
    _rqctx: RequestContext<ApiContext>,
    _query_params: Query<OrganizationsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path = "/v0/organizations",
    tags = ["organizations"]
}]
pub async fn organizations_get(
    rqctx: RequestContext<ApiContext>,
    query_params: Query<OrganizationsQuery>,
) -> Result<ResponseOk<JsonOrganizations>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(ORGANIZATION_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        &auth_user,
        query_params.into_inner(),
        endpoint,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: &AuthUser,
    query_params: OrganizationsQuery,
    endpoint: Endpoint,
) -> Result<JsonOrganizations, ApiError> {
    let conn = &mut *context.conn().await;

    let mut query = schema::organization::table.into_boxed();

    if !auth_user.is_admin(&context.rbac) {
        let organizations = auth_user.organizations(&context.rbac, Permission::View);
        query = query.filter(schema::organization::id.eq_any(organizations));
    }

    if let Some(name) = query_params.query.name.as_ref() {
        query = query.filter(schema::organization::name.eq(name.as_ref()));
    }

    query = match query_params.order() {
        OrganizationsSort::Name => match query_params.direction {
            Some(JsonDirection::Asc) | None => query.order((
                schema::organization::name.asc(),
                schema::organization::slug.asc(),
            )),
            Some(JsonDirection::Desc) => query.order((
                schema::organization::name.desc(),
                schema::organization::slug.desc(),
            )),
        },
    };

    Ok(query
        .offset(query_params.offset())
        .limit(query_params.limit())
        .load::<QueryOrganization>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(into_json!(endpoint))
        .collect())
}

#[endpoint {
    method = POST,
    path = "/v0/organizations",
    tags = ["organizations"]
}]
pub async fn organization_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonNewOrganization>,
) -> Result<ResponseAccepted<JsonOrganization>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(ORGANIZATION_RESOURCE, Method::Post);

    let json = post_inner(rqctx.context(), body.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &ApiContext,
    json_organization: JsonNewOrganization,
    auth_user: &AuthUser,
) -> Result<JsonOrganization, ApiError> {
    let conn = &mut *context.conn().await;

    if !auth_user.is_admin(&context.rbac) {
        return Err(ApiError::CreateOrganization(auth_user.id));
    }

    // Create the organization
    let insert_organization = InsertOrganization::from_json(conn, json_organization);
    diesel::insert_into(schema::organization::table)
        .values(&insert_organization)
        .execute(conn)
        .map_err(api_error!())?;
    let query_organization = schema::organization::table
        .filter(schema::organization::uuid.eq(&insert_organization.uuid))
        .first::<QueryOrganization>(conn)
        .map_err(api_error!())?;

    let timestamp = Utc::now().timestamp();
    // Connect the user to the organization as a `Maintainer`
    let insert_org_role = InsertOrganizationRole {
        user_id: auth_user.id,
        organization_id: query_organization.id,
        role: Role::Leader.to_string(),
        created: timestamp,
        modified: timestamp,
    };
    diesel::insert_into(schema::organization_role::table)
        .values(&insert_org_role)
        .execute(conn)
        .map_err(api_error!())?;

    query_organization.into_json()
}

#[derive(Deserialize, JsonSchema)]
pub struct OrganizationParams {
    pub organization: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}",
    tags = ["organizations"]
}]
pub async fn organization_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrganizationParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path = "/v0/organizations/{organization}",
    tags = ["organizations"]
}]
pub async fn organization_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrganizationParams>,
) -> Result<ResponseOk<JsonOrganization>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(ORGANIZATION_RESOURCE, Method::GetOne);

    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: OrganizationParams,
    auth_user: &AuthUser,
) -> Result<JsonOrganization, ApiError> {
    let conn = &mut *context.conn().await;

    QueryOrganization::is_allowed_resource_id(
        conn,
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::View,
    )?
    .into_json()
}

#[endpoint {
    method = PATCH,
    path =  "/v0/organizations/{organization}",
    tags = ["organizations"]
}]
pub async fn organization_patch(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrganizationParams>,
    body: TypedBody<JsonUpdateOrganization>,
) -> Result<ResponseAccepted<JsonOrganization>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(ORGANIZATION_RESOURCE, Method::Put);

    let context = rqctx.context();
    let json = patch_inner(
        context,
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn patch_inner(
    context: &ApiContext,
    path_params: OrganizationParams,
    json_update_organization: JsonUpdateOrganization,
    auth_user: &AuthUser,
) -> Result<JsonOrganization, ApiError> {
    let conn = &mut *context.conn().await;

    let query_organization = QueryOrganization::is_allowed_resource_id(
        conn,
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::Edit,
    )?;

    let JsonUpdateOrganization { name, slug } = json_update_organization;

    if let Some(name) = name {
        diesel::update(
            schema::organization::table.filter(schema::organization::id.eq(query_organization.id)),
        )
        .set(schema::organization::name.eq(name.as_ref()))
        .execute(conn)
        .map_err(api_error!())?;
    }

    if let Some(slug) = slug {
        diesel::update(
            schema::organization::table.filter(schema::organization::id.eq(query_organization.id)),
        )
        .set(schema::organization::slug.eq(slug.as_ref()))
        .execute(conn)
        .map_err(api_error!())?;
    }

    QueryOrganization::get(conn, query_organization.id)?.into_json()
}
