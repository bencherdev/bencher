#![cfg(feature = "plus")]

use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseCreated};
use bencher_json::{
    Entitlements, PlanLevel,
    organization::plan::DEFAULT_PRICE_NAME,
    system::payment::{JsonCheckout, JsonNewCheckout},
};
use bencher_rbac::organization::Permission;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{bad_request_error, forbidden_error, issue_error, with_token_hint},
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
        })
        .map_err(with_token_hint)?;
    Ok(Post::auth_response_created(json))
}

async fn checkouts_post_inner(
    context: &ApiContext,
    json_checkout: JsonNewCheckout,
    auth_user: &AuthUser,
) -> Result<JsonCheckout, HttpError> {
    let JsonNewCheckout {
        organization,
        level,
        entitlements,
        self_hosted,
    } = json_checkout;

    if entitlements.is_some() && self_hosted.is_none() {
        return Err(bad_request_error(
            "Licensed plans are only available for Bencher Self-Hosted",
        ));
    }
    let checkout_plan = match (level, entitlements) {
        (PlanLevel::Free, _) => {
            return Err(bad_request_error("The Free plan has no checkout"));
        },
        (PlanLevel::Pro, _) => CheckoutPlan::Pro,
        (PlanLevel::Enterprise, Some(entitlements)) => CheckoutPlan::Licensed(entitlements),
        (PlanLevel::Enterprise, None) => {
            return Err(bad_request_error(
                "Enterprise checkout is only available for a Bencher Self-Hosted license",
            ));
        },
    };
    let biller = context.biller()?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserCheckout);

    // Get the organization
    let query_organization =
        QueryOrganization::from_resource_id(auth_conn!(context), &organization)?;
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

    match checkout_plan {
        CheckoutPlan::Pro => {
            biller
                .new_metered_checkout_session(
                    query_organization.uuid,
                    &customer,
                    price_name.to_owned(),
                    return_url.as_ref(),
                )
                .await
        },
        CheckoutPlan::Licensed(entitlements) => {
            biller
                .new_licensed_checkout_session(
                    query_organization.uuid,
                    &customer,
                    price_name.to_owned(),
                    entitlements,
                    return_url.as_ref(),
                )
                .await
        },
    }
    .map_err(|e| {
            issue_error(
                "Failed to create checkout session",
                &format!("Failed to create checkout session for {customer:?} at {level:?} using {price_name} with {entitlements:?}."),
                e,
            )
        })
}

/// A plan that checkout sells.
enum CheckoutPlan {
    Pro,
    Licensed(Entitlements),
}
