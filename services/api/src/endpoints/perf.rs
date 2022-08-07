use std::sync::Arc;

use bencher_json::{
    perf::JsonPerfKind,
    JsonBranch,
    JsonPerf,
    JsonPerfQuery,
    ResourceId,
};
use chrono::NaiveDateTime;
use diesel::{
    JoinOnDsl,
    NullableExpressionMethods,
    QueryDsl,
    RunQueryDsl,
};
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseHeaders,
    HttpResponseOk,
    Path,
    RequestContext,
    TypedBody,
};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    db::{
        model::{
            branch::QueryBranch,
            perf::QueryPerf,
            project::QueryProject,
        },
        schema,
    },
    diesel::ExpressionMethods,
    util::{
        cors::get_cors,
        headers::CorsHeaders,
        http_error,
        Context,
    },
};

const PERF_ERROR: &str = "Failed to get benchmark data.";

#[derive(Deserialize, JsonSchema)]
pub struct PathParams {
    pub project: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/perf",
    tags = ["projects", "perf"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<PathParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = PUT,
    path =  "/v0/projects/{project}/perf",
    tags = ["projects", "perf"]
}]
pub async fn put(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<PathParams>,
    body: TypedBody<JsonPerfQuery>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<JsonPerf>>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();
    let path_params = path_params.into_inner();
    let JsonPerfQuery {
        branches,
        testbeds,
        benchmarks,
        kind,
        start_time,
        end_time,
    } = body.into_inner();
    let branches: Vec<String> = branches.into_iter().map(|uuid| uuid.to_string()).collect();
    let testbeds: Vec<String> = testbeds.into_iter().map(|uuid| uuid.to_string()).collect();
    let benchmarks: Vec<String> = benchmarks
        .into_iter()
        .map(|uuid| uuid.to_string())
        .collect();

    let conn = db_connection.lock().await;
    // let mut perf_data = Vec::new();
    for branch in &branches {
        for testbed in &testbeds {
            for benchmark in &benchmarks {
                let query = schema::perf::table
                    .left_join(
                        schema::benchmark::table
                            .on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
                    )
                    .filter(schema::benchmark::uuid.eq(benchmark))
                    .inner_join(
                        schema::report::table.on(schema::perf::report_id.eq(schema::report::id)),
                    )
                    .left_join(
                        schema::testbed::table
                            .on(schema::report::testbed_id.eq(schema::testbed::id)),
                    )
                    .filter(schema::testbed::uuid.eq(testbed))
                    .inner_join(
                        schema::version::table
                            .on(schema::report::version_id.eq(schema::version::id)),
                    )
                    .left_join(
                        schema::branch::table.on(schema::version::branch_id.eq(schema::branch::id)),
                    )
                    .filter(schema::branch::uuid.eq(branch));

                let query = match kind {
                    JsonPerfKind::Latency => {
                        let latency =
                            query
                                .inner_join(schema::latency::table.on(
                                    schema::perf::latency_id.eq(schema::latency::id.nullable()),
                                ))
                                .select((
                                    schema::perf::uuid,
                                    schema::report::start_time,
                                    schema::report::end_time,
                                    schema::version::number,
                                    schema::version::hash,
                                    schema::latency::lower_variance,
                                    schema::latency::upper_variance,
                                    schema::latency::duration,
                                ))
                                .order(schema::version::number)
                                .load::<(String, String, String, i32, Option<String>, i64, i64, i64)>(
                                    &*conn,
                                )
                                .map_err(|_| http_error!(PERF_ERROR))?;
                    },
                    JsonPerfKind::Throughput => {
                        todo!()
                    },
                    JsonPerfKind::Compute => {
                        todo!()
                    },
                    JsonPerfKind::Memory => {
                        todo!()
                    },
                    JsonPerfKind::Storage => {
                        todo!()
                    },
                };

                //     .select((schema::perf::uuid))
                //     .load::<(String)>(&*conn)
                //     .map_err(|_| http_error!(PERF_ERROR))?;

                // perf_data.push(data);
            }
        }
    }

    // let query_project = QueryProject::from_resource_id(&*conn,
    // &path_params.project)?; let json: Vec<JsonBranch> = schema::branch::table
    //     .filter(schema::branch::project_id.eq(&query_project.id))
    //     .order(schema::branch::name)
    //     .load::<QueryBranch>(&*conn)
    //     .map_err(|_| http_error!("Failed to get branches."))?
    //     .into_iter()
    //     .filter_map(|query| query.to_json(&*conn).ok())
    //     .collect();

    todo!();
    // Ok(HttpResponseHeaders::new(
    //     HttpResponseOk(json),
    //     CorsHeaders::new_pub("PUT".into()),
    // ))
}
