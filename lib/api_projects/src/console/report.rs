use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    hash::Hash,
    time::Duration,
};

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{
    BranchName, BranchSlug, BranchUuid, DateTime, DateTimeMillis, GitHash, HeadUuid, MeasureUuid,
    MetricName, ParameterSet, ProjectResourceId, ReportUuid, ResourceName, TestbedSlug,
    TestbedUuid, ThresholdUuid,
    project::{
        alert::AlertStatus,
        console::{
            ConsoleLineGroup, ConsoleLineSort, DEFAULT_CONSOLE_HISTORY_POINTS,
            DEFAULT_CONSOLE_LINES_PER_PAGE, JsonConsoleBranch, JsonConsoleLineGroup,
            JsonConsoleReport, JsonConsoleReportCounts, JsonConsoleReportLine,
            JsonConsoleReportLink, JsonConsoleReportQueryParams, JsonConsoleSeries,
            JsonConsoleTestbed, JsonConsoleWindow, MAX_CONSOLE_HISTORY_POINTS,
            MAX_CONSOLE_HISTORY_REPORTS, MAX_CONSOLE_WINDOW_DAYS,
        },
        head::{JsonVersion, VersionNumber},
        report::{Adapter, Iteration, JsonReportAlertsCounts},
    },
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    actor_conn,
    context::{ApiContext, DbConnection},
    error::{bad_request_error, resource_not_found_err, with_auth_hint},
    model::{
        project::{
            QueryProject,
            benchmark::BenchmarkId,
            branch::BranchId,
            measure::MeasureId,
            metric::MetricId,
            report::{QueryReport, ReportId},
            threshold::ThresholdId,
            variant::VariantId,
        },
        user::actor::{ApiActor, PubProjectBearerToken},
    },
    schema,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, JoinOnDsl as _,
    NullableExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{
    AlertRow, BenchmarkRow, Check, Limits, MeasureRow, ModelRow, PointReport, Points,
    PointsBuilder, SeriesBuilder, Tables, Thinned, VariantRow, alert_json, before, thin, unique,
};
use crate::perf::DEFAULT_REPORT_HISTORY;

#[derive(Deserialize, JsonSchema)]
pub struct ProjConsoleReportParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The UUID for a report.
    pub report: ReportUuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/console/reports/{report}",
    tags = ["projects", "reports"],
    unpublished = true,
}]
pub async fn proj_console_report_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjConsoleReportParams>,
    _query_params: Query<JsonConsoleReportQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View a report's lines for the console
///
/// One page of a report's lines in drawing order, each with its history over a
/// window that ends at the report, and the report's identity and neighbors.
/// The user must be authenticated, with `view` permissions for a private project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/console/reports/{report}",
    tags = ["projects", "reports"],
    unpublished = true,
}]
pub async fn proj_console_report_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjConsoleReportParams>,
    query_params: Query<JsonConsoleReportQueryParams>,
) -> Result<ResponseOk<JsonConsoleReport>, HttpError> {
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
        path_params.into_inner(),
        query_params.into_inner(),
        &api_actor,
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::response_ok(json, api_actor.is_auth()))
}

