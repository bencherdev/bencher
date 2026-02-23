use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Patch, Post, ResponseCreated, ResponseDeleted, ResponseOk,
    TotalCount,
};
use bencher_json::{
    JsonDirection, JsonNewOrganization, JsonOrganization, JsonOrganizations, JsonPagination,
    OrganizationResourceId, ResourceName, Search, organization::JsonUpdateOrganization,
};
use bencher_rbac::organization::Permission;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{forbidden_error, resource_conflict_err, resource_not_found_err},
    model::{
        organization::{InsertOrganization, QueryOrganization, UpdateOrganization},
        user::auth::{AuthUser, BearerToken},
    },
    schema, write_conn,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    TextExpressionMethods as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

pub type OrganizationsPagination = JsonPagination<OrganizationsSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationsSort {
    /// Sort by organization name.
    #[default]
    Name,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OrganizationsQuery {
    /// Filter by organization name, exact match.
    pub name: Option<ResourceName>,
    /// Search by organization name, slug, or UUID.
    pub search: Option<Search>,
}

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
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// List organizations
///
/// List all organizations where the user is a member.
/// The user must have `view` permissions for each organization.
/// By default, the organizations are sorted in alphabetical order by name.
/// The HTTP response header `X-Total-Count` contains the total number of organizations.
#[endpoint {
    method = GET,
    path = "/v0/organizations",
    tags = ["organizations"]
}]
pub async fn organizations_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    pagination_params: Query<OrganizationsPagination>,
    query_params: Query<OrganizationsQuery>,
) -> Result<ResponseOk<JsonOrganizations>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        &auth_user,
        pagination_params.into_inner(),
        query_params.into_inner(),
    )
    .await?;
    Ok(Get::auth_response_ok_with_total_count(json, total_count))
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: &AuthUser,
    pagination_params: OrganizationsPagination,
    query_params: OrganizationsQuery,
) -> Result<(JsonOrganizations, TotalCount), HttpError> {
    let organizations = get_ls_query(context, auth_user, &pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryOrganization>(auth_conn!(context))
        .map_err(resource_not_found_err!(
            Organization,
            (auth_user, &pagination_params, &query_params)
        ))?;

    // Drop connection lock before iterating
    let json_organizations = auth_conn!(context, |conn| {
        organizations
            .into_iter()
            .map(|org| org.into_json(conn))
            .collect()
    });

    let total_count = get_ls_query(context, auth_user, &pagination_params, &query_params)
        .count()
        .get_result::<i64>(auth_conn!(context))
        .map_err(resource_not_found_err!(
            Organization,
            (auth_user, &pagination_params, &query_params)
        ))?
        .try_into()?;

    Ok((json_organizations, total_count))
}

fn get_ls_query<'q>(
    context: &ApiContext,
    auth_user: &AuthUser,
    pagination_params: &OrganizationsPagination,
    query_params: &'q OrganizationsQuery,
) -> schema::organization::BoxedQuery<'q, diesel::sqlite::Sqlite> {
    let mut query = schema::organization::table.into_boxed();

    query = query.filter(schema::organization::deleted.is_null());

    if !auth_user.is_admin(&context.rbac) {
        let organizations = auth_user.organizations(&context.rbac, Permission::View);
        query = query.filter(schema::organization::id.eq_any(organizations));
    }

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::organization::name.eq(name));
    }
    if let Some(search) = query_params.search.as_ref() {
        query = query.filter(
            schema::organization::name
                .like(search)
                .or(schema::organization::slug.like(search))
                .or(schema::organization::uuid.like(search)),
        );
    }

    match pagination_params.order() {
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
    }
}

/// Create an organization
///
/// Create a new organization.
/// The user must be authenticated to use this route.
#[endpoint {
    method = POST,
    path = "/v0/organizations",
    tags = ["organizations"]
}]
pub async fn organization_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    body: TypedBody<JsonNewOrganization>,
) -> Result<ResponseCreated<JsonOrganization>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(rqctx.context(), body.into_inner(), &auth_user).await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    context: &ApiContext,
    json_organization: JsonNewOrganization,
    auth_user: &AuthUser,
) -> Result<JsonOrganization, HttpError> {
    let insert_organization = InsertOrganization::from_json(auth_conn!(context), json_organization);
    let query_organization =
        QueryOrganization::create(context, auth_user, insert_organization).await?;
    Ok(query_organization.into_json(auth_conn!(context)))
}

#[derive(Deserialize, JsonSchema)]
pub struct OrganizationParams {
    /// The slug or UUID for an organization.
    pub organization: OrganizationResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}",
    tags = ["organizations"]
}]
pub async fn organization_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrganizationParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into(), Delete.into()]))
}

/// View an organization
///
/// View an organization where the user is a member.
/// The user must have `view` permissions for the organization.
#[endpoint {
    method = GET,
    path = "/v0/organizations/{organization}",
    tags = ["organizations"]
}]
pub async fn organization_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrganizationParams>,
) -> Result<ResponseOk<JsonOrganization>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: OrganizationParams,
    auth_user: &AuthUser,
) -> Result<JsonOrganization, HttpError> {
    auth_conn!(context, |conn| {
        QueryOrganization::is_allowed_resource_id(
            conn,
            &context.rbac,
            &path_params.organization,
            auth_user,
            Permission::View,
        )?
        .into_json_full(conn)
    })
}

