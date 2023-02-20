#![cfg(feature = "plus")]

use std::str::FromStr;

use bencher_billing::SubscriptionId;
use bencher_json::{
    organization::metered::{JsonNewPlan, JsonPlan, DEFAULT_PRICE_NAME},
    JsonEmpty, JsonUser, ResourceId,
};
use bencher_rbac::organization::Permission;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    context::Context,
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::organization::QueryOrganization,
    model::user::{auth::AuthUser, QueryUser},
    schema,
    util::cors::{get_cors, CorsResponse},
    ApiError,
};

use super::Resource;

const PLAN_RESOURCE: Resource = Resource::Plan;

#[derive(Deserialize, JsonSchema)]
pub struct DirPath {
    pub organization: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/plan",
    tags = ["organizations", "plan"]
}]
pub async fn dir_options(
    _rqctx: RequestContext<Context>,
    _path_params: Path<DirPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path =  "/v0/organizations/{organization}/plan",
    tags = ["organizations", "plan"]
}]
pub async fn post(
    rqctx: RequestContext<Context>,
    path_params: Path<DirPath>,
    body: TypedBody<JsonNewPlan>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(PLAN_RESOURCE, Method::Post);

    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &Context,
    path_params: DirPath,
    json_plan: JsonNewPlan,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    let api_context = &mut *context.lock().await;
    let conn = &mut api_context.database.connection;

    // Check to see if there is a Biller
    // The Biller is only available on Bencher Cloud
    let Some(biller) = &api_context.biller else {
        return Err(ApiError::BencherCloudOnly(
            format!("POST /v0/organizations/{}/plan", path_params.organization),
        ));
    };

    // Get the organization
    let query_org = QueryOrganization::from_resource_id(conn, &path_params.organization)?;
    // Check to see if user has permission to manage the organization
    api_context
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
        .map_err(api_error!())?
        .into_json()?;

    // Create a customer for the user
    let customer = biller
        .get_or_create_customer(&json_user.name, &json_user.email)
        .await?;

    // Create a payment method for the user
    let payment_method = biller
        .create_payment_method(&customer, json_plan.card)
        .await?;

    // Create a metered subscription for the organization
    let subscription = biller
        .create_metered_subscription(
            Uuid::from_str(&query_org.uuid).map_err(api_error!())?,
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
        .map_err(api_error!())?;

    Ok(JsonEmpty::default())
}

#[derive(Deserialize, JsonSchema)]
pub struct OnePath {
    pub organization: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/plan",
    tags = ["organizations", "plan"]
}]
pub async fn one_options(
    _rqctx: RequestContext<Context>,
    _path_params: Path<OnePath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/plan",
    tags = ["organizations", "plan"]
}]
pub async fn get_one(
    rqctx: RequestContext<Context>,
    path_params: Path<OnePath>,
) -> Result<ResponseOk<JsonPlan>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(PLAN_RESOURCE, Method::GetOne);

    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(
    context: &Context,
    path_params: OnePath,
    auth_user: &AuthUser,
) -> Result<JsonPlan, ApiError> {
    let api_context = &mut *context.lock().await;
    let conn = &mut api_context.database.connection;

    // Check to see if there is a Biller
    // The Biller is only available on Bencher Cloud
    let Some(biller) = &api_context.biller else {
        return Err(ApiError::BencherCloudOnly(
            format!("GET /v0/organizations/{}/plan", path_params.organization),
        ));
    };

    // Get the organization
    let query_org = QueryOrganization::from_resource_id(conn, &path_params.organization)?;
    // Check to see if user has permission to manage the organization
    api_context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_org)?;

    if let Some(subscription) = &query_org.subscription {
        let subscription_id = subscription.parse()?;
        biller
            .get_subscription(&subscription_id)
            .await
            // .map_err(Into::into)
            ;
        todo!()
    } else {
        Err(ApiError::NoMeteredPlan(query_org.id))
    }
}
