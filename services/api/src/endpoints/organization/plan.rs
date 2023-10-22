#![cfg(feature = "plus")]

use bencher_billing::{Biller, Customer, PaymentMethod};
use bencher_json::{
    organization::plan::{JsonLicense, JsonNewPlan, JsonPlan, DEFAULT_PRICE_NAME},
    DateTime, LicensedPlanId, MeteredPlanId, ResourceId,
};
use bencher_license::Licensor;
use bencher_rbac::organization::Permission;
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use http::StatusCode;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, Post, ResponseAccepted, ResponseOk},
        Endpoint,
    },
    error::{
        bad_request_error, forbidden_error, issue_error, locked_error, payment_required_error,
        resource_conflict_err, resource_conflict_error, resource_not_found_err, BencherResource,
    },
    model::{
        organization::plan::{InsertPlan, QueryPlan},
        user::{auth::AuthUser, QueryUser},
    },
    model::{organization::QueryOrganization, user::auth::BearerToken},
    schema,
};

// Metrics are bundled by the thousand for licensed plans
pub const ENTITLEMENTS_QUANTITY: u64 = 1_000;

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
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
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
    // Check to see if there is a Biller
    // The Biller is only available on Bencher Cloud
    let Some(biller) = &context.biller else {
        return Err(locked_error(format!(
            "Tried to use a Bencher Cloud route when Self-Hosted: GET /v0/organizations/{org}/plan",
            org = path_params.organization
        )));
    };
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

    if let Some(metered_plan_id) = query_plan.metered_plan.clone() {
        biller
            .get_plan(metered_plan_id.clone())
            .await
            .map_err(resource_not_found_err!(Plan, query_plan))
    } else if let Some(licensed_plan_id) = query_plan.licensed_plan.clone() {
        let mut json_plan = biller
            .get_plan(licensed_plan_id.clone())
            .await
            .map_err(resource_not_found_err!(Plan, query_plan))?;

        let Some(license) = &query_plan.license else {
            return Err(issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to find license for licensed plan",
                &format!(
                    "Failed to find license for plan ({query_plan:?}) even though licensed plan exists ({licensed_plan_id:?}).",
                ),
                "Failed to find license for licensed plan",
            ));
        };

        let token_data = context
            .licensor
            .validate_organization(license, query_organization.uuid)
            .map_err(payment_required_error)?;

        let json_license = JsonLicense {
            key: license.clone(),
            organization: query_organization.uuid,
            entitlements: token_data.claims.entitlements(),
            issued_at: token_data.claims.issued_at(),
            expiration: token_data.claims.expiration(),
        };
        json_plan.license = Some(json_license);

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
    // Check to see if there is a Biller
    // The Biller is only available on Bencher Cloud
    let Some(biller) = &context.biller else {
        return Err(locked_error(format!("Tried to use a Bencher Cloud route when Self-Hosted: POST /v0/organizations/{org}/plan", org =path_params.organization)));
    };
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

    let json_user = schema::user::table
        .filter(schema::user::id.eq(auth_user.id))
        .first::<QueryUser>(conn)
        .map_err(resource_not_found_err!(User, auth_user))?
        .into_json();

    // Create a customer for the user
    let customer = biller
        .get_or_create_customer(&json_user.name, &json_user.email, json_user.uuid)
        .await
        .map_err(resource_not_found_err!(Plan, json_user))?;

    // Create a payment method for the user
    let payment_method = biller
        .create_payment_method(&customer, json_plan.card.clone())
        .await
        .map_err(resource_not_found_err!(Plan, customer))?;

    let (plan, insert_plan) = create_plan(
        biller,
        &context.licensor,
        json_plan,
        &query_organization,
        &customer,
        &payment_method,
    )
    .await?;

    diesel::insert_into(schema::plan::table)
        .values(&insert_plan)
        .execute(conn)
        .map_err(resource_conflict_err!(Plan, insert_plan))?;

    Ok(plan)
}

async fn create_plan(
    biller: &Biller,
    licensor: &Licensor,
    json_plan: JsonNewPlan,
    query_organization: &QueryOrganization,
    customer: &Customer,
    payment_method: &PaymentMethod,
) -> Result<(JsonPlan, InsertPlan), HttpError> {
    Ok(if let Some(entitlements) = json_plan.entitlements {
        let entitlements = u64::from(entitlements);
        if entitlements == 0 || entitlements % ENTITLEMENTS_QUANTITY != 0 {
            return Err(bad_request_error(format!(
                "Entitlements ({entitlements}) must be a multiple of 1000",
            )));
        }

        // Create a licensed subscription for the organization
        let subscription = biller
            .create_licensed_subscription(
                query_organization.uuid,
                customer,
                payment_method,
                json_plan.level,
                DEFAULT_PRICE_NAME.into(),
                entitlements,
            )
            .await
            .map_err(resource_conflict_err!(
                Plan,
                (&query_organization, customer, json_plan.level, entitlements)
            ))?;

        let licensed_plan_id: LicensedPlanId = subscription
            .id
            .as_ref()
            .parse()
            .map_err(resource_not_found_err!(Plan, subscription))?;
        // TODO if the org uuid isn't given then add it to the the given org as its license
        let organization_uuid = json_plan.organization.unwrap_or(query_organization.uuid);
        let license = licensor
            .new_annual_license(organization_uuid, entitlements)
            .map_err(|e| issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create license",
                &format!("Failed to create license for organization ({query_organization:?}) with entitlements ({entitlements})."),
                e,
            ))?;
        let timestamp = DateTime::now();

        let plan = biller
            .get_plan(licensed_plan_id.clone())
            .await
            .map_err(resource_not_found_err!(Plan, subscription))?;

        (
            plan,
            InsertPlan {
                organization_id: query_organization.id,
                metered_plan: None,
                licensed_plan: Some(licensed_plan_id),
                license: Some(license),
                created: timestamp,
                modified: timestamp,
            },
        )
    } else {
        // Create a metered subscription for the organization
        let subscription = biller
            .create_metered_subscription(
                query_organization.uuid,
                customer,
                payment_method,
                json_plan.level,
                DEFAULT_PRICE_NAME.into(),
            )
            .await
            .map_err(resource_conflict_err!(
                Plan,
                (&query_organization, customer, json_plan.level)
            ))?;

        let metered_plan_id: MeteredPlanId = subscription
            .id
            .as_ref()
            .parse()
            .map_err(resource_not_found_err!(Plan, subscription))?;
        let timestamp = DateTime::now();

        let plan = biller
            .get_plan(metered_plan_id.clone())
            .await
            .map_err(resource_not_found_err!(Plan, subscription))?;

        (
            plan,
            InsertPlan {
                organization_id: query_organization.id,
                metered_plan: Some(metered_plan_id),
                licensed_plan: None,
                license: None,
                created: timestamp,
                modified: timestamp,
            },
        )
    })
}
