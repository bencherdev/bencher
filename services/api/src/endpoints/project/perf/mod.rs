use bencher_json::{
    project::{
        boundary::JsonBoundary,
        branch::{JsonVersion, VersionNumber},
        perf::{Iteration, JsonPerfMetric, JsonPerfMetrics, JsonPerfQueryParams},
    },
    DateTime, GitHash, JsonBenchmark, JsonBranch, JsonPerf, JsonPerfQuery, JsonTestbed, ReportUuid,
    ResourceId,
};
use diesel::{
    ExpressionMethods, NullableExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper,
};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use http::StatusCode;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::{ApiContext, DbConnection},
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    error::{bad_request_error, issue_error, resource_not_found_err},
    model::user::auth::AuthUser,
    model::{
        project::{
            benchmark::{BenchmarkId, QueryBenchmark},
            branch::{BranchId, QueryBranch},
            metric::QueryMetric,
            metric_kind::{MetricKindId, QueryMetricKind},
            testbed::{QueryTestbed, TestbedId},
            threshold::{
                alert::QueryAlert, boundary::QueryBoundary, statistic::QueryStatistic,
                QueryThreshold,
            },
            QueryProject,
        },
        user::auth::PubBearerToken,
    },
    schema,
};

pub mod img;

#[derive(Deserialize, JsonSchema)]
pub struct ProjPerfParams {
    pub project: ResourceId,
}

#[allow(clippy::unused_async)]
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

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/perf",
    tags = ["projects", "perf"]
}]
pub async fn proj_perf_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjPerfParams>,
    query_params: Query<JsonPerfQueryParams>,
) -> Result<ResponseOk<JsonPerf>, HttpError> {
    // Second round of marshaling
    let json_perf_query = query_params
        .into_inner()
        .try_into()
        .map_err(bad_request_error)?;

    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    let json = get_inner(
        rqctx.context(),
        path_params.into_inner(),
        json_perf_query,
        auth_user.as_ref(),
    )
    .await?;
    Ok(Get::response_ok(json, auth_user.is_some()))
}

async fn get_inner(
    context: &ApiContext,
    path_params: ProjPerfParams,
    json_perf_query: JsonPerfQuery,
    auth_user: Option<&AuthUser>,
) -> Result<JsonPerf, HttpError> {
    let conn = &mut *context.conn().await;

    let project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    let JsonPerfQuery {
        branches,
        testbeds,
        benchmarks,
        metric_kind,
        start_time,
        end_time,
    } = json_perf_query;

    let metric_kind = QueryMetricKind::from_resource_id(conn, project.id, &metric_kind)?;

    let times = Times {
        start_time,
        end_time,
    };

    let mut results = Vec::new();
    let mut ids = Ids {
        metric_kind_id: metric_kind.id,
        ..Default::default()
    };
    let mut dimensions = Dimensions::Zero;

    for branch in &branches {
        let Ok(branch) = QueryBranch::from_uuid(conn, project.id, *branch) else {
            continue;
        };
        ids.branch_id = branch.id;
        dimensions = dimensions.branch(&project, branch);
        for testbed in &testbeds {
            let Ok(testbed) = QueryTestbed::from_uuid(conn, project.id, *testbed) else {
                continue;
            };
            ids.testbed_id = testbed.id;
            dimensions = dimensions.testbed(&project, testbed)?;

            for benchmark in &benchmarks {
                let Ok(benchmark) = QueryBenchmark::from_uuid(conn, project.id, *benchmark) else {
                    continue;
                };
                ids.benchmark_id = benchmark.id;
                dimensions = dimensions.benchmark(&project, benchmark)?;
                let (two_d, query_dimensions) = dimensions.into_query()?;
                dimensions = two_d;

                perf_query(conn, &project, ids, query_dimensions, times, &mut results)?;
            }
        }
    }

    Ok(JsonPerf {
        project: project.into_json(conn)?,
        metric_kind: metric_kind.into_json(conn)?,
        start_time,
        end_time,
        results,
    })
}

