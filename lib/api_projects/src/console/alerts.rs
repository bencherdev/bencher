use bencher_endpoint::{CorsResponse, Endpoint, Patch, ResponseOk};
use bencher_json::{
    DateTime, ProjectResourceId,
    project::alert::{
        AlertStatus, JsonAlertsFilter, JsonUpdateAlerts, JsonUpdatedAlerts, MAX_UPDATE_ALERTS,
    },
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{bad_request_error, resource_conflict_err, with_auth_hint},
    model::{
        project::{ProjectId, QueryProject, threshold::alert::UpdateAlert},
        user::actor::{ApiActor, PubProjectBearerToken},
    },
    schema, write_conn,
};
use diesel::{
    BoolExpressionMethods as _, BoxableExpression, ExpressionMethods as _, JoinOnDsl as _,
    QueryDsl as _, RunQueryDsl as _, dsl::exists, sql_types::Bool, sqlite::Sqlite,
};
use dropshot::{HttpError, Path, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::alerts::archived_filter;

#[derive(Deserialize, JsonSchema)]
pub struct ProjConsoleAlertsParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/console/alerts",
    tags = ["projects", "alerts"],
    unpublished = true,
}]
pub async fn proj_console_alerts_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjConsoleAlertsParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Patch.into()]))
}

/// Update many alerts
///
/// Dismiss or reactivate the alerts of a project, selected by a list of alerts or by a filter.
/// The user must have `edit` permissions for the project,
/// or provide a valid project key for the project.
/// Silenced alerts never change.
#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/console/alerts",
    tags = ["projects", "alerts"],
    unpublished = true,
}]
pub async fn proj_console_alerts_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjConsoleAlertsParams>,
    body: TypedBody<JsonUpdateAlerts>,
) -> Result<ResponseOk<JsonUpdatedAlerts>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = patch_inner(
        rqctx.context(),
        &api_actor,
        path_params.into_inner(),
        body.into_inner(),
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Patch::auth_response_ok(json))
}

async fn patch_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjConsoleAlertsParams,
    json_alerts: JsonUpdateAlerts,
) -> Result<JsonUpdatedAlerts, HttpError> {
    let query_project = QueryProject::is_allowed_actor_auth(
        auth_conn!(context),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        api_actor,
        Permission::Edit,
    )?;

    let JsonUpdateAlerts {
        status,
        alerts,
        filter,
    } = json_alerts;
    let selection: Selection = match (alerts, filter) {
        (Some(alerts), None) => {
            check_len("alerts", alerts.len())?;
            Box::new(schema::alert::uuid.eq_any(alerts))
        },
        (None, Some(filter)) => matching(query_project.id, filter)?,
        (Some(_), Some(_)) | (None, None) => {
            return Err(bad_request_error(
                "Select the alerts by either `alerts` or `filter`",
            ));
        },
    };

    let update_alert = UpdateAlert::status_change(Some(status), context.clock.now());
    let changed = diesel::update(
        schema::alert::table
            .filter(schema::alert::project_id.eq(query_project.id))
            .filter(schema::alert::status.ne(AlertStatus::Silenced))
            .filter(schema::alert::status.ne(AlertStatus::from(status)))
            .filter(selection),
    )
    .set(&update_alert)
    .execute(write_conn!(context))
    .map_err(resource_conflict_err!(Alert, (&query_project, status)))?;

    Ok(JsonUpdatedAlerts {
        changed: u32::try_from(changed).unwrap_or(u32::MAX),
    })
}

type Selection = Box<dyn BoxableExpression<schema::alert::table, Sqlite, SqlType = Bool>>;

/// The alerts of the project that the filter matches, leaving out those on an archived
/// branch, testbed, or measure as the alert list does.
///
/// Dismiss all holds the write lock, so the alert rows are read by their own columns and only
/// a window reaches through to the reports that raised them.
fn matching(project_id: ProjectId, filter: JsonAlertsFilter) -> Result<Selection, HttpError> {
    let JsonAlertsFilter {
        status,
        branches,
        testbeds,
        measures,
        thresholds,
        start_time,
        end_time,
    } = filter;
    check_len("branches", branches.len())?;
    check_len("testbeds", testbeds.len())?;
    check_len("measures", measures.len())?;
    check_len("thresholds", thresholds.len())?;

    let mut threshold_ids = schema::threshold::table
        .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::threshold::branch_id)))
        .inner_join(
            schema::testbed::table.on(schema::testbed::id.eq(schema::threshold::testbed_id)),
        )
        .inner_join(
            schema::measure::table.on(schema::measure::id.eq(schema::threshold::measure_id)),
        )
        .filter(schema::threshold::project_id.eq(project_id))
        .filter(archived_filter(None))
        .select(schema::threshold::id)
        .into_boxed();
    if !branches.is_empty() {
        threshold_ids = threshold_ids.filter(schema::branch::uuid.eq_any(branches));
    }
    if !testbeds.is_empty() {
        threshold_ids = threshold_ids.filter(schema::testbed::uuid.eq_any(testbeds));
    }
    if !measures.is_empty() {
        threshold_ids = threshold_ids.filter(schema::measure::uuid.eq_any(measures));
    }
    if !thresholds.is_empty() {
        threshold_ids = threshold_ids.filter(schema::threshold::uuid.eq_any(thresholds));
    }
    let mut selection: Selection = Box::new(schema::alert::threshold_id.eq_any(threshold_ids));

    if let Some(status) = status {
        selection = Box::new(selection.and(schema::alert::status.eq(status)));
    }

    let alert_report = schema::boundary::table
        .inner_join(schema::metric::table.on(schema::metric::id.eq(schema::boundary::metric_id)))
        .inner_join(
            schema::report_benchmark::table
                .on(schema::report_benchmark::id.eq(schema::metric::report_benchmark_id)),
        )
        .inner_join(
            schema::report::table.on(schema::report::id.eq(schema::report_benchmark::report_id)),
        )
        .filter(schema::boundary::id.eq(schema::alert::boundary_id))
        .select(schema::boundary::id);
    let since = start_time.map(|start_time| schema::report::created.ge(DateTime::from(start_time)));
    let until = end_time.map(|end_time| schema::report::created.le(DateTime::from(end_time)));
    match (since, until) {
        (Some(since), Some(until)) => {
            selection = Box::new(selection.and(exists(alert_report.filter(since).filter(until))));
        },
        (Some(since), None) => {
            selection = Box::new(selection.and(exists(alert_report.filter(since))));
        },
        (None, Some(until)) => {
            selection = Box::new(selection.and(exists(alert_report.filter(until))));
        },
        (None, None) => {},
    }

    Ok(selection)
}

fn check_len(field: &str, len: usize) -> Result<(), HttpError> {
    if len > MAX_UPDATE_ALERTS {
        Err(bad_request_error(format!(
            "`{field}` lists {len} entries, more than the {MAX_UPDATE_ALERTS} allowed"
        )))
    } else {
        Ok(())
    }
}
