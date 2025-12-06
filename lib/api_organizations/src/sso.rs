#![cfg(feature = "plus")]

use bencher_endpoint::{CorsResponse, Delete, Endpoint, Post, ResponseAccepted, ResponseDeleted};
use bencher_json::{JsonNewSso, JsonSso, OrganizationResourceId, SsoUuid};
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
        user::{admin::AdminUser, auth::BearerToken},
    },
    resource_not_found_err, schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct OrgSsoPostParams {
    /// The slug or UUID for an organization.
    pub organization: OrganizationResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/sso",
    tags = ["organizations", "sso"]
}]
pub async fn org_sso_post_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgSsoPostParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
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
    path_params: Path<OrgSsoPostParams>,
    body: TypedBody<JsonNewSso>,
) -> Result<ResponseAccepted<JsonSso>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(rqctx.context(), path_params.into_inner(), body.into_inner()).await?;
    Ok(Post::auth_response_accepted(json))
}

async fn post_inner(
    context: &ApiContext,
    path_params: OrgSsoPostParams,
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
pub struct OrgSsoDeleteParams {
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
    _path_params: Path<OrgSsoDeleteParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Delete.into()]))
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
    path_params: Path<OrgSsoDeleteParams>,
) -> Result<ResponseDeleted, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner()).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: OrgSsoDeleteParams,
) -> Result<(), HttpError> {
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