async fn get_inner(
    context: &ApiContext,
    path_params: ProjConsoleReportParams,
    query_params: JsonConsoleReportQueryParams,
    api_actor: &ApiActor,
) -> Result<JsonConsoleReport, HttpError> {
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

    let Identity {
        report,
        version,
        branch_id,
        branch,
        testbed,
    } = identity(conn, &project, path_params.report)?;

    let requested = requested_start(&report, query_params.start_time, query_params.window)?;
    let points = history_points(query_params.points)?;
    let filters = Filters::new(&query_params)?;
    let (window_reports, window) = history_window(conn, &report, requested)
        .map_err(resource_not_found_err!(Report, (&project, report.uuid)))?;
    let (previous, next) = neighbors(conn, branch_id, &report)
        .map_err(resource_not_found_err!(Report, (&project, report.uuid)))?;

    let rows = line_rows(conn, report.id)
        .map_err(resource_not_found_err!(Metric, (&project, report.uuid)))?;
    let ReportLines { lines, alerts } = ReportLines::new(rows);
    let counts = report_counts(&lines, alerts);
    let lines = search(lines, query_params.search.as_deref())
        .into_iter()
        .filter(|line| filters.matches(line))
        .collect::<Vec<_>>();
    let total = to_u32(lines.len());

    let group = query_params.group.unwrap_or_default();
    let sort = query_params.sort.unwrap_or_default();
    let ordered = order(lines, group, sort);

    let mut tables = Tables::default();
    let groups = groups_json(
        &mut tables,
        group,
        &ordered,
        query_params.page,
        query_params.per_page,
    );

    let page_lines = page(ordered, query_params.page, query_params.per_page);
    let history = page_history(conn, &window_reports, &page_lines, report.id, points)
        .map_err(resource_not_found_err!(Metric, (&project, report.uuid)))?;
    let lines = page_lines
        .into_iter()
        .zip(history.series)
        .map(|(line, history)| line_json(&mut tables, line, history))
        .collect();

    let Tables {
        benchmarks,
        variants,
        measures,
        models,
        ..
    } = tables;
    let QueryReport {
        uuid,
        adapter,
        start_time,
        end_time,
        ..
    } = report;
    Ok(JsonConsoleReport {
        uuid,
        branch,
        testbed,
        version,
        start_time: start_time.into(),
        end_time: end_time.into(),
        adapter: adapter.normalize(),
        counts,
        previous,
        next,
        window,
        total,
        groups,
        lines,
        points: history.points,
        reports: history.reports,
        benchmarks,
        variants,
        measures,
        models,
    })
}

/// The start a request asks for: a start time, or a window of whole days before
/// the report's start, never both.
fn requested_start(
    report: &QueryReport,
    start_time: Option<DateTimeMillis>,
    window: Option<u16>,
) -> Result<Option<DateTime>, HttpError> {
    match (start_time, window) {
        (Some(_), Some(_)) => Err(bad_request_error(
            "Ask for either a start time or a window, not both",
        )),
        (Some(start_time), None) => Ok(Some(start_time.into())),
        (None, Some(days)) if (1..=MAX_CONSOLE_WINDOW_DAYS).contains(&days) => Ok(before(
            report.start_time,
            Duration::from_hours(24 * u64::from(days)),
        )),
        (None, Some(days)) => Err(bad_request_error(format!(
            "A window is from 1 to {MAX_CONSOLE_WINDOW_DAYS} days, not {days}"
        ))),
        (None, None) => Ok(None),
    }
}

/// The size a request asks each line's history to be thinned to.
fn history_points(points: Option<u16>) -> Result<usize, HttpError> {
    match points {
        None => Ok(usize::from(DEFAULT_CONSOLE_HISTORY_POINTS)),
        Some(points) if (2..=MAX_CONSOLE_HISTORY_POINTS).contains(&points) => {
            Ok(usize::from(points))
        },
        Some(points) => Err(bad_request_error(format!(
            "A history is from 2 to {MAX_CONSOLE_HISTORY_POINTS} points, not {points}"
        ))),
    }
}

/// The window starts four weeks before the report unless the request says
/// otherwise, never after it, and ends with it.
fn history_window(
    conn: &mut DbConnection,
    report: &QueryReport,
    start_time: Option<DateTime>,
) -> diesel::QueryResult<(Vec<PointReport>, JsonConsoleWindow)> {
    let requested = start_time
        .or_else(|| before(report.start_time, DEFAULT_REPORT_HISTORY))
        .unwrap_or(report.start_time);
    let start_time = if requested.timestamp() > report.start_time.timestamp() {
        report.start_time
    } else {
        requested
    };
    let (reports, capped) = window_reports(conn, report, start_time)?;
    let start_time = reports
        .iter()
        .map(|report| report.start_time)
        .min_by_key(DateTime::timestamp)
        .filter(|_| capped)
        .unwrap_or(start_time);
    let window = JsonConsoleWindow {
        start_time: start_time.into(),
        end_time: report.end_time.into(),
        clamped: capped,
    };
    Ok((reports, window))
}

