#![cfg(feature = "plus")]

use bencher_json::{
    organization::plan::{JsonNewPlan, JsonPlan, DEFAULT_PRICE_NAME},
    DateTime, ResourceId,
};
use bencher_rbac::organization::Permission;
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use http::StatusCode;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{
            CorsResponse, Delete, Get, Post, ResponseAccepted, ResponseDeleted, ResponseOk,
        },
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
    pub organization: ResourceId,
}

#[allow(clippy::unused_async)]
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
    let conn = &mut *context.conn().await;

    // Get the organization
    let query_organization = QueryOrganization::from_resource_id(conn, &path_params.organization)?;
    // Check to see if user has permission to manage the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
        .map_err(forbidden_error)?;
    // Get the plan for the organization
    let query_plan = QueryPlan::belonging_to(&query_organization)
        .first::<QueryPlan>(conn)
        .map_err(resource_not_found_err!(Plan, query_organization))?;

    if let Some(json_plan) = query_plan.metered_plan(biller).await? {
        Ok(json_plan)
    } else if let Some(json_plan) = query_plan
        .licensed_plan(biller, &context.licensor, query_organization.uuid)
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
) -> Result<ResponseAccepted<JsonPlan>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Post::auth_response_accepted(json))
}

async fn post_inner(
    context: &ApiContext,
    path_params: OrgPlanParams,
    json_plan: JsonNewPlan,
    auth_user: &AuthUser,
) -> Result<JsonPlan, HttpError> {
    let biller = context.biller()?;
    if !json_plan.i_agree {
        return Err(forbidden_error(
            "You must agree to the Bencher Subscription Agreement (https://bencher.dev/legal/subscription)",
        ));
    }
    let conn = &mut *context.conn().await;

    // Get the organization
    let query_organization = QueryOrganization::from_resource_id(conn, &path_params.organization)?;
    // Check to see if user has permission to manage the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
        .map_err(forbidden_error)?;
    // Check to make sure the organization doesn't already have a plan
    if let Ok(query_plan) = QueryPlan::belonging_to(&query_organization).first::<QueryPlan>(conn) {
        return Err(resource_conflict_error(
            BencherResource::Plan,
            (query_organization, query_plan),
            "Organization already has a plan",
        ));
    }

    // Create a customer for the user
    let customer = biller
        .get_or_create_customer(&json_plan.customer)
        .await
        .map_err(resource_not_found_err!(Plan, &json_plan.customer))?;

    // Create a payment method for the user
    let payment_method = biller
        .create_payment_method(&customer, json_plan.card.clone())
        .await
        .map_err(resource_not_found_err!(Plan, customer))?;

    if let Some(entitlements) = json_plan.entitlements {
        InsertPlan::licensed_plan(
            conn,
            biller,
            &context.licensor,
            &query_organization,
            &customer,
            &payment_method,
            json_plan.level,
            DEFAULT_PRICE_NAME.into(),
            entitlements,
            json_plan.organization,
        )
        .await?;
        let query_plan = QueryPlan::belonging_to(&query_organization)
            .first::<QueryPlan>(conn)
            .map_err(resource_not_found_err!(Plan, query_organization))?;
        if let Some(query_plan) = query_plan
            .licensed_plan(biller, &context.licensor, query_organization.uuid)
            .await?
        {
            Ok(query_plan)
        } else {
            Err(issue_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to find licensed plan after creating it",
                &format!(
                    "Failed to find licensed plan for organization ({query_organization:?}) after creating it even though plan exists ({query_plan:?})."
                     ),
                "Failed to find licensed plan after creating it"
                ))
        }
    } else {
        InsertPlan::metered_plan(
            conn,
            biller,
            &query_organization,
            &customer,
            &payment_method,
            json_plan.level,
            DEFAULT_PRICE_NAME.into(),
        )
        .await?;
        let query_plan = QueryPlan::belonging_to(&query_organization)
            .first::<QueryPlan>(conn)
            .map_err(resource_not_found_err!(Plan, query_organization))?;
        if let Some(query_plan) = query_plan.metered_plan(biller).await? {
            Ok(query_plan)
        } else {
            Err(issue_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to find metered plan after creating it",
                &format!(
                    "Failed to find metered plan for organization ({query_organization:?}) after creating it even though plan exists ({query_plan:?})."
                     ),
                "Failed to find metered plan after creating it"
                ))
        }
    }
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
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: OrgPlanParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    let biller = context.biller()?;
    let conn = &mut *context.conn().await;

    // Get the organization
    let query_organization = QueryOrganization::from_resource_id(conn, &path_params.organization)?;
    // Check to see if user has permission to manage the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
        .map_err(forbidden_error)?;
    // Get the plan for the organization
    let query_plan = QueryPlan::belonging_to(&query_organization)
        .first::<QueryPlan>(conn)
        .map_err(resource_not_found_err!(Plan, query_organization))?;

    if let Some(metered_plan_id) = query_plan.metered_plan.as_ref() {
        biller
            .cancel_metered_subscription(metered_plan_id.clone())
            .await
            .map_err(resource_not_found_err!(Plan, query_plan))?;
    } else if let Some(licensed_plan_id) = query_plan.licensed_plan.as_ref() {
        biller
            .cancel_licensed_subscription(licensed_plan_id.clone())
            .await
            .map_err(resource_not_found_err!(Plan, query_plan))?;

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
                .execute(conn)
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
        .execute(conn)
        .map_err(resource_conflict_err!(Plan, query_plan))?;

    Ok(())
}
