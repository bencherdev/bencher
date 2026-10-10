use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashMap, HashSet},
    hash::Hash,
};

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
#[cfg(feature = "plus")]
use bencher_json::SpecUuid;
use bencher_json::{
    BenchmarkUuid, BranchName, BranchSlug, BranchUuid, DateTime, GitHash, HeadUuid, JsonPerfQuery,
    MeasureUuid, MetricName, ParameterSet, ProjectResourceId, ReportBenchmarkUuid, ReportUuid,
    ResourceName, TestbedSlug, TestbedUuid, ThresholdUuid,
    project::{
        console::{
            JsonConsoleAlert, JsonConsoleBranch, JsonConsoleModel, JsonConsolePerf,
            JsonConsolePerfLine, JsonConsolePerfQuery, JsonConsolePerfQueryParams,
            JsonConsoleTestbed, JsonConsoleWindow, MAX_CONSOLE_PLOT_LINES,
        },
        head::VersionNumber,
        report::Iteration,
    },
};
use bencher_schema::{
    actor_conn,
    context::{ApiContext, DbConnection},
    error::{bad_request_error, resource_not_found_err, with_auth_hint},
    model::{
        project::{
            ProjectId, QueryProject,
            branch::{BranchId, head::HeadId},
            measure::MeasureId,
            report::ReportId,
            testbed::TestbedId,
            threshold::{ThresholdId, model::ModelId},
            variant::VariantId,
        },
        spec::SpecId,
        user::actor::{ApiActor, PubProjectBearerToken},
    },
    schema,
};
use diesel::{
    ExpressionMethods as _, JoinOnDsl as _, NullableExpressionMethods as _, QueryDsl as _,
    RunQueryDsl as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{
    AlertRow, BenchmarkRow, Limits, MeasureRow, ModelRow, PointReport, PointsBuilder, ReportOrder,
    SeriesBuilder, Tables, VariantRow, alert_json, before, clamp_start, model_json, unique,
};
use crate::perf::DEFAULT_REPORT_HISTORY;

#[derive(Deserialize, JsonSchema)]
pub struct ProjConsolePerfParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/console/perf",
    tags = ["projects", "perf"],
    unpublished = true,
}]
pub async fn proj_console_perf_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjConsolePerfParams>,
    _query_params: Query<JsonConsolePerfQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// Query a plot for the console
///
/// The lines of a plot query, as columns aligned to one shared x.
/// The lines are the product of the boxes: every branch, testbed, variant of each
/// benchmark that the `parameters` filter matches, measure, and metric name.
/// With no metric names, each branch, testbed, variant, and measure draws the names it reported
/// in the window.
/// Only the first 64 lines are drawn, and the response counts them all.
/// If there is no `start_time`, then the four weeks before the end are queried.
/// If the project is public, then the user does not need to be authenticated,
/// and an unauthenticated window reaches back at most three months.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/console/perf",
    tags = ["projects", "perf"],
    unpublished = true,
}]
pub async fn proj_console_perf_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjConsolePerfParams>,
    query_params: Query<JsonConsolePerfQueryParams>,
) -> Result<ResponseOk<JsonConsolePerf>, HttpError> {
    let query = query_params
        .into_inner()
        .try_into()
        .map_err(bad_request_error)?;
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_inner(rqctx.context(), path_params.into_inner(), query, &api_actor)
        .await
        .map_err(with_auth_hint)?;
    Ok(Get::response_ok(json, api_actor.is_auth()))
}

