#![cfg(feature = "plus")]

use bencher_json::{
    organization::plan::{JsonNewPlan, JsonPlan},
    DateTime, ResourceId,
};
use bencher_rbac::organization::Permission;
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use http::StatusCode;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    conn_lock,
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Delete, Get, Post, ResponseCreated, ResponseDeleted, ResponseOk},
        Endpoint,
    },
    error::{
        forbidden_error, issue_error, resource_conflict_err, resource_conflict_error,
        resource_not_found_err, BencherResource,
    },
    model::{organization::QueryOrganization, user::auth::BearerToken},
    model::{
        organization::{
            plan::{InsertPlan, QueryPlan},
            UpdateOrganization,
        },
        user::auth::AuthUser,
    },
    schema,
};

#[derive(Deserialize, JsonSchema)]
pub struct OrgPlanParams {
    /// The slug or UUID for an organization.
    pub organization: ResourceId,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/plan",
    tags = ["organizations", "plan"]
}]
pub async fn org_plan_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgPlanParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into(), Delete.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/plan",
    tags = ["organizations", "plan"]
}]
pub async fn org_plan_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgPlanParams>,
) -> Result<ResponseOk<JsonPlan>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: OrgPlanParams,
    auth_user: &AuthUser,
) -> Result<JsonPlan, HttpError> {
    let biller = context.biller()?;

    // Get the organization
    let query_organization =
        QueryOrganization::from_resource_id(conn_lock!(context), &path_params.organization)?;
    // Check to see if user has permission to manage the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
        .map_err(forbidden_error)?;
    // Get the plan for the organization
    let query_plan = QueryPlan::belonging_to(&query_organization)
        .first::<QueryPlan>(conn_lock!(context))
        .map_err(resource_not_found_err!(Plan, query_organization))?;

    if let Some(json_plan) = query_plan.to_metered_plan(biller).await? {
        Ok(json_plan)
    } else if let Some(json_plan) = query_plan
        .to_licensed_plan(biller, &context.licensor, query_organization.uuid)
        .await?
    {
        Ok(json_plan)
    } else {
        Err(issue_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to find subscription for plan",
        &format!(
            "Failed to find plan (metered or licensed) for organization ({query_organization:?}) even though plan exists ({query_plan:?})."
             ),
        "Failed to find subscription for plan"
        ))
    }
}

#[endpoint {
    method = POST,
    path =  "/v0/organizations/{organization}/plan",
    tags = ["organizations", "plan"]
}]
pub async fn org_plan_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgPlanParams>,
    body: TypedBody<JsonNewPlan>,
) -> Result<ResponseCreated<JsonPlan>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| {
        #[cfg(feature = "sentry")]
        sentry::capture_error(&e);
        e
    })?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    context: &ApiContext,
    path_params: OrgPlanParams,
    json_plan: JsonNewPlan,
    auth_user: &AuthUser,
) -> Result<JsonPlan, HttpError> {
    let biller = context.biller()?;

    // Get the organization
    let query_organization =
        QueryOrganization::from_resource_id(conn_lock!(context), &path_params.organization)?;
    // Check to see if user has permission to manage the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
        .map_err(forbidden_error)?;
    // Check to make sure the organization doesn't already have a plan
    if let Ok(query_plan) =
        QueryPlan::belonging_to(&query_organization).first::<QueryPlan>(conn_lock!(context))
    {
        return Err(resource_conflict_error(
            BencherResource::Plan,
            (query_organization, query_plan),
            "Organization already has a plan",
        ));
    }

    let JsonNewPlan {
        checkout,
        level,
        entitlements,
        self_hosted,
        remote,
    } = json_plan;

    let subscription_id = if remote.unwrap_or(true) {
        biller
            .get_checkout_session(checkout.as_ref())
            .await
            .map_err(|e| {
                issue_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to get checkout session",
                    &format!("Failed to get checkout session {checkout}.",),
                    e,
                )
            })?
    } else {
        checkout.as_ref().parse().map_err(|e| {
            issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to parse subscription ID",
                &format!("Failed to parse subscription ID {checkout}.",),
                e,
            )
        })?
    };

    if let Some(entitlements) = entitlements {
        let licensed_plan_id = subscription_id
            .as_ref()
            .parse()
            .map_err(resource_not_found_err!(Plan, subscription_id))?;
        InsertPlan::licensed_plan(
            conn_lock!(context),
            &context.licensor,
            licensed_plan_id,
            &query_organization,
            level,
            entitlements,
            self_hosted,
        )?;
        QueryPlan::belonging_to(&query_organization)
            .first::<QueryPlan>(conn_lock!(context))
            .map_err(resource_not_found_err!(Plan, query_organization))?
            .to_licensed_plan(biller, &context.licensor, query_organization.uuid).await?
            .ok_or_else(|| {
                issue_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to find licensed plan after creating it",
                &format!("Failed to find licensed plan for organization ({query_organization:?}) after creating it even though plan exists."),
                "Failed to find licensed plan after creating it"
                )
            })
    } else {
        let metered_plan_id = subscription_id
            .as_ref()
            .parse()
            .map_err(resource_not_found_err!(Plan, subscription_id))?;
        InsertPlan::metered_plan(conn_lock!(context), metered_plan_id, &query_organization)?;
        QueryPlan::belonging_to(&query_organization)
            .first::<QueryPlan>(conn_lock!(context))
            .map_err(resource_not_found_err!(Plan, query_organization))?
            .to_metered_plan(biller).await?
            .ok_or_else(|| {
                issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to find metered plan after creating it",
            &format!("Failed to find metered plan for organization ({query_organization:?}) after creating it even though plan exists."),
          "Failed to find metered plan after creating it"
            )})
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct OrgPlanQuery {
    pub remote: Option<bool>,
}