/// The lines whose benchmark, variant parameters, measure, or metric hold the
/// search, ignoring case, read the way a row reads: `benchmark key=value measure metric`.
fn search(lines: Vec<Line>, search: Option<&str>) -> Vec<Line> {
    let Some(search) = search.map(str::to_lowercase) else {
        return lines;
    };
    lines
        .into_iter()
        .filter(|line| {
            let (_, _, benchmark, _) = &line.benchmark;
            let (_, _, parameters) = &line.variant;
            let (_, _, measure, _, _) = &line.measure;
            let mut row = AsRef::<str>::as_ref(benchmark).to_owned();
            for tag in parameters.tags() {
                row.push(' ');
                row.push_str(&tag);
            }
            row.push(' ');
            row.push_str(measure.as_ref());
            row.push(' ');
            row.push_str(line.metric.as_ref());
            row.to_lowercase().contains(&search)
        })
        .collect()
}

/// The lines a request narrows to by measure, metric name, and parameters.
struct Filters {
    measure: Option<MeasureUuid>,
    metric: Option<MetricName>,
    parameters: Option<ParameterSet>,
}

impl Filters {
    fn new(query_params: &JsonConsoleReportQueryParams) -> Result<Self, HttpError> {
        Ok(Self {
            measure: query_params.measure,
            metric: query_params.metric.clone(),
            parameters: query_params
                .parameters
                .as_deref()
                .map(str::parse)
                .transpose()
                .map_err(bad_request_error)?,
        })
    }

    fn matches(&self, line: &Line) -> bool {
        self.measure.is_none_or(|measure| line.measure.1 == measure)
            && self
                .metric
                .as_ref()
                .is_none_or(|metric| line.metric == *metric)
            && self
                .parameters
                .as_ref()
                .is_none_or(|parameters| parameters.is_subset_of(&line.variant.2))
    }
}

fn page(groups: Vec<Vec<Line>>, page: Option<u32>, per_page: Option<u8>) -> Vec<Line> {
    let per_page = usize::from(per_page.unwrap_or(DEFAULT_CONSOLE_LINES_PER_PAGE));
    let page = usize::try_from(page.unwrap_or(1)).unwrap_or(usize::MAX);
    groups
        .into_iter()
        .flatten()
        .skip(page.saturating_sub(1).saturating_mul(per_page))
        .take(per_page)
        .collect()
}

struct Identity {
    report: QueryReport,
    version: JsonVersion,
    branch_id: BranchId,
    branch: JsonConsoleBranch,
    testbed: JsonConsoleTestbed,
}

type IdentityRow = (
    QueryReport,
    VersionNumber,
    Option<GitHash>,
    (BranchId, BranchUuid, BranchName, BranchSlug, HeadUuid),
    (TestbedUuid, ResourceName, TestbedSlug),
);

fn identity(
    conn: &mut DbConnection,
    project: &QueryProject,
    report_uuid: ReportUuid,
) -> Result<Identity, HttpError> {
    let (
        report,
        number,
        hash,
        (branch_id, branch_uuid, branch_name, branch_slug, head_uuid),
        testbed,
    ) = schema::report::table
        .inner_join(schema::version::table.on(schema::version::id.eq(schema::report::version_id)))
        .inner_join(schema::head::table.on(schema::head::id.eq(schema::report::head_id)))
        .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::head::branch_id)))
        .inner_join(schema::testbed::table.on(schema::testbed::id.eq(schema::report::testbed_id)))
        .filter(schema::report::project_id.eq(project.id))
        .filter(schema::report::uuid.eq(report_uuid))
        .select((
            QueryReport::as_select(),
            schema::version::number,
            schema::version::hash,
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
        ))
        .first::<IdentityRow>(conn)
        .map_err(resource_not_found_err!(Report, (project, report_uuid)))?;
    let (testbed_uuid, testbed_name, testbed_slug) = testbed;
    Ok(Identity {
        report,
        version: JsonVersion { number, hash },
        branch_id,
        branch: JsonConsoleBranch {
            uuid: branch_uuid,
            name: branch_name,
            slug: branch_slug,
            head: head_uuid,
        },
        testbed: JsonConsoleTestbed {
            uuid: testbed_uuid,
            name: testbed_name,
            slug: testbed_slug,
            #[cfg(feature = "plus")]
            spec: None,
        },
    })
}

type Neighbors = (Option<JsonConsoleReportLink>, Option<JsonConsoleReportLink>);

fn neighbors(
    conn: &mut DbConnection,
    branch_id: BranchId,
    report: &QueryReport,
) -> diesel::QueryResult<Neighbors> {
    Ok((
        neighbor(conn, branch_id, report, Neighbor::Previous)?,
        neighbor(conn, branch_id, report, Neighbor::Next)?,
    ))
}

