use std::str::FromStr;

use bencher_json::{
    project::{
        branch::JsonVersion,
        perf::{JsonPerfMetric, JsonPerfMetrics, JsonPerfQueryParams},
    },
    GitHash, JsonBenchmark, JsonBranch, JsonMetric, JsonPerf, JsonPerfQuery, JsonTestbed,
    ResourceId,
};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    context::{ApiContext, DbConnection},
    endpoints::{
        endpoint::{pub_response_ok, response_ok, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::project::{
        benchmark::{BenchmarkId, QueryBenchmark},
        branch::{BranchId, QueryBranch},
        metric::MetricId,
        metric_kind::{MetricKindId, QueryMetricKind},
        testbed::{QueryTestbed, TestbedId},
        threshold::{alert::QueryAlert, boundary::QueryBoundary, QueryThreshold},
        QueryProject,
    },
    model::user::auth::AuthUser,
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        to_date_time,
    },
    ApiError,
};

pub mod img;

use super::Resource;

const PERF_RESOURCE: Resource = Resource::Perf;

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
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/perf",
    tags = ["projects", "perf"]
}]
pub async fn proj_perf_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjPerfParams>,
    query_params: Query<JsonPerfQueryParams>,
) -> Result<ResponseOk<JsonPerf>, HttpError> {
    // Second round of marshaling
    let json_perf_query = query_params
        .into_inner()
        .try_into()
        .map_err(ApiError::from)?;

    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(PERF_RESOURCE, Method::GetLs);

    let json = get_inner(
        rqctx.context(),
        path_params.into_inner(),
        json_perf_query,
        auth_user.as_ref(),
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    if auth_user.is_some() {
        response_ok!(endpoint, json)
    } else {
        pub_response_ok!(endpoint, json)
    }
}

async fn get_inner(
    context: &ApiContext,
    path_params: ProjPerfParams,
    json_perf_query: JsonPerfQuery,
    auth_user: Option<&AuthUser>,
) -> Result<JsonPerf, ApiError> {
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
        start_time: start_time.as_ref().map(chrono::DateTime::timestamp),
        end_time: end_time.as_ref().map(chrono::DateTime::timestamp),
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
        dimensions = dimensions.branch(conn, branch)?;
        for testbed in &testbeds {
            let Ok(testbed) = QueryTestbed::from_uuid(conn, project.id, *testbed) else {
                continue;
            };
            ids.testbed_id = testbed.id;
            dimensions = dimensions.testbed(conn, testbed)?;

            for benchmark in &benchmarks {
                let Ok(benchmark) = QueryBenchmark::from_uuid(conn, project.id, *benchmark) else {
                    continue;
                };
                ids.benchmark_id = benchmark.id;
                dimensions = dimensions.benchmark(conn, benchmark)?;
                let (two_d, query_dimensions) = dimensions.into_query()?;
                dimensions = two_d;

                perf_query(conn, ids, query_dimensions, times, &mut results)?;
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
    fn branch(self, conn: &mut DbConnection, branch: QueryBranch) -> Result<Self, ApiError> {
        Ok(match self {
            Self::Zero | Self::One { .. } | Self::Two { .. } | Self::Three { .. } => Self::One {
                branch: branch.into_json(conn)?,
            },
        })
    }

    fn testbed(self, conn: &mut DbConnection, testbed: QueryTestbed) -> Result<Self, ApiError> {
        Ok(match self {
            Self::Zero => return Err(ApiError::DimensionTestbed),
            Self::One { branch } | Self::Two { branch, .. } | Self::Three { branch, .. } => {
                Self::Two {
                    branch,
                    testbed: testbed.into_json(conn)?,
                }
            },
        })
    }

    fn benchmark(
        self,
        conn: &mut DbConnection,
        benchmark: QueryBenchmark,
    ) -> Result<Self, ApiError> {
        Ok(match self {
            Self::Zero | Self::One { .. } => return Err(ApiError::DimensionBenchmark),
            Self::Two { branch, testbed }
            | Self::Three {
                branch, testbed, ..
            } => Self::Three {
                branch,
                testbed,
                benchmark: benchmark.into_json(conn)?,
            },
        })
    }

    fn into_query(self) -> Result<(Self, QueryDimensions), ApiError> {
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
            Err(ApiError::DimensionMissing)
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
    start_time: Option<i64>,
    end_time: Option<i64>,
}

type PerfQuery = (
    String,
    i32,
    i64,
    i64,
    i32,
    Option<String>,
    MetricId,
    f64,
    Option<f64>,
    Option<f64>,
);

fn perf_query(
    conn: &mut DbConnection,
    ids: Ids,
    dimensions: QueryDimensions,
    times: Times,
    results: &mut Vec<JsonPerfMetrics>,
) -> Result<(), ApiError> {
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
        .inner_join(schema::perf::table.on(schema::metric::perf_id.eq(schema::perf::id)))
        .filter(schema::perf::benchmark_id.eq(benchmark_id))
        .inner_join(schema::report::table.on(schema::perf::report_id.eq(schema::report::id)))
        .filter(schema::report::testbed_id.eq(testbed_id))
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
        // It is important to filter for the branch on the `branch_version` table and not on the branch in the `report` table.
        // This is because the `branch_version` table is the one that is updated when a branch is cloned/used as a start point.
        // In contrast, the `report` table is only set to a single branch when the report is created.
        .inner_join(schema::version::table.on(schema::report::version_id.eq(schema::version::id)))
        .left_join(
            schema::branch_version::table
                .on(schema::version::id.eq(schema::branch_version::version_id)),
        )
        .filter(schema::branch_version::branch_id.eq(branch_id))
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
            schema::metric::id,
            schema::metric::value,
            schema::metric::lower_bound,
            schema::metric::upper_bound,
        ))
        .load::<PerfQuery>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(|query| perf_metric(conn, query))
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
    conn: &mut DbConnection,
    (
        report,
        iteration,
        start_time,
        end_time,
        version_number,
        version_hash,
        metric_id,
        value,
        lower_bound,
        upper_bound,
    ): PerfQuery,
) -> Option<JsonPerfMetric> {
    // The boundary may not exist
    let boundary = QueryBoundary::from_metric_id(conn, metric_id).ok();
    let (threshold, alert) = if let Some(QueryBoundary {
        id,
        threshold_id,
        statistic_id,
        ..
    }) = boundary.as_ref()
    {
        // If a boundary exists, then a threshold and statistic must also exist
        let threshold =
            QueryThreshold::get_threshold_statistic_json(conn, *threshold_id, *statistic_id)
                .ok()?;
        // The alert may not exist
        let alert = QueryAlert::get_perf_json(conn, *id).ok();
        (Some(threshold), alert)
    } else {
        (None, None)
    };
    Some(JsonPerfMetric {
        report: Uuid::from_str(&report).ok()?,
        iteration: u32::try_from(iteration).ok()?,
        start_time: to_date_time(start_time).ok()?,
        end_time: to_date_time(end_time).ok()?,
        version: JsonVersion {
            number: u32::try_from(version_number).ok()?,
            hash: if let Some(version_hash) = version_hash.as_deref() {
                Some(GitHash::from_str(version_hash).ok()?)
            } else {
                None
            },
        },
        threshold,
        metric: JsonMetric {
            value: value.into(),
            lower_bound: lower_bound.map(Into::into),
            upper_bound: upper_bound.map(Into::into),
        },
        boundary: boundary.map(|b| b.into_json()).unwrap_or_default(),
        alert,
    })
}
