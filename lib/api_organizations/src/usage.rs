#![cfg(feature = "plus")]

use std::time::Duration;

use bencher_billing::Biller;
use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{
    DateTime, OrganizationResourceId,
    organization::usage::{JsonUsage, UsageKind},
};
use bencher_rbac::organization::Permission;
use bencher_schema::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{forbidden_error, issue_error, resource_not_found_err},
    model::{
        organization::{
            OrganizationId, QueryOrganization,
            license::{
                ACTIVE_SERIES_WINDOW, OrganizationLicense, QueryLicensePeak, count_active_series,
            },
            plan::QueryPlan,
        },
        project::{metric::QueryMetric, series},
        runner::job::QueryJob,
        user::auth::{AuthUser, BearerToken},
    },
};
use diesel::{BelongingToDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;
use slog::Logger;

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
/// View the active Benchmark Series, or the metrics on the Free plan,
/// and the bare metal runner minutes usage of an organization.
/// The user must have `manage` permissions for the organization.
/// ➕ Bencher Plus: This endpoint offers an estimate of metered usage on Bencher Cloud.
/// On Bencher Self-Hosted, it offers the active Benchmark Series of the last 30 days
/// against a valid license, and the highest count seen in its term and in the term before it.
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
    let json = get_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_inner(
    log: &Logger,
    context: &ApiContext,
    path_params: OrgUsageParams,
    auth_user: &AuthUser,
) -> Result<JsonUsage, HttpError> {
    // Get the organization
    let query_organization =
        QueryOrganization::from_resource_id(auth_conn!(context), &path_params.organization)?;
    // Check to see if user has permission to manage a project within the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::Manage, &query_organization)
        .map_err(forbidden_error)?;

    let now = context.clock.now();
    // Bencher Cloud
    if let Ok(biller) = context.biller() {
        cloud_plan_usage(log, context, biller, &query_organization).await
    // Self-Hosted Licensed
    } else if let Some(license) =
        OrganizationLicense::get_valid(&context.licensor, &query_organization, now)
    {
        licensed_usage(auth_conn!(context), &query_organization, license, now)
    // Self-Hosted Free, which a license that is not valid reads as
    } else {
        auth_conn!(context, |conn| {
            let usage = free_plan_usage(conn, &query_organization, UsageKind::SelfHostedFree, now)?;
            Ok(JsonUsage {
                previous_peak: QueryLicensePeak::previous(conn, query_organization.id, None)?,
                ..usage
            })
        })
    }
}

/// Usage for a Self-Hosted organization with a valid license: its Benchmark Series active in the
/// last 30 days against the license's, and the highest count seen in the license's term and in
/// the term before it.
fn licensed_usage(
    conn: &mut DbConnection,
    query_organization: &QueryOrganization,
    license: OrganizationLicense,
    now: DateTime,
) -> Result<JsonUsage, HttpError> {
    let start_time = now - ACTIVE_SERIES_WINDOW;
    let end_time = now;
    let active_series = count_active_series(conn, query_organization.id, now)?;
    let term_end = license.claims.term_end();
    let peak = QueryLicensePeak::for_term(conn, query_organization.id, term_end)?;
    let previous_peak = QueryLicensePeak::previous(conn, query_organization.id, Some(term_end))?;
    let runner_minutes =
        QueryJob::runner_minutes_usage(conn, query_organization.id, start_time, end_time)?;
    Ok(JsonUsage {
        organization: query_organization.uuid,
        kind: UsageKind::SelfHostedLicensed,
        plan: None,
        license: Some(license.into_json()),
        start_time,
        end_time,
        metrics: None,
        active_series: Some(active_series),
        peak,
        previous_peak,
        runner_minutes: Some(runner_minutes),
    })
}

/// Usage for a Bencher Cloud organization: a metered plan, a licensed plan managed via
/// Bencher Cloud, or Cloud Free when the organization has no plan.
async fn cloud_plan_usage(
    log: &Logger,
    context: &ApiContext,
    biller: &Biller,
    query_organization: &QueryOrganization,
) -> Result<JsonUsage, HttpError> {
    let Ok(query_plan) = QueryPlan::belonging_to(query_organization)
        .first::<QueryPlan>(auth_conn!(context))
        .map_err(resource_not_found_err!(Plan, query_organization))
    // Cloud Free
    else {
        return free_plan_usage(
            auth_conn!(context),
            query_organization,
            UsageKind::CloudFree,
            context.clock.now(),
        );
    };

    // Metered plan
    if let Some(json_plan) = query_plan.to_metered_plan(biller).await? {
        let start_time = json_plan.current_period_start;
        let end_time = json_plan.current_period_end;
        let MeteredUsage {
            active_series,
            runner_minutes,
        } = metered_plan_usage(
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
            metrics: None,
            active_series: Some(active_series),
            peak: None,
            previous_peak: None,
            runner_minutes: Some(runner_minutes),
        })
    // Licensed plan
    } else if let Some(json_plan) = query_plan.to_licensed_plan(log, context, biller).await? {
        // Bencher Cloud counts nothing for a Self-Hosted server, so the usage runs from the
        // license's issue, or the start of the period when the read left the license out, to the
        // server's time, by which the console tells its grace.
        let license = json_plan.license.clone();
        let start_time = license
            .as_ref()
            .map_or(json_plan.current_period_start, |license| license.issued_at);
        let end_time = context.clock.now();
        Ok(JsonUsage {
            organization: query_organization.uuid,
            kind: UsageKind::CloudSelfHostedLicensed,
            plan: Some(json_plan),
            license,
            start_time,
            end_time,
            metrics: None,
            active_series: None,
            peak: None,
            previous_peak: None,
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
    now: DateTime,
) -> Result<JsonUsage, HttpError> {
    let end_time = now;
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
        active_series: None,
        peak: None,
        previous_peak: None,
        runner_minutes: Some(runner_minutes),
    })
}

/// The billable usage figures for a metered plan: its monthly-active series and its bare
/// metal `runner_minutes`.
struct MeteredUsage {
    active_series: u32,
    runner_minutes: u32,
}

/// Billable usage for a metered (Bencher Cloud) plan.
///
/// Every metered plan bills on monthly-active series (all visibilities). Bare metal
/// runner minutes are metered regardless of visibility.
fn metered_plan_usage(
    conn: &mut DbConnection,
    organization_id: OrganizationId,
    start_time: DateTime,
    end_time: DateTime,
) -> Result<MeteredUsage, HttpError> {
    let active_series = series::count_active(conn, organization_id, start_time, end_time)?;
    let runner_minutes =
        QueryJob::runner_minutes_usage(conn, organization_id, start_time, end_time)?;
    Ok(MeteredUsage {
        active_series,
        runner_minutes,
    })
}