#[derive(Clone, Copy, Default)]
struct Ids {
    metric_kind_id: MetricKindId,
    branch_id: BranchId,
    testbed_id: TestbedId,
    benchmark_id: BenchmarkId,
}

#[derive(Debug)]
enum Dimensions {
    Zero,
    One {
        branch: JsonBranch,
    },
    Two {
        branch: JsonBranch,
        testbed: JsonTestbed,
    },
    Three {
        branch: JsonBranch,
        testbed: JsonTestbed,
        benchmark: JsonBenchmark,
    },
}

impl Dimensions {
    fn branch(self, project: &QueryProject, branch: QueryBranch) -> Self {
        match self {
            Self::Zero | Self::One { .. } | Self::Two { .. } | Self::Three { .. } => Self::One {
                branch: branch.into_json_for_project(project),
            },
        }
    }

    fn testbed(self, project: &QueryProject, testbed: QueryTestbed) -> Result<Self, HttpError> {
        match self {
            Self::Zero => Err(issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected testbed dimension",
                &format!(
                    "Failed to find existing branch dimension ({self:?}) for testbed dimension ({testbed:?})"
                ),
                "dimension zero",
            )),
            Self::One { branch } | Self::Two { branch, .. } | Self::Three { branch, .. } => {
                Ok(Self::Two {
                    branch,
                    testbed: testbed.into_json_for_project(project),
                })
            },
        }
    }

    fn benchmark(
        self,
        project: &QueryProject,
        benchmark: QueryBenchmark,
    ) -> Result<Self, HttpError> {
        match self {
            Self::Zero | Self::One { .. } => Err(issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unexpected benchmark dimension",
                &format!(
                    "Failed to find existing branch and testbed dimensions ({self:?}) for benchmark dimension ({benchmark:?})"
                ),
                "dimension zero or one",
            )),
            Self::Two { branch, testbed }
            | Self::Three {
                branch, testbed, ..
            } => Ok(Self::Three {
                branch,
                testbed,
                benchmark: benchmark.into_json_for_project(project),
            }),
        }
    }

    fn into_query(self) -> Result<(Self, QueryDimensions), HttpError> {
        if let Dimensions::Three {
            branch,
            testbed,
            benchmark,
        } = self
        {
            let query_dimensions = QueryDimensions {
                branch: branch.clone(),
                testbed: testbed.clone(),
                benchmark,
            };
            Ok((Self::Two { branch, testbed }, query_dimensions))
        } else {
            Err(issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Missing dimension",
                &format!("Failed to find all three dimensions ({self:?})"),
                "dimension zero, one, or two",
            ))
        }
    }
}

struct QueryDimensions {
    branch: JsonBranch,
    testbed: JsonTestbed,
    benchmark: JsonBenchmark,
}

#[derive(Clone, Copy)]
struct Times {
    start_time: Option<DateTime>,
    end_time: Option<DateTime>,
}

type PerfQuery = (
    ReportUuid,
    Iteration,
    DateTime,
    DateTime,
    VersionNumber,
    Option<GitHash>,
    Option<(
        QueryBoundary,
        QueryThreshold,
        QueryStatistic,
        Option<QueryAlert>,
    )>,
    QueryMetric,
);