/// Update an organization
///
/// Update an organization where the user is a member.
/// The user must have `edit` permissions for the organization.
/// If updating the license, the user must have `manage` permissions for the organization.
#[endpoint {
    method = PATCH,
    path =  "/v0/organizations/{organization}",
    tags = ["organizations"]
}]
pub async fn organization_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrganizationParams>,
    body: TypedBody<JsonUpdateOrganization>,
) -> Result<ResponseOk<JsonOrganization>, HttpError> {
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
    path_params: OrganizationParams,
    json_organization: JsonUpdateOrganization,
    auth_user: &AuthUser,
) -> Result<JsonOrganization, HttpError> {
    #[cfg(not(feature = "plus"))]
    let permission = Permission::Edit;
    #[cfg(feature = "plus")]
    let license = json_organization.license();
    #[cfg(feature = "plus")]
    let permission = if license.is_some() {
        // Manage permission is required to update the license
        Permission::Manage
    } else {
        Permission::Edit
    };
    let query_organization = QueryOrganization::is_allowed_resource_id(
        auth_conn!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        permission,
    )?;
    #[cfg(feature = "plus")]
    if let Some(license) = license {
        // If updating the license make sure that the server is not Bencher Cloud
        // All Bencher Cloud license updates should be handled via plans directly
        // Only Self-Hosted should be able to update the license
        if context.is_bencher_cloud {
            return Err(bencher_schema::error::locked_error(
                "Direct license updates are not allowed on Bencher Cloud. Please update your plan instead.",
            ));
        }
        // If updating a Self-Hosted license make sure that it is actually valid for this particular organization
        context
            .licensor
            .validate_organization(license, query_organization.uuid)
            .map_err(resource_not_found_err!(
                Organization,
                (license, &query_organization)
            ))?;
    }

    let organization_query =
        schema::organization::table.filter(schema::organization::id.eq(query_organization.id));
    let update_organization = UpdateOrganization::from(json_organization);
    diesel::update(organization_query)
        .set(&update_organization)
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(Organization, update_organization))?;

    auth_conn!(context, |conn| {
        QueryOrganization::get(conn, query_organization.id)?.into_json_full(conn)
    })
}

/// Delete an organization
///
/// Delete an organization where the user is a member.
/// The user must have `delete` permissions for the organization.
/// By default, organizations are soft-deleted (along with their child projects).
/// Use `?hard=true` to permanently delete the organization (requires server admin).
#[endpoint {
    method = DELETE,
    path =  "/v0/organizations/{organization}",
    tags = ["organizations"]
}]
pub async fn organization_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrganizationParams>,
    query_params: Query<OrganizationDeleteQuery>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(
        rqctx.context(),
        path_params.into_inner(),
        query_params.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Delete::auth_response_deleted())
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OrganizationDeleteQuery {
    /// Hard delete the organization (requires server admin).
    pub hard: Option<bool>,
}

async fn delete_inner(
    context: &ApiContext,
    path_params: OrganizationParams,
    query_params: OrganizationDeleteQuery,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    // Verify that the user is allowed
    let query_organization = QueryOrganization::is_allowed_resource_id(
        auth_conn!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::Delete,
    )?;

    if query_params.hard.unwrap_or_default() {
        // Hard delete requires server admin
        if !auth_user.is_admin(&context.rbac) {
            return Err(forbidden_error(format!(
                "Hard delete requires server admin: {auth_user:?}"
            )));
        }
        diesel::delete(
            schema::organization::table.filter(schema::organization::id.eq(query_organization.id)),
        )
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(Organization, query_organization))?;
    } else {
        // Soft delete: timestamp + mangle slug/name to free UNIQUE constraints
        let now = context.clock.now();
        let deleted_name = format!(
            "{}--deleted-{}",
            query_organization.name, query_organization.uuid
        );
        let deleted_slug = format!(
            "{}--deleted-{}",
            query_organization.slug, query_organization.uuid
        );

        let conn = write_conn!(context);

        // Bulk soft-delete all non-deleted projects under this org
        diesel::update(
            schema::project::table
                .filter(schema::project::organization_id.eq(query_organization.id))
                .filter(schema::project::deleted.is_null()),
        )
        .set((
            schema::project::deleted.eq(now),
            schema::project::name.eq(diesel::dsl::sql::<diesel::sql_types::Text>(
                "name || '--deleted-' || uuid",
            )),
            schema::project::slug.eq(diesel::dsl::sql::<diesel::sql_types::Text>(
                "slug || '--deleted-' || uuid",
            )),
        ))
        .execute(conn)
        .map_err(resource_conflict_err!(Organization, query_organization))?;

        // Soft-delete the org
        diesel::update(
            schema::organization::table.filter(schema::organization::id.eq(query_organization.id)),
        )
        .set((
            schema::organization::deleted.eq(now),
            schema::organization::name.eq(deleted_name),
            schema::organization::slug.eq(deleted_slug),
        ))
        .execute(conn)
        .map_err(resource_conflict_err!(Organization, query_organization))?;
    }

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OrganizationDelete);

    Ok(())
}