#[endpoint {
    method = DELETE,
    path =  "/v0/organizations/{organization}/plan",
    tags = ["organizations", "plan"]
}]
pub async fn org_plan_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgPlanParams>,
    query_params: Query<OrgPlanQuery>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(
        rqctx.context(),
        path_params.into_inner(),
        query_params.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| {
        #[cfg(feature = "sentry")]
        sentry::capture_error(&e);
        e
    })?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: OrgPlanParams,
    query_params: OrgPlanQuery,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    let biller = context.biller()?;

    // Get the organization
    let query_organization =
        QueryOrganization::from_resource_id(conn_lock!(context), &path_params.organization)?;
    // Check to see if user has permission to manage the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
        .map_err(forbidden_error)?;
    // Get the plan for the organization
    let query_plan = QueryPlan::belonging_to(&query_organization)
        .first::<QueryPlan>(conn_lock!(context))
        .map_err(resource_not_found_err!(Plan, query_organization))?;

    let remote = query_params.remote.unwrap_or(true);
    if let Some(metered_plan_id) = query_plan.metered_plan.as_ref() {
        if remote {
            biller
                .cancel_metered_subscription(metered_plan_id)
                .await
                .map_err(resource_not_found_err!(Plan, query_plan))?;
        }
    } else if let Some(licensed_plan_id) = query_plan.licensed_plan.as_ref() {
        if remote {
            biller
                .cancel_licensed_subscription(licensed_plan_id)
                .await
                .map_err(resource_not_found_err!(Plan, query_plan))?;
        }

        if query_organization.license.is_some() {
            let organization_query = schema::organization::table
                .filter(schema::organization::id.eq(query_organization.id));
            let update_organization = UpdateOrganization {
                name: None,
                slug: None,
                license: Some(None),
                modified: DateTime::now(),
            };
            diesel::update(organization_query)
                .set(&update_organization)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(Organization, update_organization))?;
        }
    } else {
        return Err(issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to find subscription for plan deletion",
            &format!(
                "Failed to find plan (metered or licensed) for organization ({query_organization:?}) even though plan exists ({query_plan:?})."
                 ),
            "Failed to find subscription for plan deletion"
            ));
    }

    diesel::delete(schema::plan::table.filter(schema::plan::id.eq(query_plan.id)))
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Plan, query_plan))?;

    Ok(())
}
