use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    ops::Range,
    str::FromStr,
    time::Duration,
};

use bencher_endpoint::{CorsResponse, Endpoint, Get, Patch, ResponseOk};
use bencher_json::{
    BranchName, BranchSlug, BranchUuid, DateTime, GitHash, HeadUuid, MeasureUuid, MetricName,
    ProjectResourceId, ReportUuid, ResourceName, TestbedSlug, TestbedUuid, ThresholdUuid,
    project::{
        alert::{
            AlertStatus, JsonAlertsFilter, JsonUpdateAlerts, JsonUpdatedAlerts, MAX_UPDATE_ALERTS,
        },
        console::{
            ConsoleAlertStatus, DEFAULT_CONSOLE_ALERTS_PER_PAGE, JsonConsoleAlertGroup,
            JsonConsoleAlertLine, JsonConsoleAlerts, JsonConsoleAlertsCounts,
            JsonConsoleAlertsQueryParams, JsonConsoleBranch, JsonConsoleTestbed, JsonConsoleWindow,
            MAX_CONSOLE_ALERTS_PER_PAGE, MAX_CONSOLE_HISTORY_REPORTS,
        },
        head::{JsonVersion, VersionNumber},
        report::Iteration,
    },
    urlencoded::from_urlencoded_list,
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    actor_conn, auth_conn,
    context::{ApiContext, DbConnection},
    error::{bad_request_error, resource_conflict_err, resource_not_found_err, with_auth_hint},
    model::{
        project::{
            ProjectId, QueryProject,
            branch::{BranchId, head::HeadId},
            metric::MetricId,
            report::{QueryReport, ReportId},
            testbed::TestbedId,
            threshold::alert::{AlertId, UpdateAlert},
        },
        user::actor::{ApiActor, PubProjectBearerToken},
    },
    schema, write_conn,
};
use diesel::{
    BoolExpressionMethods as _, BoxableExpression, ExpressionMethods as _, JoinOnDsl as _,
    QueryDsl as _, RunQueryDsl as _, SelectableExpression, SelectableHelper as _,
    dsl::{count_star, exists},
    sql_types::Bool,
    sqlite::Sqlite,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{
    AlertRow, BenchmarkRow, Check, Limits, MeasureRow, ModelRow, PointReport, Tables, VariantRow,
    alert_json, before,
    report::{
        BoundaryRow, History, HistoryRow, Line, LineKey, WindowSource, history_points, line_json,
        window_holds, window_length, windows_history_rows,
    },
};
use crate::{alerts::archived_filter, perf::DEFAULT_REPORT_HISTORY};

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
    Ok(Endpoint::cors(&[Get.into(), Patch.into()]))
}

/// List alerts for the console
///
/// A page of the alerts of a project, grouped by the report that raised them, newest report
/// first, each as the report draws the line it raised on, with its history over a window that
/// ends at the report.
/// The user must be signed in and allowed to view the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/console/alerts",
    tags = ["projects", "alerts"],
    unpublished = true,
}]
pub async fn proj_console_alerts_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjConsoleAlertsParams>,
    query_params: Query<JsonConsoleAlertsQueryParams>,
) -> Result<ResponseOk<JsonConsoleAlerts>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_inner(
        rqctx.context(),
        &api_actor,
        path_params.into_inner(),
        query_params.into_inner(),
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::auth_response_ok(json))
}

async fn get_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjConsoleAlertsParams,
    query_params: JsonConsoleAlertsQueryParams,
) -> Result<JsonConsoleAlerts, HttpError> {
    let query = AlertsQuery::new(query_params)?;
    let project = QueryProject::is_allowed_actor_pub(
        actor_conn!(context, api_actor),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        api_actor,
    )?;
    // Only the public plot is served without a login, even for a public project.
    if !api_actor.is_auth() {
        return Err(project
            .auth_state(api_actor)
            .auth_error(&path_params.project, Permission::View));
    }
    let conn = actor_conn!(context, api_actor);

    let counts = counts(conn, project.id, &query)?;
    let total = status_total(query.status, counts);
    if query.per_page == 0 || total == 0 {
        return Ok(alerts_json(total, counts, Page::default(), Vec::new()));
    }
    let keys = page_keys(conn, project.id, &query)?;
    let Some(&(_, first_alert)) = keys.first() else {
        return Ok(alerts_json(total, counts, Page::default(), Vec::new()));
    };
    let report_ids = unique_ids(keys.iter().map(|(report_id, _)| *report_id));
    let rows = group_rows(conn, project.id, &query, &report_ids)?;
    let page = Page::new(rows, &report_ids, first_alert, usize::from(query.per_page));
    let histories = page
        .histories(conn, &query)
        .map_err(resource_not_found_err!(Metric, project.id))?;
    Ok(alerts_json(total, counts, page, histories))
}

