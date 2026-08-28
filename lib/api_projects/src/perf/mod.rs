use std::collections::{BTreeMap, HashMap};

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
#[cfg(feature = "plus")]
use bencher_json::SpecUuid;
use bencher_json::{
    BenchmarkUuid, BranchUuid, DateTime, GitHash, HeadUuid, JsonPerf, JsonPerfQuery, MeasureUuid,
    MetricName, MetricUuid, ParameterSet, ProjectResourceId, ReportUuid, TestbedUuid,
    project::{
        alert::JsonPerfAlert,
        boundary::JsonBoundary,
        head::{JsonVersion, VersionNumber},
        metric::JsonMetricTriple,
        perf::{
            JsonMetricEntry, JsonPerfBoundary, JsonPerfLine, JsonPerfMetrics, JsonPerfQueryParams,
        },
        report::Iteration,
        threshold::JsonThresholdModel,
    },
};
#[cfg(feature = "plus")]
use bencher_schema::model::spec::QuerySpec;
use bencher_schema::model::spec::SpecId;
use bencher_schema::{
    actor_conn,
    context::{ApiContext, DbConnection},
    error::{bad_request_error, resource_not_found_err, with_auth_hint},
    model::{
        project::{
            ProjectId, QueryProject,
            benchmark::QueryBenchmark,
            branch::{QueryBranch, head::QueryHead},
            measure::QueryMeasure,
            metric::QueryMetric,
            parameter::{ParameterId, QueryParameter},
            report::report_benchmark::ReportBenchmarkId,
            testbed::QueryTestbed,
            threshold::{
                QueryThreshold, alert::QueryAlert, boundary::QueryBoundary, model::QueryModel,
            },
        },
        user::actor::{ApiActor, PubProjectBearerToken},
    },
    schema,
};
use diesel::{
    ExpressionMethods as _, JoinOnDsl as _, NullableExpressionMethods as _, QueryDsl as _,
    RunQueryDsl as _, SelectableHelper as _, query_dsl::LoadQuery,
};
use dropshot::{HttpError, Path, Query, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

pub mod img;

const MAX_PERMUTATIONS: usize = 255;

#[derive(Deserialize, JsonSchema)]
pub struct ProjPerfParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/perf",
    tags = ["projects", "perf"]
}]
pub async fn proj_perf_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjPerfParams>,
    _query_params: Query<JsonPerfQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// Query project performance metrics
///
/// Query the performance metrics for a project.
/// The query results are every permutation of each branch, testbed, benchmark, and measure.
/// There is a limit of 255 permutations for a single request.
/// Therefore, only the first 255 permutations are returned.
/// Each permutation returns one result per variant of its benchmark,
/// narrowed by the `parameters` filter when one is given.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/perf",
    tags = ["projects", "perf"]
}]
pub async fn proj_perf_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjPerfParams>,
    query_params: Query<JsonPerfQueryParams>,
) -> Result<ResponseOk<JsonPerf>, HttpError> {
    // Second round of marshaling
    let json_perf_query = query_params
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
    let json = get_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        json_perf_query,
        &api_actor,
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::response_ok(json, api_actor.is_auth()))
}

pub async fn get_inner(
    log: &slog::Logger,
    context: &ApiContext,
    path_params: ProjPerfParams,
    json_perf_query: JsonPerfQuery,
    api_actor: &ApiActor,
) -> Result<JsonPerf, HttpError> {
    let project = QueryProject::is_allowed_actor_pub(
        actor_conn!(context, api_actor),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        api_actor,
    )?;

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
    } = json_perf_query;

    let times = Times {
        start_time,
        end_time,
    };

    let results = perf_results(
        log,
        context,
        api_actor,
        &project,
        &branches,
        &heads,
        &testbeds,
        #[cfg(feature = "plus")]
        &specs,
        &benchmarks,
        &parameters,
        &measures,
        times,
    )
    .await?;

    Ok(JsonPerf {
        project: project.into_json(actor_conn!(context, api_actor))?,
        start_time,
        end_time,
        results,
    })
}

#[derive(Clone, Copy)]
struct Times {
    start_time: Option<DateTime>,
    end_time: Option<DateTime>,
}

