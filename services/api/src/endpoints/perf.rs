use std::sync::Arc;

use bencher_json::{
    JsonBranch,
    JsonPerf,
    JsonPerfQuery,
    ResourceId,
};
use chrono::NaiveDateTime;
use diesel::{
    JoinOnDsl,
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
    let JsonPerfQuery {
        branches,
        testbeds,
        benchmarks,
        kind,
        start_time,
        end_time,
    } = body.into_inner();
    let path_params = path_params.into_inner();
    let kind = serde_json::to_string(&kind).map_err(|_| http_error!(PERF_ERROR))?;

    let conn = db_connection.lock().await;
    let mut perf_data = Vec::new();
    for branch in branches {
        for testbed in testbeds {
            for benchmark in benchmarks {
                let data = schema::perf::table
                    .inner_join(
                        schema::benchmark::table
                            .on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
                    )
                    .filter(schema::benchmark::uuid.eq(&benchmark.to_string()))
                    .select((schema::benchmark::uuid, schema::perf::uuid))
                    .load::<(String, String)>(&*conn)
                    .map_err(|_| http_error!(PERF_ERROR))?;

                perf_data.push(data);
            }
        }
    }

    let query_project = QueryProject::from_resource_id(&*conn, &path_params.project)?;
    let json: Vec<JsonBranch> = schema::branch::table
        .filter(schema::branch::project_id.eq(&query_project.id))
        .order(schema::branch::name)
        .load::<QueryBranch>(&*conn)
        .map_err(|_| http_error!("Failed to get branches."))?
        .into_iter()
        .filter_map(|query| query.to_json(&*conn).ok())
        .collect();

    todo!();
    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("PUT".into()),
    ))
}
