#![cfg(feature = "plus")]

use bencher_billing::Biller;
use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Patch, Post, ResponseCreated, ResponseDeleted, ResponseOk,
};
use bencher_json::{
    OrganizationResourceId, PlanStatus,
    organization::plan::{JsonNewPlan, JsonPlan, JsonUpdatePlan},
};
use bencher_rbac::organization::Permission;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{
        BencherResource, bad_request_error, forbidden_error, issue_error, resource_conflict_err,
        resource_conflict_error, resource_not_found_err, service_unavailable_error,
    },
    model::{
        organization::{
            QueryOrganization, UpdateOrganization,
            plan::{InsertPlan, QueryPlan},
        },
        user::{
            admin::AdminUser,
            auth::{AuthUser, BearerToken},
        },
    },
    schema, write_conn,
};
use diesel::{
    BelongingToDsl as _, ExpressionMethods as _, OptionalExtension as _, QueryDsl as _,
    RunQueryDsl as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct OrgPlanParams {
    /// The slug or UUID for an organization.
    pub organization: OrganizationResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/plan",
    tags = ["organizations", "plan"]
}]
pub async fn org_plan_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgPlanParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[
        Get.into(),
        Post.into(),
        Patch.into(),
        Delete.into(),
    ]))
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
        QueryOrganization::from_resource_id(auth_conn!(context), &path_params.organization)?;
    // Check to see if user has permission to manage the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
        .map_err(forbidden_error)?;
    // Get the plan for the organization
    let query_plan = QueryPlan::belonging_to(&query_organization)
        .first::<QueryPlan>(auth_conn!(context))
        .map_err(resource_not_found_err!(Plan, query_organization))?;

    if let Some(json_plan) = query_plan.to_metered_plan(biller).await? {
        Ok(json_plan)
    } else if let Some(json_plan) = query_plan
        .to_licensed_plan(biller, &context.licensor)
        .await?
    {
        Ok(json_plan)
    } else {
        Err(issue_error(
            "Failed to find subscription for plan",
            &format!(
                "Failed to find plan (metered or licensed) for organization ({query_organization:?}) even though plan exists ({query_plan:?})."
            ),
            "Failed to find subscription for plan",
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
    .inspect_err(|e| {
        #[cfg(feature = "sentry")]
        sentry::capture_error(e);
        #[cfg(not(feature = "sentry"))]
        let _ = e;
    })?;
    Ok(Post::auth_response_created(json))
}

/// Update an organization's metered subscription plan.
/// Schedules or clears a cancel-at-period-end for the organization's metered
/// subscription. Available to organization managers and must be called with a
/// user session JWT, not a user API key.
#[endpoint {
    method = PATCH,
    path =  "/v0/organizations/{organization}/plan",
    tags = ["organizations", "plan"]
}]
pub async fn org_plan_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgPlanParams>,
    body: TypedBody<JsonUpdatePlan>,
) -> Result<ResponseOk<JsonPlan>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    // A user API key cannot update a subscription plan; require a user session JWT
    // (mirrors the user-key management endpoints).
    if auth_user.user_key_id.is_some() {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OrganizationPlanUpdateBlocked);
        return Err(forbidden_error(
            "A user API key cannot update a subscription plan. Authenticate with a JWT (user session token) instead.",
        ));
    }
    let json = patch_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Patch::auth_response_ok(json))
}

