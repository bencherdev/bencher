#![cfg(feature = "plus")]

use bencher_json::{
    organization::plan::DEFAULT_PRICE_NAME,
    system::payment::{JsonCheckout, JsonNewCheckout, JsonNewPayment, JsonPayment},
    JsonPlan, NonEmpty,
};
use bencher_rbac::organization::Permission;
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use http::StatusCode;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, Post, ResponseCreated, ResponseOk},
        Endpoint,
    },
    error::{forbidden_error, issue_error, resource_not_found_err},
    model::{
        organization::QueryOrganization,
        user::{
            auth::{AuthUser, BearerToken},
            same_user,
        },
    },
};

use super::auth::PLAN_ARG;

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/payments",
    tags = ["payments"]
}]
pub async fn payments_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

#[endpoint {
    method = POST,
    path =  "/v0/payments",
    tags = ["payments"]
}]
pub async fn payments_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    body: TypedBody<JsonNewPayment>,
) -> Result<ResponseCreated<JsonPayment>, HttpError> {
    sentry::capture_message("Payments endpoint activated", sentry::Level::Info);
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(rqctx.context(), body.into_inner(), &auth_user)
        .await
        .map_err(|e| {
            #[cfg(feature = "sentry")]
            sentry::capture_error(&e);
            e
        })?;
    Ok(Post::pub_response_created(json))
}

async fn post_inner(
    context: &ApiContext,
    json_payment: JsonNewPayment,
    auth_user: &AuthUser,
) -> Result<JsonPayment, HttpError> {
    let biller = context.biller()?;

    same_user!(auth_user, context.rbac, json_payment.customer.uuid);

    // Create a customer for the user
    let customer_id = biller
        .get_or_create_customer(&json_payment.customer)
        .await
        .map_err(resource_not_found_err!(Plan, json_payment.customer))?;

    // Create a payment method for the user
    let payment_method_id = biller
        .create_payment_method(customer_id.clone(), json_payment.card)
        .await
        .map_err(resource_not_found_err!(Plan, &customer_id))?;

    Ok(JsonPayment {
        customer: customer_id.as_ref().parse().map_err(|e| {
            issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to parse customer ID",
                &format!("Failed to parse customer ID ({customer_id})."),
                e,
            )
        })?,
        payment_method: payment_method_id.as_ref().parse().map_err(|e| {
            issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to parse payment method ID",
                &format!("Failed to parse payment method ID ({payment_method_id})."),
                e,
            )
        })?,
    })
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/checkout",
    tags = ["checkout"]
}]
pub async fn checkouts_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

#[endpoint {
    method = POST,
    path =  "/v0/checkout",
    tags = ["checkout"]
}]
pub async fn checkouts_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    body: TypedBody<JsonNewCheckout>,
) -> Result<ResponseCreated<JsonCheckout>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = checkouts_post_inner(rqctx.context(), body.into_inner(), &auth_user)
        .await
        .map_err(|e| {
            #[cfg(feature = "sentry")]
            sentry::capture_error(&e);
            e
        })?;
    Ok(Post::auth_response_created(json))
}

async fn checkouts_post_inner(
    context: &ApiContext,
    json_checkout: JsonNewCheckout,
    auth_user: &AuthUser,
) -> Result<JsonCheckout, HttpError> {
    let biller = context.biller()?;
    let JsonNewCheckout {
        organization,
        level,
        entitlements,
        self_hosted_organization,
    } = json_checkout;
    let conn = &mut *context.conn().await;

    // Get the organization
    let query_organization = QueryOrganization::from_resource_id(conn, &organization)?;
    // Check to see if user has permission to manage the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
        .map_err(forbidden_error)?;
    let customer = auth_user.to_customer();

    // TODO Dynamic pricing
    let price_name = DEFAULT_PRICE_NAME;
    let return_url = context
        .endpoint
        .clone()
        .join(&format!(
            "/console/organizations/{organization}/checkout?session_id={{CHECKOUT_SESSION_ID}}&{PLAN_ARG}={level}{license}{self_hosted}",
            organization = query_organization.slug,
            license = entitlements
                .map(|entitlements| format!("&license={entitlements}"))
                .unwrap_or_default(),
            self_hosted = self_hosted_organization
                .map(|uuid| format!("&self_hosted={uuid}"))
                .unwrap_or_default(),
        ))
        .unwrap_or_else(|_| context.endpoint.clone());
    biller
        .new_checkout_session(
            &customer,
            level,
            price_name.to_owned(),
            entitlements,
            return_url.as_ref(),
        )
        .await.map_err(|e| {
            issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create checkout session",
                &format!("Failed to create checkout session for {customer:?} at {level:?} using {price_name} with {entitlements:?}."),
                e,
            )
        })
}

#[derive(Deserialize, JsonSchema)]
pub struct CheckoutParams {
    pub session: NonEmpty,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/checkout/{session}",
    tags = ["checkout"]
}]
pub async fn checkout_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/checkout/{session}",
    tags = ["checkout"]
}]
pub async fn checkout_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<CheckoutParams>,
    body: TypedBody<JsonNewCheckout>,
) -> Result<ResponseOk<JsonPlan>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: CheckoutParams,
    json_checkout: JsonNewCheckout,
    auth_user: &AuthUser,
) -> Result<JsonPlan, HttpError> {
    let biller = context.biller()?;
    let JsonNewCheckout {
        organization,
        level,
        entitlements,
        self_hosted_organization,
    } = json_checkout;
    let conn = &mut *context.conn().await;

    // Get the organization
    let query_organization = QueryOrganization::from_resource_id(conn, &organization)?;
    // Check to see if user has permission to manage the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
        .map_err(forbidden_error)?;
    let customer = auth_user.to_customer();

    let CheckoutParams { session } = path_params;
    let plan = biller
        .get_checkout_session_plan(session.as_ref())
        .await
        .map_err(|e| {
            issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get checkout session",
                &format!("Failed to get checkout session {session}.",),
                e,
            )
        })?;

    // // Get the organization
    // let query_organization = QueryOrganization::from_resource_id(conn, &organization)?;
    // // Check to see if user has permission to manage the organization
    // context
    //     .rbac
    //     .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
    //     .map_err(forbidden_error)?;
    // let customer = auth_user.to_customer();

    todo!()
}