#[derive(Clone, Copy)]
enum Neighbor {
    Previous,
    Next,
}

/// The closest report on the same branch, under any of its heads, and the same
/// testbed, by when it ended.
fn neighbor(
    conn: &mut DbConnection,
    branch_id: BranchId,
    report: &QueryReport,
    neighbor: Neighbor,
) -> diesel::QueryResult<Option<JsonConsoleReportLink>> {
    let QueryReport {
        id,
        testbed_id,
        end_time,
        ..
    } = *report;
    let query = schema::report::table
        .inner_join(schema::head::table.on(schema::head::id.eq(schema::report::head_id)))
        .inner_join(schema::version::table.on(schema::version::id.eq(schema::report::version_id)))
        .filter(schema::head::branch_id.eq(branch_id))
        .filter(schema::report::testbed_id.eq(testbed_id))
        .select((
            schema::report::uuid,
            schema::report::start_time,
            schema::version::hash,
            schema::report::adapter,
        ))
        .into_boxed();
    // The plain bound starts the index walk at this report; the `OR` breaks the tie.
    let query = match neighbor {
        Neighbor::Previous => query
            .filter(schema::report::end_time.le(end_time))
            .filter(
                schema::report::end_time
                    .lt(end_time)
                    .or(schema::report::id.lt(id)),
            )
            .order((schema::report::end_time.desc(), schema::report::id.desc())),
        Neighbor::Next => query
            .filter(schema::report::end_time.ge(end_time))
            .filter(
                schema::report::end_time
                    .gt(end_time)
                    .or(schema::report::id.gt(id)),
            )
            .order((schema::report::end_time.asc(), schema::report::id.asc())),
    };
    Ok(query
        .first::<(ReportUuid, DateTime, Option<GitHash>, Adapter)>(conn)
        .optional()?
        .map(|(uuid, start_time, hash, adapter)| JsonConsoleReportLink {
            uuid,
            start_time: start_time.into(),
            hash,
            adapter: adapter.normalize(),
        }))
}

/// The window's reports on the report's own head and testbed, up to and
/// including the report itself, newest first and at most
/// [`MAX_CONSOLE_HISTORY_REPORTS`] of them, with whether there were more.
///
/// A report comes before another when it ended first, which is the order the
/// `(testbed_id, end_time)` index can walk.
fn window_reports(
    conn: &mut DbConnection,
    report: &QueryReport,
    window_start: DateTime,
) -> diesel::QueryResult<(Vec<PointReport>, bool)> {
    let QueryReport {
        id,
        head_id,
        testbed_id,
        end_time,
        ..
    } = *report;
    let mut reports = schema::report::table
        .inner_join(
            schema::head_version::table
                .on(schema::head_version::version_id.eq(schema::report::version_id)),
        )
        .inner_join(schema::version::table.on(schema::version::id.eq(schema::report::version_id)))
        .filter(schema::head_version::head_id.eq(head_id))
        .filter(schema::report::testbed_id.eq(testbed_id))
        .filter(schema::report::start_time.ge(window_start))
        // Implied by the start, and what bounds the index range from below.
        .filter(schema::report::end_time.ge(window_start))
        .filter(schema::report::end_time.le(end_time))
        .filter(
            schema::report::end_time
                .lt(end_time)
                .or(schema::report::id.le(id)),
        )
        .order((schema::report::end_time.desc(), schema::report::id.desc()))
        .limit(i64::try_from(MAX_CONSOLE_HISTORY_REPORTS + 1).unwrap_or(i64::MAX))
        .select((
            schema::report::id,
            schema::report::uuid,
            schema::report::start_time,
            schema::report::end_time,
            schema::report::created,
            schema::version::number,
            schema::version::hash,
        ))
        .load::<(
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
            |(id, uuid, start_time, end_time, created, version, hash)| PointReport {
                id,
                uuid,
                start_time,
                end_time,
                created,
                version,
                hash,
            },
        )
        .collect::<Vec<_>>();
    let capped = reports.len() > MAX_CONSOLE_HISTORY_REPORTS;
    reports.truncate(MAX_CONSOLE_HISTORY_REPORTS);
    Ok((reports, capped))
}

