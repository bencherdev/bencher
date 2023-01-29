use std::str::FromStr;

use bencher_json::{
    project::perf::{JsonPerfMetric, JsonPerfMetrics},
    JsonMetric, JsonPerf, JsonPerfQuery, ResourceId,
};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl, SqliteConnection};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    context::Context,
    endpoints::{
        endpoint::{pub_response_accepted, response_accepted, ResponseAccepted},
        Endpoint, Method,
    },
    error::api_error,
    model::project::{
        branch::QueryBranch, metric_kind::QueryMetricKind, report::to_date_time,
        testbed::QueryTestbed, QueryProject,
    },
    model::user::auth::AuthUser,
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        same_project::SameProject,
    },
    ApiError,
};

use super::Resource;

const PERF_RESOURCE: Resource = Resource::Perf;

#[derive(Deserialize, JsonSchema)]
pub struct DirPath {
    pub project: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/perf",
    tags = ["projects", "perf"]
}]
pub async fn options(
    _rqctx: RequestContext<Context>,
    _path_params: Path<DirPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/perf",
    tags = ["projects", "perf"]
}]
pub async fn post(
    rqctx: RequestContext<Context>,
    path_params: Path<DirPath>,
    body: TypedBody<JsonPerfQuery>,
) -> Result<ResponseAccepted<JsonPerf>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(PERF_RESOURCE, Method::Post);

    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        auth_user.as_ref(),
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    if auth_user.is_some() {
        response_accepted!(endpoint, json)
    } else {
        pub_response_accepted!(endpoint, json)
    }
}

#[derive(Clone, Copy, Default)]
struct Ids {
    project_id: i32,
    metric_kind_id: i32,
    branch_id: i32,
    testbed_id: i32,
}

#[derive(Clone, Copy, Default)]
struct Uuids {
    branch: Uuid,
    testbed: Uuid,
    benchmark: Uuid,
}

#[derive(Clone, Copy)]
struct Times {
    start_time_nanos: Option<i64>,
    end_time_nanos: Option<i64>,
}

async fn post_inner(
    context: &Context,
    path_params: DirPath,
    json_perf_query: JsonPerfQuery,
    auth_user: Option<&AuthUser>,
) -> Result<JsonPerf, ApiError> {
    let api_context = &mut *context.lock().await;
    let project_id =
        QueryProject::is_allowed_public(api_context, &path_params.project, auth_user)?.id;
    let conn = &mut api_context.database.connection;

    let JsonPerfQuery {
        branches,
        testbeds,
        benchmarks,
        metric_kind,
        start_time,
        end_time,
    } = json_perf_query;

    let metric_kind = QueryMetricKind::from_resource_id(conn, project_id, &metric_kind)?;

    let times = Times {
        start_time_nanos: start_time.as_ref().map(chrono::DateTime::timestamp_nanos),
        end_time_nanos: end_time.as_ref().map(chrono::DateTime::timestamp_nanos),
    };

    let mut results = Vec::new();
    let mut ids = Ids {
        project_id,
        metric_kind_id: metric_kind.id,
        ..Default::default()
    };
    let mut uuids = Uuids::default();

    for branch in &branches {
        let Ok(branch_id) = QueryBranch::get_id(conn, branch) else {
            continue;
        };
        ids.branch_id = branch_id;
        uuids.branch = *branch;
        for testbed in &testbeds {
            let Ok(testbed_id) = QueryTestbed::get_id(conn, testbed) else {
                continue;
            };
            ids.testbed_id = testbed_id;
            uuids.testbed = *testbed;
            for benchmark in &benchmarks {
                uuids.benchmark = *benchmark;

                perf_query(conn, ids, uuids, times, &mut results)?;
            }
        }
    }

    Ok(JsonPerf {
        metric_kind: Uuid::from_str(&metric_kind.uuid).map_err(api_error!())?,
        start_time,
        end_time,
        results,
    })
}

// #[allow(clippy::type_complexity)]
type PerfQuery = (
    String,
    i32,
    i64,
    i64,
    i32,
    Option<String>,
    f64,
    Option<f64>,
    Option<f64>,
);

fn perf_query(
    conn: &mut SqliteConnection,
    ids: Ids,
    uuids: Uuids,
    times: Times,
    results: &mut Vec<JsonPerfMetrics>,
) -> Result<(), ApiError> {
    let Ids {
        project_id,
        metric_kind_id,
        branch_id,
        testbed_id,
    } = ids;

    // Verify that the branch and testbed are part of the same project
    SameProject::validate_ids(conn, project_id, branch_id, testbed_id)?;

    let Uuids {
        branch,
        testbed,
        benchmark,
    } = uuids;

    let mut query = schema::perf::table
        .left_join(
            schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
        )
        .filter(schema::benchmark::uuid.eq(benchmark.to_string()))
        .filter(schema::benchmark::project_id.eq(project_id))
        .inner_join(schema::report::table.on(schema::perf::report_id.eq(schema::report::id)))
        .into_boxed();

    let Times {
        start_time_nanos,
        end_time_nanos,
    } = times;

    if let Some(start_time) = start_time_nanos {
        query = query.filter(schema::report::start_time.ge(start_time));
    }
    if let Some(end_time) = end_time_nanos {
        query = query.filter(schema::report::end_time.le(end_time));
    }

    let metrics = query
        .inner_join(schema::version::table.on(schema::report::version_id.eq(schema::version::id)))
        .left_join(
            schema::branch_version::table
                .on(schema::version::id.eq(schema::branch_version::version_id)),
        )
        .filter(schema::branch_version::branch_id.eq(branch_id))
        .filter(schema::report::testbed_id.eq(testbed_id))
        .inner_join(schema::metric::table.on(schema::perf::id.eq(schema::metric::perf_id)))
        .filter(schema::metric::metric_kind_id.eq(metric_kind_id))
        .order((
            schema::version::number,
            schema::report::start_time,
            schema::perf::iteration,
        ))
        .select((
            schema::perf::uuid,
            schema::perf::iteration,
            schema::report::start_time,
            schema::report::end_time,
            schema::version::number,
            schema::version::hash,
            schema::metric::value,
            schema::metric::lower_bound,
            schema::metric::upper_bound,
        ))
        .load::<PerfQuery>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(perf_metric)
        .collect();

    results.push(JsonPerfMetrics {
        branch,
        testbed,
        benchmark,
        metrics,
    });

    Ok(())
}

#[allow(clippy::cast_sign_loss)]
fn perf_metric(
    (
        uuid,
        iteration,
        start_time,
        end_time,
        version_number,
        version_hash,
        value,
        lower_bound,
        upper_bound,
    ): PerfQuery,
) -> Option<JsonPerfMetric> {
    Some(JsonPerfMetric {
        uuid: Uuid::from_str(&uuid).map_err(api_error!()).ok()?,
        iteration: iteration as u32,
        start_time: to_date_time(start_time).ok()?,
        end_time: to_date_time(end_time).ok()?,
        version_number: version_number as u32,
        version_hash,
        metric: JsonMetric {
            value: value.into(),
            lower_bound: lower_bound.map(Into::into),
            upper_bound: upper_bound.map(Into::into),
        },
    })
}