async fn get_inner(
    context: &ApiContext,
    path_params: ProjConsolePerfParams,
    query: JsonConsolePerfQuery,
    api_actor: &ApiActor,
) -> Result<JsonConsolePerf, HttpError> {
    let project = QueryProject::is_allowed_actor_pub(
        actor_conn!(context, api_actor),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        api_actor,
    )?;
    let conn = actor_conn!(context, api_actor);

    let JsonConsolePerfQuery { perf, metrics } = query;
    let JsonPerfQuery {
        branches,
        heads,
        testbeds,
        #[cfg(feature = "plus")]
        specs,
        benchmarks,
        parameters,
        measures,
        start_time,
        end_time,
    } = perf;

    let end_time = end_time.unwrap_or_else(|| context.clock.now());
    if let Some(start_time) = start_time
        && start_time.timestamp() > end_time.timestamp()
    {
        return Err(bad_request_error(format!(
            "The start time ({start_time}) is after the end time ({end_time})"
        )));
    }
    let requested = start_time
        .or_else(|| before(end_time, DEFAULT_REPORT_HISTORY))
        .unwrap_or(end_time);
    let (start_time, clamped) = clamp_start(&context.clock, api_actor, requested);
    // A window that ends before the reach reads nothing, and is echoed as empty
    // rather than inverted.
    let reachable = start_time.timestamp() <= end_time.timestamp();
    let window = JsonConsoleWindow {
        start_time: if reachable { start_time } else { end_time }.into(),
        end_time: end_time.into(),
        clamped,
    };
    let read_window = Window {
        start_time,
        end_time,
    };

    let boxes = Boxes {
        branches: branch_heads(conn, &project, &branches, &heads)
            .map_err(resource_not_found_err!(Branch, &project))?,
        testbeds: testbed_specs(
            conn,
            &project,
            &testbeds,
            #[cfg(feature = "plus")]
            &specs,
        )
        .map_err(resource_not_found_err!(Testbed, &project))?,
        benchmarks: benchmark_variants(conn, &project, &benchmarks, parameters.as_deref())
            .map_err(resource_not_found_err!(Benchmark, &project))?,
        measures: measure_rows(conn, &project, &measures)
            .map_err(resource_not_found_err!(Measure, &project))?,
    };
    let series = boxes.series();

    let names = match metrics {
        Some(metrics) => Names::Every(first_of_each(metrics, Clone::clone)),
        None => Names::Reported(
            reported_names(conn, &boxes, &series, read_window)
                .map_err(resource_not_found_err!(Metric, &project))?,
        ),
    };
    let lines_of = |series: &Series| names.of(&boxes, *series).len();
    let total = series.iter().map(lines_of).sum::<usize>();
    // Only the series whose lines can be drawn are read.
    let mut lines_before = 0;
    let read = series
        .iter()
        .take_while(|series| {
            let drawn = lines_before < MAX_CONSOLE_PLOT_LINES;
            lines_before += lines_of(series);
            drawn
        })
        .count();
    let read_series = series.get(..read).unwrap_or_default();
    let rows = if read_series.is_empty() {
        Vec::new()
    } else {
        plot_rows(conn, &boxes, read_series, names.named(), read_window)
            .map_err(resource_not_found_err!(Metric, &project))?
    };

    let plot = Plot::new(&boxes, read_series, &names, rows);
    let models =
        plot_models(conn, &plot.model_ids()).map_err(resource_not_found_err!(Model, &project))?;
    Ok(plot.into_json(&boxes, &models, window, total))
}

#[derive(Clone, Copy)]
struct Window {
    start_time: DateTime,
    end_time: DateTime,
}

/// The metric names each series draws.
enum Names {
    /// Named by the query: every series draws each of them.
    Every(Vec<MetricName>),
    /// Not named: each series draws the names it reported in the window, in
    /// name order.
    Reported(HashMap<SeriesKey, Vec<MetricName>>),
}

impl Names {
    fn of(&self, boxes: &Boxes, series: Series) -> &[MetricName] {
        match self {
            Self::Every(names) => names,
            Self::Reported(names) => boxes
                .key(series)
                .and_then(|key| names.get(&key))
                .map_or(&[], Vec::as_slice),
        }
    }

    fn named(&self) -> Option<&[MetricName]> {
        match self {
            Self::Every(names) => Some(names),
            Self::Reported(_) => None,
        }
    }
}

