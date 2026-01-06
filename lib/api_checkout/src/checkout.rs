#![cfg(feature = "plus")]

use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseCreated};
use bencher_json::{
    organization::plan::DEFAULT_PRICE_NAME,
    system::payment::{JsonCheckout, JsonNewCheckout},
};
use bencher_rbac::organization::Permission;
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::{forbidden_error, issue_error},
    model::{
        organization::QueryOrganization,
        user::auth::{AuthUser, BearerToken},
    },
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};

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
        .inspect_err(|e| {
            #[cfg(feature = "sentry")]
            sentry::capture_error(&e);
            #[cfg(not(feature = "sentry"))]
            let _ = e;
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

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserCheckout);

    // Get the organization
    let query_organization =
        QueryOrganization::from_resource_id(conn_lock!(context), &organization)?;
    // Check to see if user has permission to manage the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
        .map_err(forbidden_error)?;
    let customer = auth_user.to_customer();

    // TODO Dynamic pricing
    let price_name = DEFAULT_PRICE_NAME;
    let return_url = context
        .console_url
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
        .unwrap_or_else(|_| context.console_url.clone());

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
                "Failed to create checkout session",
                &format!("Failed to create checkout session for {customer:?} at {level:?} using {price_name} with {entitlements:?}."),
                e,
            )
        })
}