/// One metric of the report, once per threshold that checked it.
type LineRow = (
    Iteration,
    BenchmarkRow,
    VariantRow,
    MeasureRow,
    (MetricId, MetricName, f64),
    Option<(BoundaryRow, ThresholdUuid, ModelRow, Option<AlertRow>)>,
);

type BoundaryRow = (ThresholdId, Option<f64>, Option<f64>, Option<f64>);

fn line_rows(conn: &mut DbConnection, report_id: ReportId) -> diesel::QueryResult<Vec<LineRow>> {
    schema::report_benchmark::table
        .inner_join(
            schema::benchmark::table
                .on(schema::benchmark::id.eq(schema::report_benchmark::benchmark_id)),
        )
        .inner_join(
            schema::variant::table.on(schema::variant::id.eq(schema::report_benchmark::variant_id)),
        )
        .inner_join(
            schema::metric::table
                .on(schema::metric::report_benchmark_id.eq(schema::report_benchmark::id)),
        )
        .inner_join(schema::measure::table.on(schema::measure::id.eq(schema::metric::measure_id)))
        // Flat, with explicit `ON` clauses, so SQLite seeks each outer join
        // instead of scanning the boundary table.
        .left_join(schema::boundary::table.on(schema::boundary::metric_id.eq(schema::metric::id)))
        .left_join(
            schema::threshold::table.on(schema::threshold::id.eq(schema::boundary::threshold_id)),
        )
        .left_join(schema::model::table.on(schema::model::id.eq(schema::boundary::model_id)))
        .left_join(schema::alert::table.on(schema::alert::boundary_id.eq(schema::boundary::id)))
        .filter(schema::report_benchmark::report_id.eq(report_id))
        .select((
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
                )
                    .nullable(),
            )
                .nullable(),
        ))
        .load::<LineRow>(conn)
}

/// A line: one metric name of one measure of one variant.
#[derive(Clone, PartialEq, Eq, Hash)]
struct LineKey {
    variant: VariantId,
    measure: MeasureId,
    metric: MetricName,
}

/// A line as its row shows it in this report.
struct Line {
    benchmark: BenchmarkRow,
    variant: VariantRow,
    measure: MeasureRow,
    metric: MetricName,
    iteration: Iteration,
    value: f64,
    check: Option<Check>,
}

impl Line {
    fn key(&self) -> LineKey {
        LineKey {
            variant: self.variant.0,
            measure: self.measure.0,
            metric: self.metric.clone(),
        }
    }

    fn alerting(&self) -> bool {
        self.check
            .as_ref()
            .is_some_and(|check| check.alert.is_some())
    }

    fn score(&self) -> Option<f64> {
        self.check
            .as_ref()
            .and_then(|check| check.score(self.value))
    }

    fn group_key(&self, group: ConsoleLineGroup) -> GroupKey {
        match group {
            ConsoleLineGroup::Benchmark => GroupKey::Benchmark(self.benchmark.0),
            ConsoleLineGroup::Measure => GroupKey::Measure(self.measure.0),
        }
    }

    /// Benchmark, variant parameters, measure, and metric name, with the row
    /// identifiers last so the order is total.
    fn cmp_name(&self, other: &Self) -> Ordering {
        let (benchmark_id, _, benchmark_name, _) = &self.benchmark;
        let (variant_id, _, parameters) = &self.variant;
        let (measure_id, _, measure_name, _, _) = &self.measure;
        let (other_benchmark_id, _, other_benchmark_name, _) = &other.benchmark;
        let (other_variant_id, _, other_parameters) = &other.variant;
        let (other_measure_id, _, other_measure_name, _, _) = &other.measure;
        AsRef::<str>::as_ref(benchmark_name)
            .cmp(AsRef::<str>::as_ref(other_benchmark_name))
            .then_with(|| parameters.cmp(other_parameters))
            .then_with(|| {
                AsRef::<str>::as_ref(measure_name).cmp(AsRef::<str>::as_ref(other_measure_name))
            })
            .then_with(|| {
                AsRef::<str>::as_ref(&self.metric).cmp(AsRef::<str>::as_ref(&other.metric))
            })
            .then_with(|| benchmark_id.cmp(other_benchmark_id))
            .then_with(|| variant_id.cmp(other_variant_id))
            .then_with(|| measure_id.cmp(other_measure_id))
    }