/// A request read and checked before anything is read for it.
struct AlertsQuery {
    status: ConsoleAlertStatus,
    /// What the alerts were raised on and when, which Dismiss all selects by too.
    filter: JsonAlertsFilter,
    window: Duration,
    points: usize,
    page: u32,
    per_page: u8,
}

impl AlertsQuery {
    fn new(query_params: JsonConsoleAlertsQueryParams) -> Result<Self, HttpError> {
        let JsonConsoleAlertsQueryParams {
            status,
            branches,
            testbeds,
            measures,
            thresholds,
            reports,
            start_time,
            end_time,
            window,
            points,
            page,
            per_page,
        } = query_params;
        if let (Some(start_time), Some(end_time)) = (start_time, end_time)
            && i64::from(start_time) > i64::from(end_time)
        {
            return Err(bad_request_error(format!(
                "The window starts ({start_time}) after it ends ({end_time})"
            )));
        }
        let per_page = per_page.unwrap_or(DEFAULT_CONSOLE_ALERTS_PER_PAGE);
        if per_page > MAX_CONSOLE_ALERTS_PER_PAGE {
            return Err(bad_request_error(format!(
                "A page is at most {MAX_CONSOLE_ALERTS_PER_PAGE} alerts, not {per_page}"
            )));
        }
        Ok(Self {
            status: status.unwrap_or_default(),
            filter: JsonAlertsFilter {
                status: None,
                branches: list("branches", branches.as_deref())?,
                testbeds: list("testbeds", testbeds.as_deref())?,
                measures: list("measures", measures.as_deref())?,
                thresholds: list("thresholds", thresholds.as_deref())?,
                reports: list("reports", reports.as_deref())?,
                start_time,
                end_time,
            },
            window: window_length(window)?.unwrap_or(DEFAULT_REPORT_HISTORY),
            points: history_points(points)?,
            page: page.unwrap_or(1).max(1),
            per_page,
        })
    }
}

/// An empty list filters nothing, as an empty list does in a Dismiss all filter.
fn list<T: FromStr>(field: &str, list: Option<&str>) -> Result<Vec<T>, HttpError> {
    match list {
        None | Some("") => Ok(Vec::new()),
        Some(list) => from_urlencoded_list(list)
            .map_err(|error| bad_request_error(format!("`{field}`: {error}"))),
    }
}

fn status_total(status: ConsoleAlertStatus, counts: JsonConsoleAlertsCounts) -> u32 {
    let JsonConsoleAlertsCounts {
        active,
        dismissed,
        silenced,
    } = counts;
    match status {
        ConsoleAlertStatus::Active => active,
        ConsoleAlertStatus::Dismissed => dismissed.saturating_add(silenced),
        ConsoleAlertStatus::All => active.saturating_add(dismissed).saturating_add(silenced),
    }
}

/// The alerts that match the filters, by status, read the way Dismiss all selects them.
fn counts(
    conn: &mut DbConnection,
    project_id: ProjectId,
    query: &AlertsQuery,
) -> Result<JsonConsoleAlertsCounts, HttpError> {
    let counts = schema::alert::table
        .filter(schema::alert::project_id.eq(project_id))
        .filter(matching(project_id, query.filter.clone())?)
        .group_by(schema::alert::status)
        .select((schema::alert::status, count_star()))
        .load::<(AlertStatus, i64)>(conn)
        .map_err(resource_not_found_err!(Alert, project_id))?;
    let mut json = JsonConsoleAlertsCounts {
        active: 0,
        dismissed: 0,
        silenced: 0,
    };
    for (status, count) in counts {
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        match status {
            AlertStatus::Active => json.active = count,
            AlertStatus::Dismissed => json.dismissed = count,
            AlertStatus::Silenced => json.silenced = count,
        }
    }
    Ok(json)
}