/// Every name each series of the query reported in the window.
fn reported_names(
    conn: &mut DbConnection,
    boxes: &Boxes,
    series: &[Series],
    window: Window,
) -> diesel::QueryResult<HashMap<SeriesKey, Vec<MetricName>>> {
    if series.is_empty() {
        return Ok(HashMap::new());
    }
    let Window {
        start_time,
        end_time,
    } = window;
    let rows = schema::report::table
        .inner_join(
            schema::head_version::table
                .on(schema::head_version::version_id.eq(schema::report::version_id)),
        )
        .inner_join(
            schema::report_benchmark::table
                .on(schema::report_benchmark::report_id.eq(schema::report::id)),
        )
        .inner_join(
            schema::metric::table
                .on(schema::metric::report_benchmark_id.eq(schema::report_benchmark::id)),
        )
        .filter(schema::head_version::head_id.eq_any(unique(
            boxes.branches.iter().map(|branch| branch.head_id),
        )))
        .filter(schema::report::testbed_id.eq_any(unique(
            boxes.testbeds.iter().map(|testbed| testbed.testbed.0),
        )))
        .filter(schema::report_benchmark::variant_id.eq_any(unique(
            series
                .iter()
                .filter_map(|series| boxes.variant(*series).map(|variant| variant.0)),
        )))
        // SQLite would otherwise drive the read off `index_metric_measure`,
        // which spans every report the measure was ever in.
        .filter((schema::metric::measure_id + 0).eq_any(unique(
            boxes.measures.iter().map(|measure| measure.0),
        )))
        .filter(schema::report::start_time.ge(start_time))
        .filter(schema::report::end_time.ge(start_time))
        .filter(schema::report::end_time.le(end_time))
        .select((
            // Selected bare, the head moves SQLite's head check after the metric
            // seeks, so every other branch's runs on the testbed are read too.
            schema::head_version::head_id + 0,
            schema::report::testbed_id,
            schema::report::spec_id,
            schema::report_benchmark::variant_id,
            schema::metric::measure_id,
            schema::metric::name,
        ))
        .distinct()
        .load::<(HeadId, TestbedId, Option<SpecId>, VariantId, MeasureId, MetricName)>(conn)?;
    let mut names: HashMap<SeriesKey, BTreeSet<MetricName>> = HashMap::new();
    for (head_id, testbed_id, spec_id, variant_id, measure_id, name) in rows {
        for testbed in boxes.testbeds.iter().filter(|testbed| {
            testbed.testbed.0 == testbed_id
                && testbed.spec_id.is_none_or(|spec| spec_id == Some(spec))
        }) {
            names
                .entry((head_id, testbed_id, testbed.spec_id, variant_id, measure_id))
                .or_default()
                .insert(name.clone());
        }
    }
    Ok(names
        .into_iter()
        .map(|(key, names)| (key, names.into_iter().collect()))
        .collect())
}

/// The first of each item with the same key, in order.
fn first_of_each<T, K: Eq + Hash, F: Fn(&T) -> K>(items: Vec<T>, key: F) -> Vec<T> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(key(item)))
        .collect()
}

/// The boxes of the query, resolved in their order. A value that does not
/// resolve is left out, so the rest of the query still draws.
struct Boxes {
    branches: Vec<BranchHead>,
    testbeds: Vec<TestbedSpec>,
    benchmarks: Vec<(BenchmarkRow, Vec<VariantRow>)>,
    measures: Vec<MeasureRow>,
}

type BranchRow = (BranchId, BranchUuid, BranchName, BranchSlug);
type TestbedRow = (TestbedId, TestbedUuid, ResourceName, TestbedSlug);

struct BranchHead {
    branch: BranchRow,
    head_id: HeadId,
    head: HeadUuid,
}

struct TestbedSpec {
    testbed: TestbedRow,
    spec_id: Option<SpecId>,
    #[cfg(feature = "plus")]
    spec: Option<SpecUuid>,
}

/// One branch, testbed, variant, and measure: a line for each metric name.
#[derive(Clone, Copy)]
struct Series {
    branch: usize,
    testbed: usize,
    benchmark: usize,
    variant: usize,
    measure: usize,
}

impl Boxes {
    /// Every series in the order the boxes name them.
    fn series(&self) -> Vec<Series> {
        let mut series = Vec::new();
        for branch in 0..self.branches.len() {
            for testbed in 0..self.testbeds.len() {
                for (benchmark, (_, variants)) in self.benchmarks.iter().enumerate() {
                    for variant in 0..variants.len() {
                        for measure in 0..self.measures.len() {
                            series.push(Series {
                                branch,
                                testbed,
                                benchmark,
                                variant,
                                measure,
                            });
                        }
                    }
                }
            }
        }
        series
    }

    fn head_id(&self, series: Series) -> Option<HeadId> {
        self.branches
            .get(series.branch)
            .map(|branch| branch.head_id)
    }

    fn testbed(&self, series: Series) -> Option<&TestbedSpec> {
        self.testbeds.get(series.testbed)
    }

    fn variant(&self, series: Series) -> Option<&VariantRow> {
        self.benchmarks
            .get(series.benchmark)
            .and_then(|(_, variants)| variants.get(series.variant))
    }

    fn measure(&self, series: Series) -> Option<&MeasureRow> {
        self.measures.get(series.measure)
    }

