#![cfg(feature = "plus")]

use std::time::Duration;

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{
    DateTime, OrganizationResourceId,
    organization::usage::{JsonUsage, UsageKind},
};
use bencher_rbac::organization::Permission;
use bencher_schema::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{forbidden_error, issue_error, payment_required_error, resource_not_found_err},
    model::{
        organization::{OrganizationId, QueryOrganization, plan::QueryPlan},
        project::metric::QueryMetric,
        runner::job::QueryJob,
        user::auth::{AuthUser, BearerToken},
    },
};
use diesel::{BelongingToDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

// 30 days
const DEFAULT_USAGE_HISTORY: Duration = Duration::from_hours(720);

#[derive(Deserialize, JsonSchema)]
pub struct OrgUsageParams {
    /// The slug or UUID for an organization.
    pub organization: OrganizationResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/usage",
    tags = ["organizations", "usage"]
}]
pub async fn org_usage_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgUsageParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View organization usage
///
/// View the metrics and bare metal runner minutes usage of an organization.
/// The user must have `manage` permissions for the organization.
/// ➕ Bencher Plus: This endpoint offers an estimate of metered usage
/// and exact usage for licensed organizations, both on Bencher Cloud and Bencher Self-Hosted.
#[endpoint {
    method = GET,
    path = "/v0/organizations/{organization}/usage",
    tags = ["organizations", "usage"]
}]
pub async fn org_usage_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgUsageParams>,
) -> Result<ResponseOk<JsonUsage>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_inner(
    context: &ApiContext,
    path_params: OrgUsageParams,
    auth_user: &AuthUser,
) -> Result<JsonUsage, HttpError> {
    let licensor = &context.licensor;

    // Get the organization
    let query_organization =
        QueryOrganization::from_resource_id(auth_conn!(context), &path_params.organization)?;
    // Check to see if user has permission to manage a project within the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
        .map_err(forbidden_error)?;

    // Bencher Cloud
    if let Ok(biller) = context.biller() {
        let Ok(query_plan) = QueryPlan::belonging_to(&query_organization)
            .first::<QueryPlan>(auth_conn!(context))
            .map_err(resource_not_found_err!(Plan, query_organization))
        // Cloud Free
        else {
            return free_plan_usage(
                auth_conn!(context),
                &query_organization,
                UsageKind::CloudFree,
            );
        };

        // Metered plan
        if let Some(json_plan) = query_plan.to_metered_plan(biller).await? {
            let start_time = json_plan.current_period_start;
            let end_time = json_plan.current_period_end;
            let (metrics, runner_minutes) = metered_plan_usage(
                auth_conn!(context),
                query_organization.id,
                start_time,
                end_time,
            )?;
            Ok(JsonUsage {
                organization: query_organization.uuid,
                kind: UsageKind::CloudMetered,
                plan: Some(json_plan),
                license: None,
                start_time,
                end_time,
                metrics: Some(metrics),
                runner_minutes: Some(runner_minutes),
            })
        // Licensed plan
        } else if let Some(json_plan) = query_plan.to_licensed_plan(biller, licensor).await? {
            let Some(json_license) = json_plan.license.clone() else {
                return Err(issue_error(
                    "No license JSON found for licensed plan",
                    &format!(
                        "Failed to find license for licensed plan ({query_plan:?}) as JSON ({json_plan:?})",
                    ),
                    "License JSON not found",
                ));
            };
            let start_time = json_license.issued_at;
            let end_time = json_license.expiration;
            Ok(JsonUsage {
                organization: query_organization.uuid,
                kind: UsageKind::CloudSelfHostedLicensed,
                plan: Some(json_plan),
                license: Some(json_license),
                start_time,
                end_time,
                metrics: None,
                runner_minutes: None,
            })
        } else {
            Err(issue_error(
                "Failed to find subscription for plan usage",
                &format!(
                    "Failed to find plan (metered or licensed) for organization ({query_organization:?}) even though plan exists ({query_plan:?})."
                ),
                "Failed to find subscription for plan usage",
            ))
        }
    // Self-Hosted Licensed
    } else if let Some(license) = query_organization.license.clone() {
        let json_license = licensor
            .into_json(license)
            .map_err(payment_required_error)?;
        let start_time = json_license.issued_at;
        let end_time = json_license.expiration;
        let (metrics, runner_minutes) = query_usage(
            auth_conn!(context),
            query_organization.id,
            start_time,
            end_time,
        )?;
        Ok(JsonUsage {
            organization: query_organization.uuid,
            kind: UsageKind::SelfHostedLicensed,
            plan: None,
            license: Some(json_license),
            start_time,
            end_time,
            metrics: Some(metrics),
            runner_minutes: Some(runner_minutes),
        })
    // Self-Hosted Free
    } else {
        free_plan_usage(
            auth_conn!(context),
            &query_organization,
            UsageKind::SelfHostedFree,
        )
    }
}

fn query_usage(
    conn: &mut DbConnection,
    organization_id: OrganizationId,
    start_time: DateTime,
    end_time: DateTime,
) -> Result<(u32, u32), HttpError> {
    let metrics = QueryMetric::usage(conn, organization_id, start_time, end_time)?;
    let runner_minutes =
        QueryJob::runner_minutes_usage(conn, organization_id, start_time, end_time)?;
    Ok((metrics, runner_minutes))
}

fn free_plan_usage(
    conn: &mut DbConnection,
    query_organization: &QueryOrganization,
    kind: UsageKind,
) -> Result<JsonUsage, HttpError> {
    let end_time = DateTime::now();
    let start_time = (end_time.into_inner() - DEFAULT_USAGE_HISTORY).into();
    let (metrics, runner_minutes) = query_usage(conn, query_organization.id, start_time, end_time)?;
    Ok(JsonUsage {
        organization: query_organization.uuid,
        kind,
        plan: None,
        license: None,
        start_time,
        end_time,
        metrics: Some(metrics),
        runner_minutes: Some(runner_minutes),
    })
}

/// Estimate billable usage for a metered (Bencher Cloud) plan.
///
/// Only Private Project metrics are metered; Public Project metrics are free and
/// unlimited (see `PlanKind::check_usage`). Bare metal runner minutes are metered
/// regardless of visibility.
fn metered_plan_usage(
    conn: &mut DbConnection,
    organization_id: OrganizationId,
    start_time: DateTime,
    end_time: DateTime,
) -> Result<(u32, u32), HttpError> {
    let metrics = QueryMetric::private_usage(conn, organization_id, start_time, end_time)?;
    let runner_minutes =
        QueryJob::runner_minutes_usage(conn, organization_id, start_time, end_time)?;
    Ok((metrics, runner_minutes))
}