/// The variants of one benchmark that a perf query plots.
///
/// The parameters filter is resolved here, in memory, so what reaches SQL is a
/// list of row identifiers and never a JSON predicate.
struct BenchmarkVariants {
    benchmark: QueryBenchmark,
    /// Keyed by row identifier and therefore in creation order, so the empty set
    /// every benchmark is born with comes first.
    variants: BTreeMap<ParameterId, QueryParameter>,
    filtered: bool,
}

impl BenchmarkVariants {
    fn parameter_ids(&self) -> Option<Vec<ParameterId>> {
        self.filtered
            .then(|| self.variants.keys().copied().collect())
    }
}

fn benchmark_variants(
    conn: &mut DbConnection,
    project: &QueryProject,
    benchmark_uuid: BenchmarkUuid,
    parameters: &[ParameterSet],
) -> Result<BenchmarkVariants, HttpError> {
    let benchmark = QueryBenchmark::from_uuid(conn, project.id, benchmark_uuid)?;
    let variants = schema::parameter::table
        .filter(schema::parameter::benchmark_id.eq(benchmark.id))
        .order(schema::parameter::id)
        .select(QueryParameter::as_select())
        .load::<QueryParameter>(conn)
        .map_err(resource_not_found_err!(
            Parameter,
            (project, benchmark_uuid)
        ))?
        .into_iter()
        .filter(|variant| {
            parameters.is_empty()
                || parameters
                    .iter()
                    .any(|filter| filter.is_subset_of(&variant.set))
        })
        .map(|variant| (variant.id, variant))
        .collect();

    Ok(BenchmarkVariants {
        benchmark,
        variants,
        filtered: !parameters.is_empty(),
    })
}

#[cfg(feature = "plus")]
enum QueriedSpec {
    Spec(Option<SpecId>),
    Missing,
}

/// A spec that does not exist skips its permutation rather than failing the query.
#[cfg(feature = "plus")]
fn queried_spec(
    conn: &mut DbConnection,
    log: &slog::Logger,
    spec_uuid: Option<SpecUuid>,
) -> QueriedSpec {
    let Some(spec_uuid) = spec_uuid else {
        return QueriedSpec::Spec(None);
    };
    match QuerySpec::get_id(conn, spec_uuid) {
        Ok(spec_id) => QueriedSpec::Spec(Some(spec_id)),
        Err(e) => {
            slog::info!(log, "Skipping perf query for nonexistent spec UUID: {spec_uuid}"; "error" => %e);
            QueriedSpec::Missing
        },
    }
}

/// A benchmark that does not exist, and a benchmark whose every variant the filter
/// excludes, both return nothing rather than an error, so the other benchmarks of
/// the same query still return their lines.
fn queried_variants<'v>(
    conn: &mut DbConnection,
    log: &slog::Logger,
    variants_cache: &'v mut HashMap<BenchmarkUuid, Option<BenchmarkVariants>>,
    project: &QueryProject,
    benchmark_uuid: BenchmarkUuid,
    parameters: &[ParameterSet],
) -> Option<&'v BenchmarkVariants> {
    variants_cache
        .entry(benchmark_uuid)
        .or_insert_with(
            || match benchmark_variants(conn, project, benchmark_uuid, parameters) {
                Ok(variants) => Some(variants),
                Err(e) => {
                    slog::info!(log, "Skipping perf query for nonexistent benchmark UUID: {benchmark_uuid}"; "error" => %e);
                    None
                },
            },
        )
        .as_ref()
        .filter(|variants| !variants.variants.is_empty())
}