    /// The group's title, with its row identifier so the order is total.
    fn cmp_group(&self, other: &Self, group: ConsoleLineGroup) -> Ordering {
        match group {
            ConsoleLineGroup::Benchmark => {
                let (id, _, name, _) = &self.benchmark;
                let (other_id, _, other_name, _) = &other.benchmark;
                AsRef::<str>::as_ref(name)
                    .cmp(AsRef::<str>::as_ref(other_name))
                    .then_with(|| id.cmp(other_id))
            },
            ConsoleLineGroup::Measure => {
                let (id, _, name, _, _) = &self.measure;
                let (other_id, _, other_name, _, _) = &other.measure;
                AsRef::<str>::as_ref(name)
                    .cmp(AsRef::<str>::as_ref(other_name))
                    .then_with(|| id.cmp(other_id))
            },
        }
    }
}

#[derive(PartialEq, Eq, Hash)]
enum GroupKey {
    Benchmark(BenchmarkId),
    Measure(MeasureId),
}

struct ReportLines {
    lines: Vec<Line>,
    alerts: JsonReportAlertsCounts,
}

impl ReportLines {
    fn new(rows: Vec<LineRow>) -> Self {
        let mut alert_statuses = HashMap::new();
        let mut metrics: Vec<(Line, Vec<Check>)> = Vec::new();
        let mut metric_index: HashMap<MetricId, usize> = HashMap::new();
        for (iteration, benchmark, variant, measure, (metric_id, metric, value), check) in rows {
            let position = *metric_index.entry(metric_id).or_insert_with(|| {
                metrics.push((
                    Line {
                        benchmark,
                        variant,
                        measure,
                        metric,
                        iteration,
                        value,
                        check: None,
                    },
                    Vec::new(),
                ));
                metrics.len() - 1
            });
            let Some((boundary, threshold_uuid, model, alert)) = check else {
                continue;
            };
            if let Some((uuid, _, status)) = alert {
                alert_statuses.insert(uuid, status);
            }
            let (threshold_id, baseline, lower_limit, upper_limit) = boundary;
            if let Some((_, checks)) = metrics.get_mut(position) {
                checks.push(Check {
                    threshold_id,
                    threshold_uuid,
                    model,
                    limits: Limits {
                        baseline,
                        lower_limit,
                        upper_limit,
                    },
                    alert: alert_json(alert),
                });
            }
        }

        // A line's row is the iteration that alerted, and otherwise its last.
        let mut lines: HashMap<LineKey, Line> = HashMap::new();
        for (mut line, checks) in metrics {
            line.check = Check::choose(checks);
            let replace = lines.get(&line.key()).is_none_or(|kept| {
                (line.alerting(), line.iteration.0) > (kept.alerting(), kept.iteration.0)
            });
            if replace {
                lines.insert(line.key(), line);
            }
        }

        let active = alert_statuses
            .values()
            .filter(|status| matches!(status, AlertStatus::Active))
            .count();
        Self {
            lines: lines.into_values().collect(),
            alerts: JsonReportAlertsCounts {
                total: to_u32(alert_statuses.len()),
                active: to_u32(active),
            },
        }
    }
}

/// The lines in drawing order, cut into their groups.
///
/// By name, groups with an alert come first and alerting lines lead their group,
/// then the rest by name. By delta, lines go worst first inside their group and
/// groups by their worst line, with lines no threshold scored last.
fn order(lines: Vec<Line>, group: ConsoleLineGroup, sort: ConsoleLineSort) -> Vec<Vec<Line>> {
    let mut by_key: HashMap<GroupKey, Vec<Line>> = HashMap::new();
    for line in lines {
        by_key.entry(line.group_key(group)).or_default().push(line);
    }
    let mut groups = by_key.into_values().collect::<Vec<_>>();
    for lines in &mut groups {
        match sort {
            ConsoleLineSort::Name => {
                lines.sort_by(|a, b| b.alerting().cmp(&a.alerting()).then_with(|| a.cmp_name(b)));
            },
            ConsoleLineSort::Delta => {
                lines.sort_by(|a, b| worst_first(a.score(), b.score()).then_with(|| a.cmp_name(b)));
            },
        }
    }
    groups.sort_by(|a, b| {
        let (Some(first_a), Some(first_b)) = (a.first(), b.first()) else {
            return Ordering::Equal;
        };
        // Each group is sorted already, so its first line says whether it
        // alerted, and is its worst.
        let lead = match sort {
            ConsoleLineSort::Name => first_b.alerting().cmp(&first_a.alerting()),
            ConsoleLineSort::Delta => worst_first(first_a.score(), first_b.score()),
        };
        lead.then_with(|| first_a.cmp_group(first_b, group))
    });
    groups
}

