#![cfg(feature = "plus")]

use bencher_json::system::payment::{JsonNewPayment, JsonPayment};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use http::StatusCode;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Post, ResponseCreated},
        Endpoint,
    },
    error::{issue_error, resource_not_found_err},
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
    body: TypedBody<JsonNewPayment>,
) -> Result<ResponseCreated<JsonPayment>, HttpError> {
    let json = post_inner(rqctx.context(), body.into_inner())
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
) -> Result<JsonPayment, HttpError> {
    let biller = context.biller()?;

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