    fn key(&self, series: Series) -> Option<SeriesKey> {
        let testbed = self.testbed(series)?;
        Some((
            self.head_id(series)?,
            testbed.testbed.0,
            testbed.spec_id,
            self.variant(series)?.0,
            self.measure(series)?.0,
        ))
    }
}

/// A series by its head, testbed, the spec its testbed entry names, variant, and
/// measure.
type SeriesKey = (HeadId, TestbedId, Option<SpecId>, VariantId, MeasureId);

fn branch_heads(
    conn: &mut DbConnection,
    project: &QueryProject,
    branches: &[BranchUuid],
    heads: &[Option<HeadUuid>],
) -> diesel::QueryResult<Vec<BranchHead>> {
    let rows = schema::branch::table
        .left_join(schema::head::table.on(schema::head::id.nullable().eq(schema::branch::head_id)))
        .filter(schema::branch::project_id.eq(project.id))
        .filter(schema::branch::uuid.eq_any(branches))
        .select((
            (
                schema::branch::id,
                schema::branch::uuid,
                schema::branch::name,
                schema::branch::slug,
            ),
            (schema::head::id, schema::head::uuid).nullable(),
        ))
        .load::<(BranchRow, Option<(HeadId, HeadUuid)>)>(conn)?;
    let named_heads = heads.iter().flatten().copied().collect::<Vec<_>>();
    let head_rows = if named_heads.is_empty() {
        Vec::new()
    } else {
        schema::head::table
            .filter(schema::head::uuid.eq_any(named_heads))
            .select((
                schema::head::uuid,
                schema::head::id,
                schema::head::branch_id,
            ))
            .load::<(HeadUuid, HeadId, BranchId)>(conn)?
    };
    Ok(branches
        .iter()
        .zip(heads)
        .filter_map(|(branch_uuid, head_uuid)| {
            let (branch, current) = rows
                .iter()
                .find(|((_, uuid, _, _), _)| uuid == branch_uuid)?;
            // A head names one branch, so a head on another branch draws nothing.
            let (head_id, head) = match head_uuid {
                Some(head_uuid) => head_rows
                    .iter()
                    .find(|(uuid, _, branch_id)| uuid == head_uuid && *branch_id == branch.0)
                    .map(|(uuid, head_id, _)| (*head_id, *uuid))?,
                None => (*current)?,
            };
            Some(BranchHead {
                branch: branch.clone(),
                head_id,
                head,
            })
        })
        .collect::<Vec<_>>())
    .map(|branches| first_of_each(branches, |branch| (branch.branch.0, branch.head_id)))
}

fn testbed_specs(
    conn: &mut DbConnection,
    project: &QueryProject,
    testbeds: &[TestbedUuid],
    #[cfg(feature = "plus")] specs: &[Option<SpecUuid>],
) -> diesel::QueryResult<Vec<TestbedSpec>> {
    let rows = schema::testbed::table
        .filter(schema::testbed::project_id.eq(project.id))
        .filter(schema::testbed::uuid.eq_any(testbeds))
        .select((
            schema::testbed::id,
            schema::testbed::uuid,
            schema::testbed::name,
            schema::testbed::slug,
        ))
        .load::<TestbedRow>(conn)?;
    // Each testbed's spec: `None` when the spec named does not exist, which
    // draws nothing for that testbed.
    #[cfg(feature = "plus")]
    let specs = {
        let named = specs.iter().flatten().copied().collect::<Vec<_>>();
        let spec_rows = if named.is_empty() {
            Vec::new()
        } else {
            schema::spec::table
                .filter(schema::spec::uuid.eq_any(named))
                .select((schema::spec::uuid, schema::spec::id))
                .load::<(SpecUuid, SpecId)>(conn)?
        };
        specs
            .iter()
            .map(|spec| match spec {
                Some(spec_uuid) => spec_rows
                    .iter()
                    .find(|(uuid, _)| uuid == spec_uuid)
                    .map(|(uuid, spec_id)| (Some(*spec_id), Some(*uuid))),
                None => Some((None, None)),
            })
            .collect::<Vec<_>>()
    };
    #[cfg(not(feature = "plus"))]
    let specs = vec![Some(None); testbeds.len()];
    Ok(testbeds
        .iter()
        .zip(specs)
        .filter_map(|(testbed_uuid, spec)| {
            let testbed = rows.iter().find(|(_, uuid, _, _)| uuid == testbed_uuid)?;
            Some(testbed_spec(testbed.clone(), spec?))
        })
        .collect::<Vec<_>>())
    .map(|testbeds| first_of_each(testbeds, |testbed| (testbed.testbed.0, testbed.spec_id)))
}