fn worst_first(a: Option<f64>, b: Option<f64>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => b.total_cmp(&a),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// The client holds the first page, so later pages leave the groups out.
fn groups_json(
    tables: &mut Tables,
    group: ConsoleLineGroup,
    ordered: &[Vec<Line>],
    page: Option<u32>,
    per_page: Option<u8>,
) -> Vec<JsonConsoleLineGroup> {
    // A page of no lines only counts them.
    if page.unwrap_or(1) > 1 || per_page == Some(0) {
        return Vec::new();
    }
    ordered
        .iter()
        .map(|lines| group_json(tables, group, lines))
        .collect()
}

fn group_json(
    tables: &mut Tables,
    group: ConsoleLineGroup,
    lines: &[Line],
) -> JsonConsoleLineGroup {
    let key = match (group, lines.first()) {
        (ConsoleLineGroup::Benchmark, Some(line)) => tables.benchmark(&line.benchmark),
        (ConsoleLineGroup::Measure, Some(line)) => tables.measure(&line.measure),
        (ConsoleLineGroup::Benchmark | ConsoleLineGroup::Measure, None) => {
            debug_assert!(false, "a group has a line");
            0
        },
    };
    JsonConsoleLineGroup {
        key,
        lines: to_u32(lines.len()),
        variants: count(lines.iter().map(|line| line.variant.0)),
        alerts: to_u32(lines.iter().filter(|line| line.alerting()).count()),
    }
}

fn line_json(tables: &mut Tables, line: Line, history: JsonConsoleSeries) -> JsonConsoleReportLine {
    let Line {
        benchmark,
        variant,
        measure,
        metric,
        iteration: _,
        value,
        check,
    } = line;
    let benchmark = tables.benchmark(&benchmark);
    let variant = tables.variant(&variant, benchmark);
    let measure = tables.measure(&measure);
    let model = check.as_ref().map(|check| tables.model(check));
    let (limits, alert) = check.map_or(
        (
            Limits {
                baseline: None,
                lower_limit: None,
                upper_limit: None,
            },
            None,
        ),
        |check| (check.limits, check.alert),
    );
    let Limits {
        baseline,
        lower_limit,
        upper_limit,
    } = limits;
    JsonConsoleReportLine {
        benchmark,
        variant,
        measure,
        metric,
        value,
        model,
        baseline,
        lower_limit,
        upper_limit,
        alert,
        history,
    }
}

/// Each metric of the page's lines in the window's reports, with the boundary
/// and alert of the threshold its line follows.
type HistoryRow = (
    ReportId,
    Iteration,
    VariantId,
    MeasureId,
    MetricName,
    f64,
    Option<BoundaryRow>,
    Option<AlertRow>,
);

/// One statement for every line of the page: the window's reports crossed with
/// the page's variants and measures, each pair an index seek.
fn history_rows(
    conn: &mut DbConnection,
    window_reports: &[PointReport],
    lines: &[Line],
) -> diesel::QueryResult<Vec<HistoryRow>> {
    let report_ids = window_reports
        .iter()
        .map(|report| report.id)
        .collect::<Vec<_>>();
    let variant_ids = unique(lines.iter().map(|line| line.variant.0));
    let measure_ids = unique(lines.iter().map(|line| line.measure.0));
    let threshold_ids = unique(
        lines
            .iter()
            .filter_map(|line| line.check.as_ref().map(|check| check.threshold_id)),
    );
    schema::metric::table
        .inner_join(
            schema::report_benchmark::table
                .on(schema::report_benchmark::id.eq(schema::metric::report_benchmark_id)),
        )
        // The threshold filter belongs in the join, so a point with no boundary
        // from these thresholds still comes back.
        .left_join(
            schema::boundary::table.on(schema::boundary::metric_id
                .eq(schema::metric::id)
                .and(schema::boundary::threshold_id.eq_any(threshold_ids))),
        )
        .left_join(schema::alert::table.on(schema::alert::boundary_id.eq(schema::boundary::id)))
        .filter(schema::report_benchmark::report_id.eq_any(report_ids))
        .filter(schema::report_benchmark::variant_id.eq_any(variant_ids))
        // SQLite would otherwise drive the read off `index_metric_measure`,
        // which spans every report the measure was ever in.
        .filter((schema::metric::measure_id + 0).eq_any(measure_ids))
        .select((
            schema::report_benchmark::report_id,
            schema::report_benchmark::iteration,
            schema::report_benchmark::variant_id,
            schema::metric::measure_id,
            schema::metric::name,
            schema::metric::value,
            (
                schema::boundary::threshold_id,
                schema::boundary::baseline,
                schema::boundary::lower_limit,
                schema::boundary::upper_limit,
            )
                .nullable(),
            (
                schema::alert::uuid,
                schema::alert::boundary_limit,
                schema::alert::status,
            )
                .nullable(),
        ))
        .load::<HistoryRow>(conn)
}

/// The page's lines over the window, each thinned to `points`.
fn page_history(
    conn: &mut DbConnection,
    window_reports: &[PointReport],
    lines: &[Line],
    report_id: ReportId,
    points: usize,
) -> diesel::QueryResult<Thinned> {
    let History { points: x, series } = if lines.is_empty() {
        History::default()
    } else {
        let rows = history_rows(conn, window_reports, lines)?;
        History::new(window_reports, lines, rows)
    };
    Ok(thin(x, series, report_id, points))
}

#[derive(Default)]
struct History {
    points: Points,
    series: Vec<JsonConsoleSeries>,
}

impl History {
    fn new(window_reports: &[PointReport], lines: &[Line], rows: Vec<HistoryRow>) -> Self {
        let reports = window_reports
            .iter()
            .map(|report| (report.id, report))
            .collect::<HashMap<_, _>>();
        let line_index = lines
            .iter()
            .enumerate()
            .map(|(position, line)| (line.key(), position))
            .collect::<HashMap<_, _>>();
        // The variants and measures cross, so keep only the rows of a page line.
        let rows = rows
            .into_iter()
            .filter_map(|row| {
                let (_, _, variant, measure, metric, ..) = &row;
                let key = LineKey {
                    variant: *variant,
                    measure: *measure,
                    metric: metric.clone(),
                };
                line_index.get(&key).map(|position| (*position, row))
            })
            .collect::<Vec<_>>();

        let mut points = PointsBuilder::default();
        for (_, (report_id, iteration, ..)) in &rows {
            if let Some(report) = reports.get(report_id) {
                points.add(report, *iteration);
            }
        }
        let points = points.build();

        let mut series = lines
            .iter()
            .map(|_| SeriesBuilder::new(points.json.x.len()))
            .collect::<Vec<_>>();
        for (position, (report_id, iteration, _, _, _, value, boundary, alert)) in rows {
            let (Some(point), Some(builder), Some(line)) = (
                points.index.get(&(report_id, iteration)).copied(),
                series.get_mut(position),
                lines.get(position),
            ) else {
                continue;
            };
            builder.value(point, value);
            let followed = line.check.as_ref().map(|check| check.threshold_id);
            if let Some((threshold_id, baseline, lower_limit, upper_limit)) = boundary
                && followed == Some(threshold_id)
            {
                builder.limits(
                    point,
                    Limits {
                        baseline,
                        lower_limit,
                        upper_limit,
                    },
                    alert_json(alert),
                );
            }
        }

        Self {
            points,
            series: series.into_iter().map(SeriesBuilder::build).collect(),
        }
    }
}

fn report_counts(lines: &[Line], alerts: JsonReportAlertsCounts) -> JsonConsoleReportCounts {
    JsonConsoleReportCounts {
        benchmarks: count(lines.iter().map(|line| line.benchmark.0)),
        variants: count(lines.iter().map(|line| line.variant.0)),
        measures: count(lines.iter().map(|line| line.measure.0)),
        metrics: count(lines.iter().map(|line| line.metric.clone())),
        alerts,
    }
}

fn count<T: Eq + Hash, I: Iterator<Item = T>>(items: I) -> u32 {
    to_u32(items.collect::<HashSet<_>>().len())
}

fn to_u32(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}