#[allow(clippy::too_many_lines)]
fn perf_query(
    conn: &mut DbConnection,
    project: &QueryProject,
    ids: Ids,
    dimensions: QueryDimensions,
    times: Times,
    results: &mut Vec<JsonPerfMetrics>,
) -> Result<(), HttpError> {
    let Ids {
        metric_kind_id,
        branch_id,
        testbed_id,
        benchmark_id,
    } = ids;

    let QueryDimensions {
        branch,
        testbed,
        benchmark,
    } = dimensions;

    let mut query = schema::metric::table
        .filter(schema::metric::metric_kind_id.eq(metric_kind_id))
        .inner_join(
            schema::perf::table.inner_join(
                schema::report::table
                    .inner_join(schema::version::table.inner_join(schema::branch_version::table)),
            ),
        )
        // It is important to filter for the branch on the `branch_version` table and not on the branch in the `report` table.
        // This is because the `branch_version` table is the one that is updated when a branch is cloned/used as a start point.
        // In contrast, the `report` table is only set to a single branch when the report is created.
        .filter(schema::branch_version::branch_id.eq(branch_id))
        .filter(schema::report::testbed_id.eq(testbed_id))
        .filter(schema::perf::benchmark_id.eq(benchmark_id))
        // There may or may not be a boundary for any given metric
        .left_join(
            schema::boundary::table
                .inner_join(schema::threshold::table)
                .inner_join(schema::statistic::table)
                // There may or may not be an alert for any given boundary
                .left_join(schema::alert::table),
        )
        .into_boxed();

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

    let metrics = query
        // Order by the version number so that the oldest version is first.
        // Because multiple reports can use the same version (via git hash), order by the start time next.
        // Then within a report order by the iteration number.
        .order((
            schema::version::number,
            schema::report::start_time,
            schema::perf::iteration,
        ))
        .select((
            schema::report::uuid,
            schema::perf::iteration,
            schema::report::start_time,
            schema::report::end_time,
            schema::version::number,
            schema::version::hash,
            (
                (
                    schema::boundary::id,
                    schema::boundary::uuid,
                    schema::boundary::threshold_id,
                    schema::boundary::statistic_id,
                    schema::boundary::metric_id,
                    schema::boundary::lower_limit,
                    schema::boundary::upper_limit,
                ),
                (
                    schema::threshold::id,
                    schema::threshold::uuid,
                    schema::threshold::project_id,
                    schema::threshold::metric_kind_id,
                    schema::threshold::branch_id,
                    schema::threshold::testbed_id,
                    schema::threshold::statistic_id,
                    schema::threshold::created,
                    schema::threshold::modified,
                ),
                (
                    schema::statistic::id,
                    schema::statistic::uuid,
                    schema::statistic::threshold_id,
                    schema::statistic::test,
                    schema::statistic::min_sample_size,
                    schema::statistic::max_sample_size,
                    schema::statistic::window,
                    schema::statistic::lower_boundary,
                    schema::statistic::upper_boundary,
                    schema::statistic::created,
                ),
                (
                    schema::alert::id,
                    schema::alert::uuid,
                    schema::alert::boundary_id,
                    schema::alert::boundary_limit,
                    schema::alert::status,
                    schema::alert::modified,
                ).nullable()
            ).nullable(),
            QueryMetric::as_select(),
        ))
        .load::<PerfQuery>(conn)
        .map_err(resource_not_found_err!(Metric, (project.clone(), metric_kind_id, branch_id, testbed_id, benchmark_id)))?
        .into_iter()
        .map(|query| perf_metric(project, query))
        .collect();

    results.push(JsonPerfMetrics {
        branch,
        testbed,
        benchmark,
        metrics,
    });

    Ok(())
}

fn perf_metric(
    project: &QueryProject,
    (
        report_uuid,
        iteration,
        start_time,
        end_time,
        version_number,
        version_hash,
        boundary_limit,
        query_metric,
    ): PerfQuery,
) -> JsonPerfMetric {
    let version = JsonVersion {
        number: version_number,
        hash: version_hash,
    };

    let (boundary, threshold, alert) =
        if let Some((query_boundary, query_threshold, query_statistic, query_alert)) =
            boundary_limit
        {
            let boundary = query_boundary.into_json();
            let threshold = Some(
                query_threshold.into_threshold_statistic_json_for_project(project, query_statistic),
            );
            let alert = query_alert.map(QueryAlert::into_perf_json);
            (boundary, threshold, alert)
        } else {
            (JsonBoundary::default(), None, None)
        };

    JsonPerfMetric {
        report: report_uuid,
        iteration,
        start_time,
        end_time,
        version,
        threshold,
        metric: query_metric.into_json(),
        boundary,
        alert,
    }
}
