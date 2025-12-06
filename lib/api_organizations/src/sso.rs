#![cfg(feature = "plus")]

use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Post, ResponseAccepted, ResponseDeleted, ResponseOk,
    TotalCount,
};
use bencher_json::{
    JsonDirection, JsonNewSso, JsonPagination, JsonSso, JsonSsos, OrganizationResourceId, SsoUuid,
};
use bencher_rbac::organization::Permission;
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::{payment_required_error, resource_conflict_err},
    model::{
        organization::{
            QueryOrganization,
            plan::LicenseUsage,
            sso::{InsertSso, QuerySso},
        },
        user::{
            admin::AdminUser,
            auth::{AuthUser, BearerToken},
        },
    },
    resource_not_found_err, schema,
};
use diesel::{BelongingToDsl as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct OrgSsosParams {
    /// The slug or UUID for an organization.
    pub organization: OrganizationResourceId,
}

pub type OrgSsoPagination = JsonPagination<OrgSsoSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrgSsoSort {
    /// Sort by SSO domain.
    #[default]
    Domain,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/sso",
    tags = ["organizations", "sso"]
}]
pub async fn org_ssos_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgSsosParams>,
    _pagination_params: Query<OrgSsoPagination>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// List SSO domains for an organization
///
/// ➕ Bencher Plus: List all single sign-on (SSO) domains for an organization.
/// The user must be a member of the organization to use this route.
#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/sso",
    tags = ["organizations", "sso"]
}]
pub async fn org_ssos_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgSsosParams>,
    pagination_params: Query<OrgSsoPagination>,
) -> Result<ResponseOk<JsonSsos>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        &auth_user,
        path_params.into_inner(),
        pagination_params.into_inner(),
    )
    .await?;
    Ok(Get::auth_response_ok_with_total_count(json, total_count))
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: &AuthUser,
    path_params: OrgSsosParams,
    pagination_params: OrgSsoPagination,
) -> Result<(JsonSsos, TotalCount), HttpError> {
    let query_organization = QueryOrganization::is_allowed_resource_id(
        conn_lock!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::View,
    )?;

    let ssos = get_ls_query(&query_organization, &pagination_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QuerySso>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Sso,
            (&query_organization, &pagination_params)
        ))?;

    // Drop connection lock before iterating
    let json_ssos = ssos.into_iter().map(QuerySso::into_json).collect();

    let total_count = get_ls_query(&query_organization, &pagination_params)
        .count()
        .get_result::<i64>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Sso,
            (&query_organization, &pagination_params)
        ))?
        .try_into()?;

    Ok((json_ssos, total_count))
}

fn get_ls_query<'q>(
    query_organization: &'q QueryOrganization,
    pagination_params: &OrgSsoPagination,
) -> schema::sso::BoxedQuery<'q, diesel::sqlite::Sqlite> {
    let query = QuerySso::belonging_to(query_organization).into_boxed();

    match pagination_params.order() {
        OrgSsoSort::Domain => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::sso::domain.asc()),
            Some(JsonDirection::Desc) => query.order(schema::sso::domain.desc()),
        },
    }
}

/// Add an SSO domain to an organization
///
/// ➕ Bencher Plus: Add a single sign-on (SSO) domain to an organization.
/// The user must be an admin on the server to use this route.
#[endpoint {
    method = POST,
    path =  "/v0/organizations/{organization}/sso",
    tags = ["organizations", "sso"]
}]
pub async fn org_sso_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgSsosParams>,
    body: TypedBody<JsonNewSso>,
) -> Result<ResponseAccepted<JsonSso>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(rqctx.context(), path_params.into_inner(), body.into_inner()).await?;
    Ok(Post::auth_response_accepted(json))
}

async fn post_inner(
    context: &ApiContext,
    path_params: OrgSsosParams,
    json_new_sso: JsonNewSso,
) -> Result<JsonSso, HttpError> {
    // Get the organization
    let query_organization =
        QueryOrganization::from_resource_id(conn_lock!(context), &path_params.organization)?;

    // Either the server is Bencher Cloud or the organization must have a valid Bencher Plus license
    let is_allowed = context.is_bencher_cloud
        || LicenseUsage::get(
            &context.database.connection,
            &context.licensor,
            &query_organization,
        )
        .await?
        .is_some();
    if !is_allowed {
        return Err(payment_required_error(
            "You must have a valid Bencher Plus Enterprise license for the organization to add SSO",
        ));
    }

    let insert_sso = InsertSso::from_json(query_organization.id, json_new_sso);
    diesel::insert_into(schema::sso::table)
        .values(&insert_sso)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(
            Sso,
            (&query_organization, &insert_sso)
        ))?;

    let query_sso = QuerySso::from_uuid(conn_lock!(context), insert_sso.uuid).map_err(
        resource_not_found_err!(Sso, (&query_organization, &insert_sso)),
    )?;

    Ok(query_sso.into_json())
}

#[derive(Deserialize, JsonSchema)]
pub struct OrgSsoParams {
    /// The slug or UUID for an organization.
    pub organization: OrganizationResourceId,
    /// The UUID for an SSO domain.
    pub sso: SsoUuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/sso/{sso}",
    tags = ["organizations", "sso"]
}]
pub async fn org_sso_delete_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgSsoParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Delete.into()]))
}

/// View an SSO domain for an organization
///
/// ➕ Bencher Plus: View a single sign-on (SSO) domain from an organization.
/// The user must be a member of the organization to use this route.
#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/sso/{sso}",
    tags = ["organizations", "sso"]
}]
pub async fn org_sso_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgSsoParams>,
) -> Result<ResponseOk<JsonSso>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: OrgSsoParams,
    auth_user: &AuthUser,
) -> Result<JsonSso, HttpError> {
    let query_organization = QueryOrganization::is_allowed_resource_id(
        conn_lock!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::View,
    )?;

    QuerySso::belonging_to(&query_organization)
        .filter(schema::sso::uuid.eq(path_params.sso))
        .first::<QuerySso>(conn_lock!(context))
        .map(QuerySso::into_json)
        .map_err(resource_not_found_err!(
            Sso,
            (&query_organization, path_params.sso)
        ))
}

/// Remove an SSO domain from an organization
///
/// ➕ Bencher Plus: Remove a single sign-on (SSO) domain from an organization.
/// The user must be an admin on the server to use this route.
#[endpoint {
    method = DELETE,
    path =  "/v0/organizations/{organization}/sso/{sso}",
    tags = ["organizations", "sso"]
}]
pub async fn org_sso_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgSsoParams>,
) -> Result<ResponseDeleted, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner()).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(context: &ApiContext, path_params: OrgSsoParams) -> Result<(), HttpError> {
    // Get the organization
    let query_organization =
        QueryOrganization::from_resource_id(conn_lock!(context), &path_params.organization)?;

    diesel::delete(
        schema::sso::table
            .filter(schema::sso::organization_id.eq(query_organization.id))
            .filter(schema::sso::uuid.eq(path_params.sso)),
    )
    .execute(conn_lock!(context))
    .map_err(resource_conflict_err!(
        Sso,
        (&query_organization, path_params.sso)
    ))?;

    Ok(())
}