/// Update the organization's metered subscription: schedule or clear a
/// cancel-at-period-end. Returns the refreshed plan.
async fn patch_inner(
    context: &ApiContext,
    path_params: OrgPlanParams,
    json_plan: JsonUpdatePlan,
    auth_user: &AuthUser,
) -> Result<JsonPlan, HttpError> {
    let biller = context.biller()?;

    // Get the organization
    let query_organization =
        QueryOrganization::from_resource_id(auth_conn!(context), &path_params.organization)?;
    // Check to see if user has permission to manage the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
        .map_err(forbidden_error)?;
    // Get the plan for the organization
    let query_plan = QueryPlan::belonging_to(&query_organization)
        .first::<QueryPlan>(auth_conn!(context))
        .map_err(resource_not_found_err!(Plan, query_organization))?;

    // The cancel-at-period-end schedule only applies to a metered subscription;
    // licensed plans are canceled immediately and have no period-end schedule to
    // change.
    let Some(metered_plan_id) = query_plan.metered_plan.as_ref() else {
        return Err(bad_request_error(
            "Only a metered plan's cancellation can be updated; this organization has no metered subscription",
        ));
    };
    biller
        .set_metered_cancel_at_period_end(metered_plan_id, json_plan.cancel_at_period_end)
        .await
        .map_err(resource_conflict_err!(Plan, query_plan))?;

    query_plan.to_metered_plan(biller).await?.ok_or_else(|| {
        issue_error(
            "Failed to find subscription for updated plan",
            &format!(
                "Failed to find metered plan for organization ({query_organization:?}) after update even though plan exists ({query_plan:?})."
            ),
            "Failed to find subscription for updated plan",
        )
    })
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
        QueryOrganization::from_resource_id(auth_conn!(context), &path_params.organization)?;
    // Check to see if user has permission to manage the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
        .map_err(forbidden_error)?;
    // Block creating a plan when the organization still has an active plan; prune a
    // stale lapsed metered plan row first so the organization can subscribe again.
    prune_or_conflict_existing_plan(context, biller, &query_organization).await?;

    let JsonNewPlan {
        checkout,
        level,
        entitlements,
        self_hosted,
        remote,
    } = json_plan;

    if context.is_bencher_cloud && entitlements.is_some() && self_hosted.is_none() {
        return Err(bad_request_error(
            "Licensed plans are only available for Bencher Self-Hosted",
        ));
    }

    let subscription_id = if remote.unwrap_or(true) {
        biller
            .get_checkout_session(checkout.as_ref())
            .await
            .map_err(|e| {
                issue_error(
                    "Failed to get checkout session",
                    &format!("Failed to get checkout session {checkout}."),
                    e,
                )
            })?
    } else {
        checkout.as_ref().parse().map_err(|e| {
            issue_error(
                "Failed to parse subscription ID",
                &format!("Failed to parse subscription ID {checkout}."),
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
            write_conn!(context),
            &context.licensor,
            licensed_plan_id,
            &query_organization,
            level,
            entitlements,
            self_hosted,
        )?;
        QueryPlan::belonging_to(&query_organization)
            .first::<QueryPlan>(auth_conn!(context))
            .map_err(resource_not_found_err!(Plan, query_organization))?
            .to_licensed_plan(biller, &context.licensor).await?
            .ok_or_else(|| {
                issue_error(
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
        InsertPlan::metered_plan(write_conn!(context), metered_plan_id, &query_organization)?;
        QueryPlan::belonging_to(&query_organization)
            .first::<QueryPlan>(auth_conn!(context))
            .map_err(resource_not_found_err!(Plan, query_organization))?
            .to_metered_plan(biller).await?
            .ok_or_else(|| {
                issue_error(
                "Failed to find metered plan after creating it",
            &format!("Failed to find metered plan for organization ({query_organization:?}) after creating it even though plan exists."),
          "Failed to find metered plan after creating it"
            )})
    }
}

/// Block plan creation when the organization still has a plan that is active, trialing,
/// or recoverable (a dunning `past_due`/`unpaid`/`incomplete`/`paused` subscription still
/// exists in Stripe). Only a *terminal* metered subscription (canceled or
/// incomplete-expired), or one Stripe reports as gone, leaves a stale local plan row safe
/// to prune so the organization can subscribe again now that the daily reconciliation
/// sweep is gone; pruning a still-live subscription would orphan it and let the org
/// create a duplicate (double billing). Licensed (Self-Hosted) plan rows do not lapse on
/// their own, so they always block.
async fn prune_or_conflict_existing_plan(
    context: &ApiContext,
    biller: &Biller,
    query_organization: &QueryOrganization,
) -> Result<(), HttpError> {
    let Ok(query_plan) =
        QueryPlan::belonging_to(query_organization).first::<QueryPlan>(auth_conn!(context))
    else {
        return Ok(());
    };

    // A licensed (Self-Hosted) plan carries no metered subscription and always blocks.
    let Some(metered_plan_id) = &query_plan.metered_plan else {
        return Err(plan_conflict(query_organization, query_plan));
    };

    // Resolve the live metered subscription status (`None` => Stripe reports it gone). A
    // transient Stripe error surfaces as 503 rather than being mistaken for "gone", so we
    // never prune a subscription that might still be live.
    let status = biller
        .metered_plan_status(metered_plan_id)
        .await
        .map_err(service_unavailable_error)?;

    if subscription_is_live(status) {
        // Still live in Stripe (active, trialing, or dunning): block, so we never orphan a
        // live subscription and let the org create a duplicate.
        Err(plan_conflict(query_organization, query_plan))
    } else {
        // Gone or terminal: prune the stale row so the org can subscribe again.
        diesel::delete(schema::plan::table.filter(schema::plan::id.eq(query_plan.id)))
            .execute(write_conn!(context))
            .map_err(resource_conflict_err!(Plan, query_plan))?;
        Ok(())
    }
}

/// The conflict error returned when an organization already has a plan that blocks
/// creating a new one.
fn plan_conflict(query_organization: &QueryOrganization, query_plan: QueryPlan) -> HttpError {
    resource_conflict_error(
        BencherResource::Plan,
        (query_organization.clone(), query_plan),
        "Organization already has a plan",
    )
}

/// Whether a subscription is still live in Stripe, given its status (`None` when Stripe
/// reports it gone via a 404).
///
/// Live means active, trialing, or in a recoverable dunning state (`past_due` / `unpaid` /
/// `incomplete` / `paused`): the subscription still exists in Stripe. Not live means gone
/// (404) or *terminal* (canceled / incomplete-expired). Matched exhaustively so a new
/// `PlanStatus` forces a decision here.
///
/// This answers the single question behind two separate decisions: plan creation blocks on
/// a live subscription (otherwise it prunes the stale row so the org can re-subscribe), and
/// plan deletion cancels a live subscription (otherwise it skips, since canceling a
/// gone/terminal one would 404).
fn subscription_is_live(status: Option<PlanStatus>) -> bool {
    match status {
        None | Some(PlanStatus::Canceled | PlanStatus::IncompleteExpired) => false,
        Some(
            PlanStatus::Active
            | PlanStatus::Trialing
            | PlanStatus::PastDue
            | PlanStatus::Unpaid
            | PlanStatus::Incomplete
            | PlanStatus::Paused,
        ) => true,
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct OrgPlanQuery {
    pub remote: Option<bool>,
}

/// Delete an organization's subscription plan (admin only).
/// Immediately cancels the organization's subscription and removes the plan.
/// Restricted to Bencher server administrators.
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
    // Admin only: an immediate hard-cancel + plan removal, usable cross-org.
    AdminUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(
        rqctx.context(),
        path_params.into_inner(),
        query_params.into_inner(),
    )
    .await
    .inspect_err(|e| {
        #[cfg(feature = "sentry")]
        sentry::capture_error(&e);
        #[cfg(not(feature = "sentry"))]
        let _ = e;
    })?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: OrgPlanParams,
    query_params: OrgPlanQuery,
) -> Result<(), HttpError> {
    let biller = context.biller()?;

    // Get the organization
    let query_organization =
        QueryOrganization::from_resource_id(auth_conn!(context), &path_params.organization)?;
    // Get the plan for the organization
    let query_plan = QueryPlan::belonging_to(&query_organization)
        .first::<QueryPlan>(auth_conn!(context))
        .map_err(resource_not_found_err!(Plan, query_organization))?;

    let remote = query_params.remote.unwrap_or(true);
    // Wait to return the result of the biller delete until after the plan has been deleted locally
    let delete_plan_result = delete_plan(context, biller, &query_organization, &query_plan, remote)
        .await
        .inspect_err(|e| {
            #[cfg(feature = "sentry")]
            sentry::capture_error(&e);
            #[cfg(not(feature = "sentry"))]
            let _ = e;
        });

    // The metered subscription is canceled immediately (see
    // `cancel_metered_subscription`); remove the local plan row now.
    diesel::delete(schema::plan::table.filter(schema::plan::id.eq(query_plan.id)))
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(Plan, query_plan))?;

    delete_plan_result
}

/// Cancel an organization's live Stripe subscription (if any) and remove its local plan
/// row. Called from organization deletion so no dangling subscription is left in Stripe
/// when an organization is deleted. A no-op when the server has no biller (Self-Hosted) or
/// the organization has no plan row.
///
/// The Stripe cancel cannot share a database transaction with the caller's organization
/// delete, so there is a small non-atomic window. Cancel-first ordering keeps it on the safe
/// side: if the org delete then fails, the org is briefly present with its subscription
/// already canceled (recoverable on retry) rather than deleted with a live subscription.
pub(crate) async fn cancel_and_remove_plan(
    context: &ApiContext,
    query_organization: &QueryOrganization,
) -> Result<(), HttpError> {
    // Only Bencher Cloud has a biller and plan rows; nothing to cancel otherwise.
    let Ok(biller) = context.biller() else {
        return Ok(());
    };
    // A missing plan row means nothing to cancel, but a real database error must not be
    // mistaken for "no plan" (that would delete the org while its subscription is still
    // live). `.optional()` maps only `NotFound` to `None` and propagates every other error.
    let Some(query_plan) = QueryPlan::belonging_to(query_organization)
        .first::<QueryPlan>(auth_conn!(context))
        .optional()
        .map_err(resource_conflict_err!(Plan, query_organization))?
    else {
        return Ok(());
    };

    // Cancel in Stripe first; propagate the error (aborting the organization deletion) if it
    // fails so we never delete the organization while its subscription is still live.
    delete_plan(context, biller, query_organization, &query_plan, true).await?;

    // The subscription is canceled (or was already gone); remove the local plan row.
    diesel::delete(schema::plan::table.filter(schema::plan::id.eq(query_plan.id)))
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(Plan, query_plan))?;

    Ok(())
}

async fn delete_plan(
    context: &ApiContext,
    biller: &Biller,
    query_organization: &QueryOrganization,
    query_plan: &QueryPlan,
    remote: bool,
) -> Result<(), HttpError> {
    if let Some(metered_plan_id) = query_plan.metered_plan.as_ref() {
        if remote {
            // Only cancel a subscription still live in Stripe; a gone/terminal one is
            // already effectively canceled, so canceling it again would 404. A transient
            // Stripe error surfaces as 503 rather than being mistaken for "gone".
            let status = biller
                .metered_plan_status(metered_plan_id)
                .await
                .map_err(service_unavailable_error)?;
            if subscription_is_live(status) {
                biller
                    .cancel_metered_subscription(metered_plan_id)
                    .await
                    .map_err(resource_not_found_err!(Plan, query_plan))?;
            }
        }
    } else if let Some(licensed_plan_id) = query_plan.licensed_plan.as_ref() {
        if remote {
            // Only cancel a subscription still live in Stripe; a gone/terminal one is
            // already effectively canceled, so canceling it again would 404. A transient
            // Stripe error surfaces as 503 rather than being mistaken for "gone".
            let status = biller
                .licensed_plan_status(licensed_plan_id)
                .await
                .map_err(service_unavailable_error)?;
            if subscription_is_live(status) {
                biller
                    .cancel_licensed_subscription(licensed_plan_id)
                    .await
                    .map_err(resource_not_found_err!(Plan, query_plan))?;
            }
        }

        if query_organization.license.is_some() {
            let organization_query = schema::organization::table
                .filter(schema::organization::id.eq(query_organization.id));
            let update_organization = UpdateOrganization {
                name: None,
                slug: None,
                license: Some(None),
                modified: context.clock.now(),
            };
            diesel::update(organization_query)
                .set(&update_organization)
                .execute(write_conn!(context))
                .map_err(resource_conflict_err!(Organization, update_organization))?;
        }
    } else {
        return Err(issue_error(
            "Failed to find subscription for plan deletion",
            &format!(
                "Failed to find plan (metered or licensed) for organization ({query_organization:?}) even though plan exists ({query_plan:?})."
            ),
            "Failed to find subscription for plan deletion",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use bencher_json::PlanStatus;

    use super::subscription_is_live;

    #[test]
    fn subscription_is_live_decides() {
        // Gone in Stripe (404) or terminal: not live, so plan creation prunes the stale row
        // and plan deletion skips canceling.
        assert!(!subscription_is_live(None));
        assert!(!subscription_is_live(Some(PlanStatus::Canceled)));
        assert!(!subscription_is_live(Some(PlanStatus::IncompleteExpired)));
        // Active, trialing, or recoverable (dunning) still exists in Stripe: live, so plan
        // creation blocks (never orphan a live subscription and let the org create a
        // duplicate) and plan deletion cancels.
        for status in [
            PlanStatus::Active,
            PlanStatus::Trialing,
            PlanStatus::PastDue,
            PlanStatus::Unpaid,
            PlanStatus::Incomplete,
            PlanStatus::Paused,
        ] {
            assert!(subscription_is_live(Some(status)));
        }
    }
}