#[cfg(feature = "plus")]
fn testbed_spec(
    testbed: TestbedRow,
    (spec_id, spec): (Option<SpecId>, Option<SpecUuid>),
) -> TestbedSpec {
    TestbedSpec {
        testbed,
        spec_id,
        spec,
    }
}

#[cfg(not(feature = "plus"))]
fn testbed_spec(testbed: TestbedRow, spec_id: Option<SpecId>) -> TestbedSpec {
    TestbedSpec { testbed, spec_id }
}

/// Each benchmark with the variants the parameters filter matches, in creation
/// order. The filter is resolved in memory, so what reaches SQL is row
/// identifiers and never a JSON predicate. A variant that never reported, such as
/// the empty one every benchmark is born with, has nothing to draw and is left out.
fn benchmark_variants(
    conn: &mut DbConnection,
    project: &QueryProject,
    benchmarks: &[BenchmarkUuid],
    parameters: Option<&[ParameterSet]>,
) -> diesel::QueryResult<Vec<(BenchmarkRow, Vec<VariantRow>)>> {
    // The project is checked on the rows: filtered in SQL, it would draw SQLite
    // onto the project's index and through every benchmark the project has.
    let rows = schema::variant::table
        .inner_join(
            schema::benchmark::table.on(schema::benchmark::id.eq(schema::variant::benchmark_id)),
        )
        .filter(schema::benchmark::uuid.eq_any(benchmarks))
        .filter(diesel::dsl::exists(schema::report_benchmark::table.filter(
            schema::report_benchmark::variant_id.eq(schema::variant::id),
        )))
        .order(schema::variant::id)
        .select((
            schema::benchmark::project_id,
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
        ))
        .load::<(ProjectId, BenchmarkRow, VariantRow)>(conn)?
        .into_iter()
        .filter_map(|(project_id, benchmark, variant)| {
            (project_id == project.id).then_some((benchmark, variant))
        })
        .collect::<Vec<_>>();
    Ok(benchmarks
        .iter()
        .filter_map(|benchmark_uuid| {
            let benchmark = rows
                .iter()
                .map(|(benchmark, _)| benchmark)
                .find(|(_, uuid, _, _)| uuid == benchmark_uuid)?
                .clone();
            let variants = rows
                .iter()
                .filter(|(variant_benchmark, (_, _, set))| {
                    variant_benchmark.0 == benchmark.0
                        && parameters.is_none_or(|parameters| {
                            parameters.iter().any(|filter| filter.is_subset_of(set))
                        })
                })
                .map(|(_, variant)| variant.clone())
                .collect::<Vec<_>>();
            Some((benchmark, variants))
        })
        .collect::<Vec<_>>())
    .map(|benchmarks| first_of_each(benchmarks, |(benchmark, _)| benchmark.0))
}

fn measure_rows(
    conn: &mut DbConnection,
    project: &QueryProject,
    measures: &[MeasureUuid],
) -> diesel::QueryResult<Vec<MeasureRow>> {
    let rows = schema::measure::table
        .filter(schema::measure::project_id.eq(project.id))
        .filter(schema::measure::uuid.eq_any(measures))
        .select((
            schema::measure::id,
            schema::measure::uuid,
            schema::measure::name,
            schema::measure::slug,
            schema::measure::units,
        ))
        .load::<MeasureRow>(conn)?;
    Ok(measures
        .iter()
        .filter_map(|measure_uuid| {
            rows.iter()
                .find(|(_, uuid, ..)| uuid == measure_uuid)
                .cloned()
        })
        .collect::<Vec<_>>())
    .map(|measures| first_of_each(measures, |measure| measure.0))
}

type PlotRowTuple = (
    (
        ReportId,
        ReportUuid,
        DateTime,
        DateTime,
        DateTime,
        Option<SpecId>,
        VersionNumber,
        Option<GitHash>,
    ),
    (HeadId, TestbedId, Iteration, ReportBenchmarkUuid),
    (VariantId, MeasureId, MetricName, f64),
    Option<(ThresholdId, ModelId, Option<f64>, Option<f64>, Option<f64>)>,
    Option<AlertRow>,
);