#[expect(
    clippy::too_many_arguments,
    reason = "perf query requires all filter dimensions"
)]
async fn perf_results(
    log: &slog::Logger,
    context: &ApiContext,
    api_actor: &ApiActor,
    project: &QueryProject,
    branches: &[BranchUuid],
    heads: &[Option<HeadUuid>],
    testbeds: &[TestbedUuid],
    #[cfg(feature = "plus")] specs: &[Option<SpecUuid>],
    benchmarks: &[BenchmarkUuid],
    parameters: &[ParameterSet],
    measures: &[MeasureUuid],
    times: Times,
) -> Result<Vec<JsonPerfLine>, HttpError> {
    let permutations = branches.len() * testbeds.len() * benchmarks.len() * measures.len();
    let gt_max_permutations = permutations > MAX_PERMUTATIONS;
    let mut results = Vec::with_capacity(permutations.min(MAX_PERMUTATIONS));
    let mut variants_cache: HashMap<BenchmarkUuid, Option<BenchmarkVariants>> = HashMap::new();
    // It is okay to use `zip` because `JsonPerfQuery` guarantees that the lengths are the same.
    for (branch_index, (branch_uuid, head_uuid)) in branches.iter().zip(heads.iter()).enumerate() {
        for (testbed_index, testbed_uuid) in testbeds.iter().enumerate() {
            #[cfg(feature = "plus")]
            let QueriedSpec::Spec(spec_id) = queried_spec(
                actor_conn!(context, api_actor),
                log,
                specs.get(testbed_index).copied().flatten(),
            ) else {
                continue;
            };
            #[cfg(not(feature = "plus"))]
            let spec_id: Option<SpecId> = None;

            for (benchmark_index, benchmark_uuid) in benchmarks.iter().enumerate() {
                let Some(variants) = queried_variants(
                    actor_conn!(context, api_actor),
                    log,
                    &mut variants_cache,
                    project,
                    *benchmark_uuid,
                    parameters,
                ) else {
                    continue;
                };

                for (measure_index, measure_uuid) in measures.iter().enumerate() {
                    if gt_max_permutations
                        && (branch_index + 1)
                            * (testbed_index + 1)
                            * (benchmark_index + 1)
                            * (measure_index + 1)
                            > MAX_PERMUTATIONS
                    {
                        return Ok(results);
                    }

                    let pq = perf_query(
                        project.id,
                        *branch_uuid,
                        *head_uuid,
                        *testbed_uuid,
                        spec_id,
                        *benchmark_uuid,
                        variants.parameter_ids(),
                        *measure_uuid,
                        times,
                    )
                    .load::<PerfQuery>(actor_conn!(context, api_actor))
                    .map_err(resource_not_found_err!(
                        Metric,
                        (
                            project,
                            branch_uuid,
                            testbed_uuid,
                            benchmark_uuid,
                            measure_uuid
                        )
                    ))?;

                    results.extend(into_perf_lines(
                        actor_conn!(context, api_actor),
                        project,
                        variants,
                        spec_id,
                        pq,
                    )?);
                }
            }
        }
    }
    Ok(results)
}

