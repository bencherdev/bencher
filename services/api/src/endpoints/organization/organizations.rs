use bencher_json::{
    organization::{member::OrganizationRole, JsonUpdateOrganization},
    DateTime, JsonDirection, JsonNewOrganization, JsonOrganization, JsonOrganizations,
    JsonPagination, NonEmpty, ResourceId,
};
use bencher_rbac::organization::Permission;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{response_accepted, response_ok, CorsResponse, ResponseAccepted, ResponseOk},
        Endpoint,
    },
    error::resource_not_found_err,
    model::{
        organization::{
            organization_role::InsertOrganizationRole, InsertOrganization, QueryOrganization,
            UpdateOrganization,
        },
        user::auth::AuthUser,
    },
    schema, ApiError,
};

pub type OrganizationsPagination = JsonPagination<OrganizationsSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationsSort {
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct OrganizationsQuery {
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
    _pagination_params: Query<OrganizationsPagination>,
    _query_params: Query<OrganizationsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Endpoint::GetLs, Endpoint::Post]))
}

#[endpoint {
    method = GET,
    path = "/v0/organizations",
    tags = ["organizations"]
}]
pub async fn organizations_get(
    rqctx: RequestContext<ApiContext>,
    pagination_params: Query<OrganizationsPagination>,
    query_params: Query<OrganizationsQuery>,
) -> Result<ResponseOk<JsonOrganizations>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let json = get_ls_inner(
        rqctx.context(),
        &auth_user,
        pagination_params.into_inner(),
        query_params.into_inner(),
    )
    .await?;
    Ok(Endpoint::GetLs.response_ok(json))
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: &AuthUser,
    pagination_params: OrganizationsPagination,
    query_params: OrganizationsQuery,
) -> Result<JsonOrganizations, HttpError> {
    let conn = &mut *context.conn().await;

    let mut query = schema::organization::table.into_boxed();

    if !auth_user.is_admin(&context.rbac) {
        let organizations = auth_user.organizations(&context.rbac, Permission::View);
        query = query.filter(schema::organization::id.eq_any(organizations));
    }

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::organization::name.eq(name.as_ref()));
    }

    query = match pagination_params.order() {
        OrganizationsSort::Name => match pagination_params.direction {
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
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryOrganization>(conn)
        .map_err(resource_not_found_err!(Organization))?
        .into_iter()
        .map(QueryOrganization::into_json)
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
    let endpoint = Endpoint::Post;

    let json = post_inner(rqctx.context(), body.into_inner(), &auth_user)
        .await
        .map_err(|e| {
            if let ApiError::HttpError(e) = e {
                e
            } else {
                endpoint.err(e).into()
            }
        })?;

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
        .map_err(ApiError::from)?;
    let query_organization = schema::organization::table
        .filter(schema::organization::uuid.eq(&insert_organization.uuid))
        .first::<QueryOrganization>(conn)
        .map_err(ApiError::from)?;

    let timestamp = DateTime::now();
    // Connect the user to the organization as a `Maintainer`
    let insert_org_role = InsertOrganizationRole {
        user_id: auth_user.id,
        organization_id: query_organization.id,
        role: OrganizationRole::Leader,
        created: timestamp,
        modified: timestamp,
    };
    diesel::insert_into(schema::organization_role::table)
        .values(&insert_org_role)
        .execute(conn)
        .map_err(ApiError::from)?;

    Ok(query_organization.into_json())
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
    Ok(Endpoint::cors(&[Endpoint::GetOne, Endpoint::Patch]))
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
    let endpoint = Endpoint::GetOne;

    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user)
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

async fn get_one_inner(
    context: &ApiContext,
    path_params: OrganizationParams,
    auth_user: &AuthUser,
) -> Result<JsonOrganization, ApiError> {
    let conn = &mut *context.conn().await;

    Ok(QueryOrganization::is_allowed_resource_id(
        conn,
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::View,
    )?
    .into_json())
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
    let endpoint = Endpoint::Patch;

    let context = rqctx.context();
    let json = patch_inner(
        context,
        path_params.into_inner(),
        body.into_inner(),
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

    response_accepted!(endpoint, json)
}

async fn patch_inner(
    context: &ApiContext,
    path_params: OrganizationParams,
    json_organization: JsonUpdateOrganization,
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

    let organization_query =
        schema::organization::table.filter(schema::organization::id.eq(query_organization.id));
    let update_organization = UpdateOrganization::from(json_organization);
    diesel::update(organization_query)
        .set(&update_organization)
        .execute(conn)
        .map_err(ApiError::from)?;

    Ok(QueryOrganization::get(conn, query_organization.id)?.into_json())
}
