#![cfg(feature = "plus")]

use bencher_json::{
    organization::plan::DEFAULT_PRICE_NAME,
    system::payment::{JsonCheckout, JsonNewCheckout, JsonNewPayment, JsonPayment},
};
use bencher_rbac::organization::Permission;
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use http::StatusCode;

use crate::{
    conn,
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Post, ResponseCreated},
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
    sentry::capture_message("Checkout endpoint activated", sentry::Level::Info);
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
        self_hosted,
    } = json_checkout;

    // Get the organization
    let query_organization = QueryOrganization::from_resource_id(conn!(context), &organization)?;
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
            "/console/organizations/{organization}/checkout?checkout={{CHECKOUT_SESSION_ID}}&level={level}{license}{self_hosted}",
            organization = query_organization.slug,
            level = level.as_ref(),
            license = entitlements
                .map(|entitlements| format!("&entitlements={entitlements}"))
                .unwrap_or_default(),
            self_hosted = self_hosted
                .map(|uuid| format!("&self_hosted={uuid}"))
                .unwrap_or_default(),
        ))
        .unwrap_or_else(|_| context.endpoint.clone());

    biller
        .new_checkout_session(
            query_organization.uuid,
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