/// The report and alert at each position of the page, in list order.
fn page_keys(
    conn: &mut DbConnection,
    project_id: ProjectId,
    query: &AlertsQuery,
) -> Result<Vec<(ReportId, AlertId)>, HttpError> {
    let offset = i64::from(query.page - 1).saturating_mul(i64::from(query.per_page));
    schema::alert::table
        .inner_join(schema::boundary::table.on(schema::boundary::id.eq(schema::alert::boundary_id)))
        .inner_join(schema::metric::table.on(schema::metric::id.eq(schema::boundary::metric_id)))
        .inner_join(
            schema::report_benchmark::table
                .on(schema::report_benchmark::id.eq(schema::metric::report_benchmark_id)),
        )
        .inner_join(
            schema::report::table.on(schema::report::id.eq(schema::report_benchmark::report_id)),
        )
        .filter(selected(project_id, query)?)
        // The alert identifier only places a page's edges inside a report; the report's own
        // order is drawn after.
        .order((
            schema::report::created.desc(),
            schema::report::id.desc(),
            schema::alert::id.asc(),
        ))
        .offset(offset)
        .limit(i64::from(query.per_page))
        .select((schema::report::id, schema::alert::id))
        .load::<(ReportId, AlertId)>(conn)
        .map_err(resource_not_found_err!(Alert, project_id))
}

/// One alert that matches, with the report that raised it and the line it raised on.
type GroupRow = (
    (AlertId, DateTime),
    QueryReport,
    (VersionNumber, Option<GitHash>),
    (BranchId, BranchUuid, BranchName, BranchSlug, HeadUuid),
    (TestbedUuid, ResourceName, TestbedSlug),
    Iteration,
    BenchmarkRow,
    VariantRow,
    MeasureRow,
    (MetricId, MetricName, f64),
    (BoundaryRow, ThresholdUuid, ModelRow, AlertRow),
);

/// Every alert that matches in the page's reports, so each report's alerts can be drawn in
/// the order its report page draws them.
fn group_rows(
    conn: &mut DbConnection,
    project_id: ProjectId,
    query: &AlertsQuery,
    report_ids: &[ReportId],
) -> Result<Vec<GroupRow>, HttpError> {
    schema::alert::table
        .inner_join(schema::boundary::table.on(schema::boundary::id.eq(schema::alert::boundary_id)))
        .inner_join(schema::metric::table.on(schema::metric::id.eq(schema::boundary::metric_id)))
        .inner_join(
            schema::report_benchmark::table
                .on(schema::report_benchmark::id.eq(schema::metric::report_benchmark_id)),
        )
        .inner_join(
            schema::report::table.on(schema::report::id.eq(schema::report_benchmark::report_id)),
        )
        .inner_join(schema::version::table.on(schema::version::id.eq(schema::report::version_id)))
        .inner_join(schema::head::table.on(schema::head::id.eq(schema::report::head_id)))
        .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::head::branch_id)))
        .inner_join(schema::testbed::table.on(schema::testbed::id.eq(schema::report::testbed_id)))
        .inner_join(
            schema::benchmark::table
                .on(schema::benchmark::id.eq(schema::report_benchmark::benchmark_id)),
        )
        .inner_join(
            schema::variant::table.on(schema::variant::id.eq(schema::report_benchmark::variant_id)),
        )
        .inner_join(schema::measure::table.on(schema::measure::id.eq(schema::metric::measure_id)))
        .inner_join(
            schema::threshold::table.on(schema::threshold::id.eq(schema::boundary::threshold_id)),
        )
        .inner_join(schema::model::table.on(schema::model::id.eq(schema::boundary::model_id)))
        // The page's reports are the project's, and reading from them spares a walk over the
        // project's alerts by status.
        .filter(schema::report::id.eq_any(report_ids))
        .filter(raised_alerts_with(project_id, query, ThresholdIndex::Skip)?)
        .select((
            (schema::alert::id, schema::alert::modified),
            QueryReport::as_select(),
            (schema::version::number, schema::version::hash),
            (
                schema::branch::id,
                schema::branch::uuid,
                schema::branch::name,
                schema::branch::slug,
                schema::head::uuid,
            ),
            (
                schema::testbed::uuid,
                schema::testbed::name,
                schema::testbed::slug,
            ),
            schema::report_benchmark::iteration,
            (
                schema::benchmark::id,
                schema::benchmark::uuid,
                schema::benchmark::name,
                schema::benchmark::slug,
            ),
            (
                schema::variant::id,
                schema::variant::uuid,
                schema::variant::parameters,
            ),
            (
                schema::measure::id,
                schema::measure::uuid,
                schema::measure::name,
                schema::measure::slug,
                schema::measure::units,
            ),
            (
                schema::metric::id,
                schema::metric::name,
                schema::metric::value,
            ),
            (
                (
                    schema::boundary::threshold_id,
                    schema::boundary::baseline,
                    schema::boundary::lower_limit,
                    schema::boundary::upper_limit,
                ),
                schema::threshold::uuid,
                (
                    schema::model::uuid,
                    schema::model::test,
                    schema::model::min_sample_size,
                    schema::model::max_sample_size,
                    schema::model::window,
                    schema::model::lower_boundary,
                    schema::model::upper_boundary,
                ),
                (
                    schema::alert::uuid,
                    schema::alert::boundary_limit,
                    schema::alert::status,
                ),
            ),
        ))
        .load::<GroupRow>(conn)
        .map_err(resource_not_found_err!(Alert, project_id))
}

