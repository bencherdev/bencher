#![cfg(feature = "plus")]

use bencher_json::{
    organization::plan::{JsonNewPlan, JsonPlan, DEFAULT_PRICE_NAME},
    JsonUser, ResourceId,
};
use bencher_rbac::organization::Permission;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{response_accepted, response_ok, CorsResponse, ResponseAccepted, ResponseOk},
        Endpoint,
    },
    model::organization::QueryOrganization,
    model::user::{auth::AuthUser, QueryUser},
    schema, ApiError,
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
    Ok(Endpoint::cors(&[Endpoint::Post, Endpoint::GetOne]))
}

#[endpoint {
    method = POST,
    path =  "/v0/organizations/{organization}/plan",
    tags = ["organizations", "plan"]
}]
pub async fn org_plan_post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgPlanParams>,
    body: TypedBody<JsonNewPlan>,
) -> Result<ResponseAccepted<JsonPlan>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::Post;

    let json = post_inner(
        rqctx.context(),
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

async fn post_inner(
    context: &ApiContext,
    path_params: OrgPlanParams,
    json_plan: JsonNewPlan,
    auth_user: &AuthUser,
) -> Result<JsonPlan, ApiError> {
    // Check to see if there is a Biller
    // The Biller is only available on Bencher Cloud
    let Some(biller) = &context.biller else {
        return Err(ApiError::BencherCloudOnly(format!(
            "POST /v0/organizations/{}/plan",
            path_params.organization
        )));
    };
    let conn = &mut *context.conn().await;

    // Get the organization
    let query_org = QueryOrganization::from_resource_id(conn, &path_params.organization)?;
    // Check to see if user has permission to manage the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_org)?;

    // Check to make sure the organization does not already have a metered or licensed plan
    if let Some(subscription) = query_org.subscription {
        return Err(ApiError::PlanMetered(query_org.id, subscription));
    } else if let Some(license) = query_org.license {
        return Err(ApiError::PlanLicensed(query_org.id, license));
    }

    let json_user: JsonUser = schema::user::table
        .filter(schema::user::id.eq(auth_user.id))
        .first::<QueryUser>(conn)
        .map_err(ApiError::from)?
        .into_json()?;

    // Create a customer for the user
    let customer = biller
        .get_or_create_customer(&json_user.name, &json_user.email, json_user.uuid.into())
        .await?;

    // Create a payment method for the user
    let payment_method = biller
        .create_payment_method(&customer, json_plan.card)
        .await?;

    // Create a metered subscription for the organization
    let subscription = biller
        .create_metered_subscription(
            query_org.uuid.into(),
            &customer,
            &payment_method,
            json_plan.level,
            DEFAULT_PRICE_NAME.into(),
        )
        .await?;

    // Add the metered subscription to the organization
    diesel::update(schema::organization::table.filter(schema::organization::id.eq(query_org.id)))
        .set(schema::organization::subscription.eq(subscription.id.as_ref()))
        .execute(conn)
        .map_err(ApiError::from)?;

    biller.get_plan(&subscription.id).await.map_err(Into::into)
}

#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/plan",
    tags = ["organizations", "plan"]
}]
pub async fn org_plan_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgPlanParams>,
) -> Result<ResponseOk<JsonPlan>, HttpError> {
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
    path_params: OrgPlanParams,
    auth_user: &AuthUser,
) -> Result<JsonPlan, ApiError> {
    // Check to see if there is a Biller
    // The Biller is only available on Bencher Cloud
    let Some(biller) = &context.biller else {
        return Err(ApiError::BencherCloudOnly(format!(
            "GET /v0/organizations/{}/plan",
            path_params.organization
        )));
    };
    let conn = &mut *context.conn().await;

    // Get the organization
    let query_org = QueryOrganization::from_resource_id(conn, &path_params.organization)?;
    // Check to see if user has permission to manage the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_org)?;

    if let Some(subscription) = &query_org.subscription {
        let subscription_id = subscription.parse()?;
        biller.get_plan(&subscription_id).await.map_err(Into::into)
    } else {
        Err(ApiError::NoMeteredPlan(query_org.id))
    }
}