/// One row per metric.
///
/// This reads the `metric` table directly rather than the `metric_boundary` view.
/// All of a variant's metrics for one measure sit together on
/// `index_metric_report_benchmark_measure_name`, so one range read returns every
/// one of them, where the view had to seek each conventional name separately.
#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "perf query requires all filter dimensions"
)]
fn perf_query(
    project_id: ProjectId,
    branch_uuid: BranchUuid,
    head_uuid: Option<HeadUuid>,
    testbed_uuid: TestbedUuid,
    spec_id: Option<SpecId>,
    benchmark_uuid: BenchmarkUuid,
    parameter_ids: Option<Vec<ParameterId>>,
    measure_uuid: MeasureUuid,
    times: Times,
) -> impl LoadQuery<'static, DbConnection, PerfQuery> {
    let mut query = schema::metric::table
        .inner_join(
            schema::report_benchmark::table
                .on(schema::report_benchmark::id.eq(schema::metric::report_benchmark_id)),
        )
        .inner_join(
            schema::benchmark::table
                .on(schema::benchmark::id.eq(schema::report_benchmark::benchmark_id)),
        )
        .inner_join(
            schema::report::table.on(schema::report::id.eq(schema::report_benchmark::report_id)),
        )
        .inner_join(schema::version::table.on(schema::version::id.eq(schema::report::version_id)))
        .inner_join(
            schema::head_version::table.on(schema::head_version::version_id.eq(schema::version::id)),
        )
        .inner_join(schema::head::table.on(schema::head::id.eq(schema::head_version::head_id)))
        .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::head::branch_id)))
        .inner_join(schema::testbed::table.on(schema::testbed::id.eq(schema::report::testbed_id)))
        .inner_join(schema::measure::table.on(schema::measure::id.eq(schema::metric::measure_id)))
        // Keep these joins flat with explicit `ON` clauses. SQLite cannot flatten a
        // compound right operand of an outer join, so nesting the threshold, the
        // model, and the alert inside the boundary join makes it scan the whole
        // boundary table once per request.
        .left_join(schema::boundary::table.on(schema::boundary::metric_id.eq(schema::metric::id)))
        .left_join(
            schema::threshold::table.on(schema::threshold::id.eq(schema::boundary::threshold_id)),
        )
        .left_join(schema::model::table.on(schema::model::id.eq(schema::boundary::model_id)))
        .left_join(schema::alert::table.on(schema::alert::boundary_id.eq(schema::boundary::id)))
        // It is important to filter for the branch through the `head_version` table
        // and NOT on the head in the `report` table.
        // This is because the `head_version` table is the one that is updated
        // when a head is cloned/used as a start point.
        // In contrast, the `report` table is only set to a single head when the report is created.
        // Therefore, querying from the `report` table's `head` would not return results for any other heads.
        .filter(schema::branch::uuid.eq(branch_uuid))
        .filter(schema::testbed::uuid.eq(testbed_uuid))
        .filter(schema::benchmark::uuid.eq(benchmark_uuid))
        .filter(schema::measure::uuid.eq(measure_uuid))
        // Make sure that the project is the same for all dimensions
        .filter(schema::branch::project_id.eq(project_id))
        .filter(schema::testbed::project_id.eq(project_id))
        .filter(schema::benchmark::project_id.eq(project_id))
        .filter(schema::measure::project_id.eq(project_id))
        .into_boxed();

    // Filter for the branch head if it is provided.
    // Otherwise, filter for the current, non-replaced head.
    if let Some(head_uuid) = head_uuid {
        query = query.filter(schema::head::uuid.eq(head_uuid));
    } else {
        query = query.filter(schema::branch::head_id.eq(schema::head::id.nullable()));
    }

    // Filter for the hardware spec if it is provided.
    if let Some(spec_id) = spec_id {
        query = query.filter(schema::report::spec_id.eq(spec_id));
    }

    // Already resolved to row identifiers, so this is an indexed lookup.
    if let Some(parameter_ids) = parameter_ids {
        query = query.filter(schema::report_benchmark::parameter_id.eq_any(parameter_ids));
    }

    let Times {
        start_time,
        end_time,
    } = times;
    if let Some(start_time) = start_time {
        query = query.filter(schema::report::start_time.ge(start_time));
    }
    if let Some(end_time) = end_time {
        query = query.filter(schema::report::end_time.le(end_time));
    }

    query
        // Order by the version number so that the oldest version is first.
        // Because multiple reports can use the same version (via git hash), order by the start time next.
        // Then within a report order by the iteration number.
        // Finally the report benchmark, so one variant's metrics stay together.
        .order((
            schema::version::number,
            schema::report::start_time,
            schema::report_benchmark::iteration,
            schema::report_benchmark::id,
        ))
        .select((
            QueryBranch::as_select(),
            QueryHead::as_select(),
            QueryTestbed::as_select(),
            QueryBenchmark::as_select(),
            QueryMeasure::as_select(),
            schema::report_benchmark::id,
            schema::report_benchmark::parameter_id,
            schema::report::uuid,
            schema::report_benchmark::iteration,
            schema::report::start_time,
            schema::report::end_time,
            schema::version::number,
            schema::version::hash,
            QueryMetric::as_select(),
            (
                (
                    schema::threshold::id,
                    schema::threshold::uuid,
                    schema::threshold::project_id,
                    schema::threshold::measure_id,
                    schema::threshold::branch_id,
                    schema::threshold::testbed_id,
                    schema::threshold::model_id,
                    schema::threshold::created,
                    schema::threshold::modified,
                ),
                (
                    schema::model::id,
                    schema::model::uuid,
                    schema::model::threshold_id,
                    schema::model::test,
                    schema::model::min_sample_size,
                    schema::model::max_sample_size,
                    schema::model::window,
                    schema::model::lower_boundary,
                    schema::model::upper_boundary,
                    schema::model::created,
                    schema::model::replaced,
                ),
                (
                    schema::boundary::id,
                    schema::boundary::uuid,
                    schema::boundary::metric_id,
                    schema::boundary::threshold_id,
                    schema::boundary::model_id,
                    schema::boundary::baseline,
                    schema::boundary::lower_limit,
                    schema::boundary::upper_limit,
                ),
                (
                    schema::alert::id,
                    schema::alert::uuid,
                    schema::alert::boundary_id,
                    schema::alert::boundary_limit,
                    schema::alert::status,
                    schema::alert::modified,
                )
                    .nullable(),
            )
                .nullable(),
        ))
}