type Selected<QS> = Box<dyn BoxableExpression<QS, Sqlite, SqlType = Bool>>;

/// The alerts the request lists, by the rules of [`matching`], over a query that joins each
/// alert to the report that raised it.
fn selected<QS: 'static>(
    project_id: ProjectId,
    query: &AlertsQuery,
) -> Result<Selected<QS>, HttpError>
where
    schema::alert::project_id: SelectableExpression<QS>,
    schema::alert::threshold_id: SelectableExpression<QS>,
    schema::alert::status: SelectableExpression<QS>,
    schema::report::uuid: SelectableExpression<QS>,
    schema::report::created: SelectableExpression<QS>,
{
    Ok(Box::new(
        schema::alert::project_id
            .eq(project_id)
            .and(raised_alerts::<QS>(project_id, query)?),
    ))
}

/// The alerts the request lists among alerts already known to be the project's.
fn raised_alerts<QS: 'static>(
    project_id: ProjectId,
    query: &AlertsQuery,
) -> Result<Selected<QS>, HttpError>
where
    schema::alert::threshold_id: SelectableExpression<QS>,
    schema::alert::status: SelectableExpression<QS>,
    schema::report::uuid: SelectableExpression<QS>,
    schema::report::created: SelectableExpression<QS>,
{
    raised_alerts_with(project_id, query, ThresholdIndex::Use)
}

fn raised_alerts_with<QS: 'static>(
    project_id: ProjectId,
    query: &AlertsQuery,
    index: ThresholdIndex,
) -> Result<Selected<QS>, HttpError>
where
    schema::alert::threshold_id: SelectableExpression<QS>,
    schema::alert::status: SelectableExpression<QS>,
    schema::report::uuid: SelectableExpression<QS>,
    schema::report::created: SelectableExpression<QS>,
{
    let JsonAlertsFilter {
        status: _,
        branches,
        testbeds,
        measures,
        thresholds,
        reports,
        start_time,
        end_time,
    } = query.filter.clone();
    let mut selected =
        raised_on_with::<QS>(project_id, branches, testbeds, measures, thresholds, index)?;
    match query.status {
        ConsoleAlertStatus::Active => {
            selected = Box::new(selected.and(schema::alert::status.eq(AlertStatus::Active)));
        },
        ConsoleAlertStatus::Dismissed => {
            selected = Box::new(selected.and(schema::alert::status.ne(AlertStatus::Active)));
        },
        ConsoleAlertStatus::All => {},
    }
    if !reports.is_empty() {
        selected = Box::new(selected.and(schema::report::uuid.eq_any(reports)));
    }
    if let Some(start_time) = start_time {
        selected = Box::new(selected.and(schema::report::created.ge(DateTime::from(start_time))));
    }
    if let Some(end_time) = end_time {
        selected = Box::new(selected.and(schema::report::created.le(DateTime::from(end_time))));
    }
    Ok(selected)
}