/// One metric of a read series, once per head it is under and per threshold
/// that checked it.
#[derive(Clone)]
struct PlotRow {
    report: PointReport,
    spec_id: Option<SpecId>,
    head_id: HeadId,
    testbed_id: TestbedId,
    iteration: Iteration,
    variant_id: VariantId,
    measure_id: MeasureId,
    metric: MetricName,
    value: f64,
    check: Option<PointCheck>,
}

/// What one threshold computed for one point. The model is read once per plot
/// rather than on every row.
#[derive(Clone, Copy)]
struct PointCheck {
    threshold_id: ThresholdId,
    model_id: ModelId,
    limits: Limits,
    alert: Option<JsonConsoleAlert>,
}

impl PointCheck {
    /// Of the thresholds that checked one point, the line follows the one that
    /// alerted, and otherwise the oldest.
    fn choose(checks: Vec<Self>) -> Option<Self> {
        checks
            .into_iter()
            .min_by_key(|check| (check.alert.is_none(), check.threshold_id))
    }
}

/// One statement for every read series: the window's reports on their heads and
/// testbeds, crossed with their variants and measures.
fn plot_rows(
    conn: &mut DbConnection,
    boxes: &Boxes,
    series: &[Series],
    metrics: Option<&[MetricName]>,
    window: Window,
) -> diesel::QueryResult<Vec<PlotRow>> {
    let head_ids = unique(series.iter().filter_map(|series| boxes.head_id(*series)));
    let testbed_ids = unique(
        series
            .iter()
            .filter_map(|series| boxes.testbed(*series).map(|testbed| testbed.testbed.0)),
    );
    let variant_ids = unique(
        series
            .iter()
            .filter_map(|series| boxes.variant(*series).map(|variant| variant.0)),
    );
    let measure_ids = unique(
        series
            .iter()
            .filter_map(|series| boxes.measure(*series).map(|measure| measure.0)),
    );
    let Window {
        start_time,
        end_time,
    } = window;
    let mut query = schema::report_benchmark::table
        .inner_join(schema::report::table.on(schema::report::id.eq(schema::report_benchmark::report_id)))
        .inner_join(
            schema::head_version::table
                .on(schema::head_version::version_id.eq(schema::report::version_id)),
        )
        .inner_join(schema::version::table.on(schema::version::id.eq(schema::report::version_id)))
        .inner_join(
            schema::metric::table
                .on(schema::metric::report_benchmark_id.eq(schema::report_benchmark::id)),
        )
        // Flat, with explicit `ON` clauses, so SQLite seeks each outer join
        // instead of scanning the boundary table.
        .left_join(schema::boundary::table.on(schema::boundary::metric_id.eq(schema::metric::id)))
        .left_join(schema::alert::table.on(schema::alert::boundary_id.eq(schema::boundary::id)))
        .filter(schema::head_version::head_id.eq_any(head_ids))
        .filter(schema::report::testbed_id.eq_any(testbed_ids))
        .filter(schema::report_benchmark::variant_id.eq_any(variant_ids))
        // SQLite would otherwise drive the read off `index_metric_measure`,
        // which spans every report the measure was ever in.
        .filter((schema::metric::measure_id + 0).eq_any(measure_ids))
        .filter(schema::report::start_time.ge(start_time))
        // Implied by the start, and what bounds the index range from below.
        .filter(schema::report::end_time.ge(start_time))
        .filter(schema::report::end_time.le(end_time))
        .select((
            (
                schema::report::id,
                schema::report::uuid,
                schema::report::start_time,
                schema::report::end_time,
                schema::report::created,
                schema::report::spec_id,
                schema::version::number,
                schema::version::hash,
            ),
            (
                schema::head_version::head_id,
                schema::report::testbed_id,
                schema::report_benchmark::iteration,
                // Outside the covering report index, so SQLite seeks each variant
                // of a report rather than scanning all of the report's rows.
                schema::report_benchmark::uuid,
            ),
            (
                schema::report_benchmark::variant_id,
                schema::metric::measure_id,
                schema::metric::name,
                schema::metric::value,
            ),
            (
                schema::boundary::threshold_id,
                schema::boundary::model_id,
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
        .into_boxed();
    if let Some(metrics) = metrics {
        query = query.filter(schema::metric::name.eq_any(metrics.to_vec()));
    }
    Ok(query
        .load::<PlotRowTuple>(conn)?
        .into_iter()
        .map(plot_row)
        .collect())
}

fn plot_row(row: PlotRowTuple) -> PlotRow {
    let (
        (id, uuid, start_time, end_time, created, spec_id, version, hash),
        (head_id, testbed_id, iteration, _),
        (variant_id, measure_id, metric, value),
        boundary,
        alert,
    ) = row;
    PlotRow {
        report: PointReport {
            id,
            uuid,
            start_time,
            end_time,
            created,
            version,
            hash,
        },
        spec_id,
        head_id,
        testbed_id,
        iteration,
        variant_id,
        measure_id,
        metric,
        value,
        check: boundary.map(
            |(threshold_id, model_id, baseline, lower_limit, upper_limit)| PointCheck {
                threshold_id,
                model_id,
                limits: Limits {
                    baseline,
                    lower_limit,
                    upper_limit,
                },
                alert: alert_json(alert),
            },
        ),
    }
}

/// The models the drawn lines follow, each with its threshold.
fn plot_models(
    conn: &mut DbConnection,
    model_ids: &[ModelId],
) -> diesel::QueryResult<HashMap<ModelId, JsonConsoleModel>> {
    if model_ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(schema::model::table
        .inner_join(
            schema::threshold::table.on(schema::threshold::id.eq(schema::model::threshold_id)),
        )
        .filter(schema::model::id.eq_any(model_ids))
        .select((
            schema::model::id,
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
        ))
        .load::<(ModelId, ThresholdUuid, ModelRow)>(conn)?
        .into_iter()
        .map(|(model_id, threshold, model)| (model_id, model_json(threshold, model)))
        .collect())
}

/// Each row with the drawn line it belongs to: every line on its head, testbed,
/// variant, measure, and name, where the line's spec, if it names one, matches.
fn drawn_rows(
    boxes: &Boxes,
    lines: &[(Series, MetricName)],
    rows: Vec<PlotRow>,
) -> Vec<(usize, PlotRow)> {
    let mut line_index: HashMap<_, Vec<usize>> = HashMap::new();
    for (position, (series, name)) in lines.iter().enumerate() {
        let (Some(head_id), Some(testbed), Some(variant), Some(measure)) = (
            boxes.head_id(*series),
            boxes.testbed(*series),
            boxes.variant(*series),
            boxes.measure(*series),
        ) else {
            continue;
        };
        line_index
            .entry((
                head_id,
                testbed.testbed.0,
                variant.0,
                measure.0,
                name.clone(),
            ))
            .or_default()
            .push(position);
    }
    rows.into_iter()
        .flat_map(|row| {
            let key = (
                row.head_id,
                row.testbed_id,
                row.variant_id,
                row.measure_id,
                row.metric.clone(),
            );
            let spec_id = row.spec_id;
            line_index
                .get(&key)
                .into_iter()
                .flatten()
                .copied()
                .filter(move |position| {
                    lines
                        .get(*position)
                        .and_then(|(series, _)| boxes.testbed(*series))
                        .is_some_and(|testbed| {
                            testbed
                                .spec_id
                                .is_none_or(|line_spec| spec_id == Some(line_spec))
                        })
                })
                .map(move |position| (position, row.clone()))
        })
        .collect()
}

/// The checks of a line's latest checked point, with where that point falls.
type LatestChecks = Option<((ReportOrder, u32), Vec<PointCheck>)>;

/// A line follows the threshold that checked its latest checked point.
fn followed_checks(rows: &[(usize, PlotRow)], lines: usize) -> Vec<Option<PointCheck>> {
    let mut latest: Vec<LatestChecks> = vec![None; lines];
    for (position, row) in rows {
        let (Some(check), Some(slot)) = (row.check, latest.get_mut(*position)) else {
            continue;
        };
        let order = (row.report.order(), row.iteration.0);
        match slot.as_ref().map(|(kept, _)| order.cmp(kept)) {
            Some(Ordering::Less) => {},
            Some(Ordering::Equal) => {
                if let Some((_, checks)) = slot {
                    checks.push(check);
                }
            },
            Some(Ordering::Greater) | None => *slot = Some((order, vec![check])),
        }
    }
    latest
        .into_iter()
        .map(|latest| latest.and_then(|(_, checks)| PointCheck::choose(checks)))
        .collect()
}

/// The drawn lines with their columns.
struct Plot {
    lines: Vec<(Series, MetricName)>,
    checks: Vec<Option<PointCheck>>,
    points: super::Points,
    series: Vec<SeriesBuilder>,
}

impl Plot {
    fn new(boxes: &Boxes, series: &[Series], names: &Names, rows: Vec<PlotRow>) -> Self {
        let lines = series
            .iter()
            .flat_map(|series| {
                names
                    .of(boxes, *series)
                    .iter()
                    .map(|name| (*series, name.clone()))
            })
            .take(MAX_CONSOLE_PLOT_LINES)
            .collect::<Vec<_>>();
        let rows = drawn_rows(boxes, &lines, rows);

        let mut points = PointsBuilder::default();
        for (_, row) in &rows {
            points.add(&row.report, row.iteration);
        }
        let points = points.build();
        let checks = followed_checks(&rows, lines.len());

        let mut series = lines
            .iter()
            .map(|_| SeriesBuilder::new(points.json.x.len()))
            .collect::<Vec<_>>();
        for (position, row) in rows {
            let (Some(point), Some(builder)) = (
                points.index.get(&(row.report.id, row.iteration)).copied(),
                series.get_mut(position),
            ) else {
                continue;
            };
            builder.value(point, row.value);
            let followed = checks
                .get(position)
                .and_then(Option::as_ref)
                .map(|check| check.threshold_id);
            if let Some(check) = row.check
                && followed == Some(check.threshold_id)
            {
                builder.limits(point, check.limits, check.alert);
            }
        }

        Self {
            lines,
            checks,
            points,
            series,
        }
    }

    fn model_ids(&self) -> Vec<ModelId> {
        unique(self.checks.iter().flatten().map(|check| check.model_id))
    }

    fn into_json(
        self,
        boxes: &Boxes,
        models: &HashMap<ModelId, JsonConsoleModel>,
        window: JsonConsoleWindow,
        total: usize,
    ) -> JsonConsolePerf {
        let Self {
            lines,
            checks,
            points,
            series,
        } = self;
        let mut tables = Tables::default();
        let mut branches = Vec::new();
        let mut branch_index = HashMap::new();
        let mut testbeds = Vec::new();
        let mut testbed_index = HashMap::new();
        let lines = lines
            .into_iter()
            .zip(checks)
            .zip(series)
            .filter_map(|(((line, metric), check), series)| {
                let branch_head = boxes.branches.get(line.branch)?;
                let testbed_spec = boxes.testbed(line)?;
                let (benchmark, _) = boxes.benchmarks.get(line.benchmark)?;
                let variant = boxes.variant(line)?;
                let measure = boxes.measure(line)?;
                let branch = *branch_index
                    .entry((branch_head.branch.0, branch_head.head_id))
                    .or_insert_with(|| {
                        let (_, uuid, name, slug) = branch_head.branch.clone();
                        branches.push(JsonConsoleBranch {
                            uuid,
                            name,
                            slug,
                            head: branch_head.head,
                        });
                        super::to_index(branches.len() - 1)
                    });
                let testbed = *testbed_index
                    .entry((testbed_spec.testbed.0, testbed_spec.spec_id))
                    .or_insert_with(|| {
                        let (_, uuid, name, slug) = testbed_spec.testbed.clone();
                        testbeds.push(JsonConsoleTestbed {
                            uuid,
                            name,
                            slug,
                            #[cfg(feature = "plus")]
                            spec: testbed_spec.spec,
                        });
                        super::to_index(testbeds.len() - 1)
                    });
                let benchmark = tables.benchmark(benchmark);
                let variant = tables.variant(variant, benchmark);
                let measure = tables.measure(measure);
                let model = check
                    .and_then(|check| models.get(&check.model_id))
                    .map(|model| tables.model_json(*model));
                Some(JsonConsolePerfLine {
                    branch,
                    testbed,
                    benchmark,
                    variant,
                    measure,
                    metric,
                    model,
                    series: series.build(),
                })
            })
            .collect();
        let Tables {
            benchmarks,
            variants,
            measures,
            models,
            ..
        } = tables;
        JsonConsolePerf {
            window,
            total: u32::try_from(total).unwrap_or(u32::MAX),
            lines,
            points: points.json,
            reports: points.reports,
            branches,
            testbeds,
            benchmarks,
            variants,
            measures,
            models,
        }
    }
}