/// The threshold, model, boundary, and any alert for one metric.
type PerfBoundary = (
    QueryThreshold,
    QueryModel,
    QueryBoundary,
    Option<QueryAlert>,
);

type PerfQuery = (
    QueryBranch,
    QueryHead,
    QueryTestbed,
    QueryBenchmark,
    QueryMeasure,
    ReportBenchmarkId,
    ParameterId,
    ReportUuid,
    Iteration,
    DateTime,
    DateTime,
    VersionNumber,
    Option<GitHash>,
    QueryMetric,
    Option<PerfBoundary>,
);

struct QueryDimensions {
    branch: QueryBranch,
    head: QueryHead,
    testbed: QueryTestbed,
    measure: QueryMeasure,
}

/// One variant's rows are contiguous in plot order, so each line is built as its
/// rows are read.
fn into_perf_lines(
    conn: &mut DbConnection,
    project: &QueryProject,
    variants: &BenchmarkVariants,
    spec_id: Option<SpecId>,
    rows: Vec<PerfQuery>,
) -> Result<Vec<JsonPerfLine>, HttpError> {
    let mut dimensions: Option<QueryDimensions> = None;
    let mut benchmark: Option<QueryBenchmark> = None;
    let mut lines: BTreeMap<ParameterId, Vec<PendingMetric>> = BTreeMap::new();

    for (
        query_branch,
        query_head,
        query_testbed,
        query_benchmark,
        query_measure,
        report_benchmark_id,
        parameter_id,
        report,
        iteration,
        start_time,
        end_time,
        version_number,
        version_hash,
        query_metric,
        perf_boundary,
    ) in rows
    {
        // Every row of one permutation carries the same dimensions, so the first row
        // is the one that names them.
        if dimensions.is_none() {
            dimensions = Some(QueryDimensions {
                branch: query_branch,
                head: query_head,
                testbed: query_testbed,
                measure: query_measure,
            });
            benchmark = Some(query_benchmark);
        }

        let line = lines.entry(parameter_id).or_default();
        if line
            .last()
            .is_none_or(|pending| pending.report_benchmark_id != report_benchmark_id)
        {
            line.push(PendingMetric {
                report_benchmark_id,
                report,
                iteration,
                start_time,
                end_time,
                version: JsonVersion {
                    number: version_number,
                    hash: version_hash,
                },
                value_uuid: None,
                metrics: BTreeMap::new(),
            });
        }
        let Some(pending) = line.last_mut() else {
            debug_assert!(false, "the pending metric was just pushed");
            continue;
        };
        pending.push(project, query_metric, perf_boundary);
    }

    let (Some(dimensions), Some(benchmark)) = (dimensions, benchmark) else {
        return Ok(Vec::new());
    };
    let QueryDimensions {
        branch,
        head,
        testbed,
        measure,
    } = dimensions;
    let json_branch = branch.into_json_for_head(conn, project, &head, None)?;
    let json_testbed = testbed.into_json_for_spec(conn, project, spec_id)?;
    let json_benchmark = benchmark.into_json_for_project(project);
    let json_measure = measure.into_json_for_project(project);

    let mut results = Vec::with_capacity(lines.len());
    for (parameter_id, line) in lines {
        let Some(variant) = variants.variants.get(&parameter_id) else {
            debug_assert!(false, "the queried variant is one of the matched ones");
            continue;
        };
        let metrics = line
            .into_iter()
            .map(PendingMetric::into_json)
            .collect::<Vec<_>>();
        results.push(JsonPerfLine {
            branch: json_branch.clone(),
            testbed: json_testbed.clone(),
            benchmark: json_benchmark.clone(),
            parameter: variant.clone().into_json_for_benchmark(&variants.benchmark),
            measure: json_measure.clone(),
            metrics,
        });
    }
    Ok(results)
}