/// Whether SQLite may reach alerts through their threshold, which a page of reports skips: at
/// 64 reports it would otherwise read every alert of the project.
#[derive(Clone, Copy)]
enum ThresholdIndex {
    Use,
    Skip,
}

/// The alerts on a page, grouped by report in list order, and the lines they raised on.
#[derive(Default)]
struct Page {
    groups: Vec<PageGroup>,
    /// Every alert's line, a group's alerts together in its `alerts` range.
    lines: Vec<Line>,
}

struct PageGroup {
    report: QueryReport,
    version: JsonVersion,
    branch: (BranchId, BranchUuid, BranchName, BranchSlug, HeadUuid),
    testbed: (TestbedUuid, ResourceName, TestbedSlug),
    /// The report's alerts that match, on every page.
    total: usize,
    alerts: Range<usize>,
    modified: Vec<DateTime>,
}

/// A group's history window and its alerts' histories in it.
struct GroupHistory {
    window: JsonConsoleWindow,
    thinned: super::Thinned,
}

struct GroupAlert {
    id: AlertId,
    modified: DateTime,
    line: Line,
}

impl Page {
    /// The page's slice of its reports' alerts, which start where the first key's alert sits
    /// among its report's alerts in key order.
    fn new(
        rows: Vec<GroupRow>,
        report_ids: &[ReportId],
        first_alert: AlertId,
        per_page: usize,
    ) -> Self {
        let mut by_report: HashMap<ReportId, (PageGroup, Vec<GroupAlert>)> = HashMap::new();
        for (
            (id, modified),
            report,
            (number, hash),
            branch,
            testbed,
            iteration,
            benchmark,
            variant,
            measure,
            (_, metric, value),
            (boundary, threshold_uuid, model, alert),
        ) in rows
        {
            let (threshold_id, baseline, lower_limit, upper_limit) = boundary;
            let check = Check {
                threshold_id,
                threshold_uuid,
                model,
                limits: Limits {
                    baseline,
                    lower_limit,
                    upper_limit,
                },
                alert: alert_json(Some(alert)),
            };
            let line = Line::new(
                benchmark,
                variant,
                measure,
                metric,
                iteration,
                value,
                Some(check),
            );
            by_report
                .entry(report.id)
                .or_insert_with(|| {
                    (
                        PageGroup {
                            report,
                            version: JsonVersion { number, hash },
                            branch,
                            testbed,
                            total: 0,
                            alerts: 0..0,
                            modified: Vec::new(),
                        },
                        Vec::new(),
                    )
                })
                .1
                .push(GroupAlert { id, modified, line });
        }

        let mut ordered = report_ids
            .iter()
            .filter_map(|report_id| by_report.remove(report_id))
            .collect::<Vec<_>>();
        for (group, alerts) in &mut ordered {
            group.total = alerts.len();
            alerts.sort_by(|a, b| a.line.cmp_name(&b.line).then_with(|| a.id.cmp(&b.id)));
        }
        let mut skip = ordered.first().map_or(0, |(_, alerts)| {
            alerts.iter().filter(|alert| alert.id < first_alert).count()
        });

        let mut page = Self::default();
        let mut remaining = per_page;
        for (mut group, alerts) in ordered {
            let start = page.lines.len();
            for alert in alerts.into_iter().skip(skip).take(remaining) {
                group.modified.push(alert.modified);
                page.lines.push(alert.line);
            }
            skip = 0;
            remaining -= page.lines.len() - start;
            group.alerts = start..page.lines.len();
            if !group.alerts.is_empty() {
                page.groups.push(group);
            }
        }
        page
    }

    /// Each group's history over the window that ends at its report, read for every group at
    /// once and thinned as a report's rows are.
    fn histories(
        &self,
        conn: &mut DbConnection,
        query: &AlertsQuery,
    ) -> diesel::QueryResult<Vec<GroupHistory>> {
        let starts = self
            .groups
            .iter()
            .map(|group| window_start(&group.report, query.window))
            .collect::<Vec<_>>();
        let reports = window_reports(conn, &self.groups, &starts)?;
        let windows = self
            .groups
            .iter()
            .zip(&starts)
            .map(|(group, start_time)| group_window(&reports, &group.report, *start_time))
            .collect::<Vec<_>>();

        let reads = self
            .groups
            .iter()
            .zip(&windows)
            .map(|(group, (window_reports, _))| {
                (
                    window_reports.as_slice(),
                    self.lines.get(group.alerts.clone()).unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        let rows = windows_history_rows(conn, &reads)?;
        let mut rows_by_line: HashMap<LineKey, Vec<HistoryRow>> = HashMap::new();
        for row in rows {
            rows_by_line
                .entry(LineKey::of_row(&row))
                .or_default()
                .push(row);
        }

        Ok(self
            .groups
            .iter()
            .zip(windows)
            .map(|(group, (window_reports, window))| {
                let lines = self.lines.get(group.alerts.clone()).unwrap_or_default();
                let in_window = window_reports
                    .iter()
                    .map(|report| report.id)
                    .collect::<HashSet<_>>();
                let keys = lines.iter().map(Line::key).collect::<HashSet<_>>();
                let group_rows = keys
                    .iter()
                    .filter_map(|key| rows_by_line.get(key))
                    .flatten()
                    .filter(|(report_id, ..)| in_window.contains(report_id))
                    .cloned()
                    .collect();
                GroupHistory {
                    window,
                    thinned: History::new(&window_reports, lines, group_rows)
                        .thin(group.report.id, query.points),
                }
            })
            .collect())
    }
}

/// When a report's history starts: its window before the report's start.
fn window_start(report: &QueryReport, window: Duration) -> DateTime {
    before(report.start_time, window).unwrap_or(report.start_time)
}

/// A report that a window reaches, under the head it was read for.
struct WindowReport {
    head_id: HeadId,
    testbed_id: TestbedId,
    report: PointReport,
}

/// The reports of every group's window, in one read: each group's as its report page reads
/// them, which stops one report past what a history holds.
fn window_reports(
    conn: &mut DbConnection,
    groups: &[PageGroup],
    starts: &[DateTime],
) -> diesel::QueryResult<Vec<WindowReport>> {
    let mut reaches: Option<Selected<WindowSource>> = None;
    for (group, start_time) in groups.iter().zip(starts) {
        let window = schema::report::table
            .inner_join(
                schema::head_version::table
                    .on(schema::head_version::version_id.eq(schema::report::version_id)),
            )
            .inner_join(
                schema::version::table.on(schema::version::id.eq(schema::report::version_id)),
            )
            .filter(window_holds(&group.report, *start_time))
            .order((schema::report::end_time.desc(), schema::report::id.desc()))
            .limit(i64::try_from(MAX_CONSOLE_HISTORY_REPORTS + 1).unwrap_or(i64::MAX))
            .select(schema::report::id)
            .into_boxed();
        // The window's report ids, not the head, are what SQLite should read the reports by.
        let reach: Selected<WindowSource> = Box::new(
            (schema::head_version::head_id + 0)
                .eq(group.report.head_id)
                .and(schema::report::id.eq_any(window)),
        );
        reaches = Some(match reaches {
            Some(reaches) => Box::new(reaches.or(reach)),
            None => reach,
        });
    }
    let Some(reaches) = reaches else {
        return Ok(Vec::new());
    };
    Ok(schema::report::table
        .inner_join(
            schema::head_version::table
                .on(schema::head_version::version_id.eq(schema::report::version_id)),
        )
        .inner_join(schema::version::table.on(schema::version::id.eq(schema::report::version_id)))
        .filter(reaches)
        .select((
            schema::head_version::head_id,
            schema::report::testbed_id,
            schema::report::id,
            schema::report::uuid,
            schema::report::start_time,
            schema::report::end_time,
            schema::report::created,
            schema::version::number,
            schema::version::hash,
        ))
        .load::<(
            HeadId,
            TestbedId,
            ReportId,
            ReportUuid,
            DateTime,
            DateTime,
            DateTime,
            VersionNumber,
            Option<GitHash>,
        )>(conn)?
        .into_iter()
        .map(
            |(head_id, testbed_id, id, uuid, start_time, end_time, created, version, hash)| {
                WindowReport {
                    head_id,
                    testbed_id,
                    report: PointReport {
                        id,
                        uuid,
                        start_time,
                        end_time,
                        created,
                        version,
                        hash,
                    },
                }
            },
        )
        .collect())
}

/// A report's window as its report page reads it: the reports on its head and testbed that
/// started inside the window and ended by the report, newest first and at most
/// [`MAX_CONSOLE_HISTORY_REPORTS`] of them.
fn group_window(
    reports: &[WindowReport],
    report: &QueryReport,
    start_time: DateTime,
) -> (Vec<PointReport>, JsonConsoleWindow) {
    let start = start_time.timestamp();
    let end = report.end_time.timestamp();
    let mut window = reports
        .iter()
        .filter(|candidate| {
            let PointReport {
                id,
                start_time,
                end_time,
                ..
            } = &candidate.report;
            candidate.head_id == report.head_id
                && candidate.testbed_id == report.testbed_id
                && start_time.timestamp() >= start
                && end_time.timestamp() >= start
                && end_time.timestamp() <= end
                && (end_time.timestamp() < end || *id <= report.id)
        })
        .map(|candidate| candidate.report.clone())
        .collect::<Vec<_>>();
    window
        .sort_unstable_by_key(|candidate| Reverse((candidate.end_time.timestamp(), candidate.id)));
    let capped = window.len() > MAX_CONSOLE_HISTORY_REPORTS;
    window.truncate(MAX_CONSOLE_HISTORY_REPORTS);
    let start_time = window
        .iter()
        .map(|candidate| candidate.start_time)
        .min_by_key(DateTime::timestamp)
        .filter(|_| capped)
        .unwrap_or(start_time);
    (
        window,
        JsonConsoleWindow {
            start_time: start_time.into(),
            end_time: report.end_time.into(),
            clamped: capped,
        },
    )
}

fn alerts_json(
    total: u32,
    counts: JsonConsoleAlertsCounts,
    page: Page,
    histories: Vec<GroupHistory>,
) -> JsonConsoleAlerts {
    let Page { groups, lines } = page;
    let mut lines = lines.into_iter();
    let mut tables = Tables::default();
    let mut dimensions = Dimensions::default();
    let mut reports = Vec::new();
    let mut report_index: HashMap<ReportUuid, u32> = HashMap::new();
    let groups = groups
        .into_iter()
        .zip(histories)
        .map(|(group, history)| {
            let PageGroup {
                report,
                version,
                branch,
                testbed,
                total,
                alerts: _,
                modified,
            } = group;
            let GroupHistory {
                window,
                mut thinned,
            } = history;
            // The groups share one table of reports, since nearby reports' windows overlap.
            let shared = thinned
                .reports
                .iter()
                .map(|point_report| {
                    *report_index.entry(point_report.uuid).or_insert_with(|| {
                        reports.push(point_report.clone());
                        to_u32(reports.len() - 1)
                    })
                })
                .collect::<Vec<_>>();
            for report in &mut thinned.points.report {
                *report = shared.get(*report as usize).copied().unwrap_or(*report);
            }
            let mut series = thinned.series.into_iter();
            let alerts = modified
                .into_iter()
                .filter_map(|modified| {
                    let line = lines.next()?;
                    let history = series.next().unwrap_or_default();
                    Some(JsonConsoleAlertLine {
                        line: line_json(&mut tables, line, history),
                        modified: modified.into(),
                    })
                })
                .collect();
            JsonConsoleAlertGroup {
                uuid: report.uuid,
                branch: dimensions.branch(branch, report.head_id),
                testbed: dimensions.testbed(testbed, report.testbed_id),
                version,
                start_time: report.start_time.into(),
                end_time: report.end_time.into(),
                created: report.created.into(),
                adapter: report.adapter.normalize(),
                total: to_u32(total),
                window,
                points: thinned.points,
                alerts,
            }
        })
        .collect();
    let Tables {
        benchmarks,
        variants,
        measures,
        models,
        ..
    } = tables;
    JsonConsoleAlerts {
        total,
        counts,
        groups,
        reports,
        branches: dimensions.branches,
        testbeds: dimensions.testbeds,
        benchmarks,
        variants,
        measures,
        models,
    }
}

/// The branches, each under the head its reports ran on, and testbeds of a page, each once.
#[derive(Default)]
struct Dimensions {
    branches: Vec<JsonConsoleBranch>,
    branch_index: HashMap<(BranchId, HeadId), u32>,
    testbeds: Vec<JsonConsoleTestbed>,
    testbed_index: HashMap<TestbedId, u32>,
}

impl Dimensions {
    fn branch(
        &mut self,
        (id, uuid, name, slug, head): (BranchId, BranchUuid, BranchName, BranchSlug, HeadUuid),
        head_id: HeadId,
    ) -> u32 {
        *self.branch_index.entry((id, head_id)).or_insert_with(|| {
            self.branches.push(JsonConsoleBranch {
                uuid,
                name,
                slug,
                head,
            });
            to_u32(self.branches.len() - 1)
        })
    }

    fn testbed(
        &mut self,
        (uuid, name, slug): (TestbedUuid, ResourceName, TestbedSlug),
        id: TestbedId,
    ) -> u32 {
        *self.testbed_index.entry(id).or_insert_with(|| {
            self.testbeds.push(JsonConsoleTestbed {
                uuid,
                name,
                slug,
                #[cfg(feature = "plus")]
                spec: None,
            });
            to_u32(self.testbeds.len() - 1)
        })
    }
}

fn unique_ids<I: Iterator<Item = ReportId>>(ids: I) -> Vec<ReportId> {
    let mut seen = HashSet::new();
    ids.filter(|id| seen.insert(*id)).collect()
}

fn to_u32(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
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

type Selection = Selected<schema::alert::table>;

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
        reports,
        start_time,
        end_time,
    } = filter;
    check_len("reports", reports.len())?;
    let mut selection = raised_on(project_id, branches, testbeds, measures, thresholds)?;

    if let Some(status) = status {
        selection = Box::new(selection.and(schema::alert::status.eq(status)));
    }

    let alert_report = || {
        schema::boundary::table
            .inner_join(
                schema::metric::table.on(schema::metric::id.eq(schema::boundary::metric_id)),
            )
            .inner_join(
                schema::report_benchmark::table
                    .on(schema::report_benchmark::id.eq(schema::metric::report_benchmark_id)),
            )
            .inner_join(
                schema::report::table
                    .on(schema::report::id.eq(schema::report_benchmark::report_id)),
            )
            .filter(schema::boundary::id.eq(schema::alert::boundary_id))
            .select(schema::boundary::id)
    };
    if !reports.is_empty() {
        selection = Box::new(selection.and(exists(
            alert_report().filter(schema::report::uuid.eq_any(reports)),
        )));
    }
    let since = start_time.map(|start_time| schema::report::created.ge(DateTime::from(start_time)));
    let until = end_time.map(|end_time| schema::report::created.le(DateTime::from(end_time)));
    match (since, until) {
        (Some(since), Some(until)) => {
            selection = Box::new(selection.and(exists(alert_report().filter(since).filter(until))));
        },
        (Some(since), None) => {
            selection = Box::new(selection.and(exists(alert_report().filter(since))));
        },
        (None, Some(until)) => {
            selection = Box::new(selection.and(exists(alert_report().filter(until))));
        },
        (None, None) => {},
    }

    Ok(selection)
}

/// The alerts whose threshold is on the listed branches, testbeds, and measures and is one of
/// the listed thresholds, leaving out those on an archived branch, testbed, or measure.
fn raised_on<QS: 'static>(
    project_id: ProjectId,
    branches: Vec<BranchUuid>,
    testbeds: Vec<TestbedUuid>,
    measures: Vec<MeasureUuid>,
    thresholds: Vec<ThresholdUuid>,
) -> Result<Selected<QS>, HttpError>
where
    schema::alert::threshold_id: SelectableExpression<QS>,
{
    raised_on_with(
        project_id,
        branches,
        testbeds,
        measures,
        thresholds,
        ThresholdIndex::Use,
    )
}

fn raised_on_with<QS: 'static>(
    project_id: ProjectId,
    branches: Vec<BranchUuid>,
    testbeds: Vec<TestbedUuid>,
    measures: Vec<MeasureUuid>,
    thresholds: Vec<ThresholdUuid>,
    index: ThresholdIndex,
) -> Result<Selected<QS>, HttpError>
where
    schema::alert::threshold_id: SelectableExpression<QS>,
{
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
    Ok(match index {
        ThresholdIndex::Use => Box::new(schema::alert::threshold_id.eq_any(threshold_ids)),
        ThresholdIndex::Skip => Box::new((schema::alert::threshold_id + 0).eq_any(threshold_ids)),
    })
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