/// The deprecated metric triple needs the `value`, `lower_value`, and `upper_value`
/// rows together, which is only true once the last row has been read.
struct PendingMetric {
    report_benchmark_id: ReportBenchmarkId,
    report: ReportUuid,
    iteration: Iteration,
    start_time: DateTime,
    end_time: DateTime,
    version: JsonVersion,
    /// The identifier the deprecated metric triple carries.
    value_uuid: Option<MetricUuid>,
    metrics: BTreeMap<MetricName, JsonMetricEntry>,
}

impl PendingMetric {
    fn push(
        &mut self,
        project: &QueryProject,
        query_metric: QueryMetric,
        perf_boundary: Option<PerfBoundary>,
    ) {
        let QueryMetric {
            id: _,
            uuid,
            report_benchmark_id: _,
            measure_id: _,
            name,
            value,
        } = query_metric;

        if name == MetricName::value() {
            self.value_uuid = Some(uuid);
        }

        // A metric repeats across rows only when several thresholds checked it.
        let entry = self.metrics.entry(name).or_insert(JsonMetricEntry {
            value: value.into(),
            boundaries: None,
        });
        if let Some((query_threshold, query_model, query_boundary, query_alert)) = perf_boundary {
            entry
                .boundaries
                .get_or_insert_with(Vec::new)
                .push(JsonPerfBoundary {
                    threshold: query_threshold
                        .into_threshold_model_json_for_project(project, query_model),
                    boundary: query_boundary.into_json(),
                    alert: query_alert.map(QueryAlert::into_perf_json),
                });
        }
    }

    /// A measure with no `value` metric is still a point, but it has no deprecated
    /// metric triple.
    fn into_json(self) -> JsonPerfMetrics {
        let Self {
            report_benchmark_id: _,
            report,
            iteration,
            start_time,
            end_time,
            version,
            value_uuid,
            metrics,
        } = self;

        let value = metrics.get(&MetricName::value());
        let metric = value_uuid.zip(value).map(|(uuid, value)| JsonMetricTriple {
            uuid,
            value: value.value,
            lower_value: metrics
                .get(&MetricName::lower_value())
                .map(|entry| entry.value),
            upper_value: metrics
                .get(&MetricName::upper_value())
                .map(|entry| entry.value),
        });
        // The deprecated check is the one that checked the `value` row.
        let (threshold, boundary, alert) = value.map_or((None, None, None), deprecated_check);

        JsonPerfMetrics {
            report,
            iteration,
            start_time,
            end_time,
            version,
            metrics,
            metric,
            threshold,
            boundary,
            alert,
        }
    }
}

pub(super) fn threshold_model_alert(
    project: &QueryProject,
    tma: Option<(QueryThreshold, QueryModel, Option<QueryAlert>)>,
) -> (Option<JsonThresholdModel>, Option<JsonPerfAlert>) {
    if let Some((query_threshold, query_model, query_alert)) = tma {
        let threshold =
            Some(query_threshold.into_threshold_model_json_for_project(project, query_model));
        let alert = query_alert.map(QueryAlert::into_perf_json);
        (threshold, alert)
    } else {
        (None, None)
    }
}

type DeprecatedCheck = (
    Option<JsonThresholdModel>,
    Option<JsonBoundary>,
    Option<JsonPerfAlert>,
);

fn deprecated_check(value: &JsonMetricEntry) -> DeprecatedCheck {
    let Some(perf_boundary) = value
        .boundaries
        .as_ref()
        .and_then(|boundaries| boundaries.first())
    else {
        return (None, None, None);
    };
    let JsonPerfBoundary {
        threshold,
        boundary,
        alert,
    } = perf_boundary;
    (Some(threshold.clone()), Some(*boundary), alert.clone())
}
