use std::sync::Arc;

use bencher_json::{
    JsonNewReport,
    JsonReport,
    ResourceId,
};
use diesel::{
    expression_methods::BoolExpressionMethods,
    JoinOnDsl,
    QueryDsl,
    RunQueryDsl,
};
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseAccepted,
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
            adapter::QueryAdapter,
            benchmark::{
                InsertBenchmark,
                QueryBenchmark,
            },
            branch::QueryBranch,
            perf::{
                InsertLatency,
                InsertMinMaxAvg,
                InsertPerf,
                InsertThroughput,
            },
            project::QueryProject,
            report::{
                InsertReport,
                QueryReport,
            },
            testbed::QueryTestbed,
            user::QueryUser,
            version::InsertVersion,
        },
        schema,
    },
    diesel::ExpressionMethods,
    util::{
        auth::get_token,
        cors::get_cors,
        headers::CorsHeaders,
        http_error,
        Context,
    },
};

#[derive(Deserialize, JsonSchema)]
pub struct GetLsParams {
    pub project: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/reports",
    tags = ["projects", "reports"]
}]
pub async fn get_ls_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetLsParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/reports",
    tags = ["projects", "reports"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetLsParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<JsonReport>>, CorsHeaders>, HttpError> {
    let user_uuid = get_token(&rqctx).await?;
    let db_connection = rqctx.context();
    let path_params = path_params.into_inner();

    let conn = db_connection.lock().await;
    // Verify that the user has access to the project
    let query_project = QueryProject::from_resource_id(&*conn, &path_params.project)?;
    QueryUser::has_access(&*conn, query_project.id, user_uuid)?;

    let json: Vec<JsonReport> = schema::report::table
        .left_join(schema::testbed::table.on(schema::report::testbed_id.eq(schema::testbed::id)))
        .filter(schema::testbed::project_id.eq(query_project.id))
        .select((
            schema::report::id,
            schema::report::uuid,
            schema::report::user_id,
            schema::report::version_id,
            schema::report::testbed_id,
            schema::report::adapter_id,
            schema::report::start_time,
            schema::report::end_time,
        ))
        .order(schema::report::start_time.desc())
        .load::<QueryReport>(&*conn)
        .map_err(|_| http_error!("Failed to get reports."))?
        .into_iter()
        .filter_map(|query| query.to_json(&*conn).ok())
        .collect();

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/reports",
    tags = ["reports"]
}]
pub async fn post_options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path = "/v0/reports",
    tags = ["reports"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonNewReport>,
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonReport>, CorsHeaders>, HttpError> {
    const ERROR: &str = "Failed to create report.";

    let user_uuid = get_token(&rqctx).await?;
    let db_connection = rqctx.context();
    let json_report = body.into_inner();

    let conn = db_connection.lock().await;
    // Verify that the branch and testbed are part of the same project
    let branch_id = QueryBranch::get_id(&*conn, &json_report.branch)?;
    let testbed_id = QueryTestbed::get_id(&*conn, &json_report.testbed)?;
    let branch_project_id = schema::branch::table
        .filter(schema::branch::id.eq(&branch_id))
        .select(schema::branch::project_id)
        .first::<i32>(&*conn)
        .map_err(|_| http_error!(ERROR))?;
    let testbed_project_id = schema::testbed::table
        .filter(schema::testbed::id.eq(&testbed_id))
        .select(schema::testbed::project_id)
        .first::<i32>(&*conn)
        .map_err(|_| http_error!(ERROR))?;
    if branch_project_id != testbed_project_id {
        return Err(http_error!(ERROR));
    }
    let project_id = branch_project_id;
    // Verify that the user has access to the project
    let user_id = QueryUser::has_access(&*conn, project_id, user_uuid)?;

    // If there is a hash then try to see if there is already a code version for
    // this branch with that particular hash.
    // Otherwise, create a new code version for this branch with/without the hash.
    let version_id = if let Some(hash) = json_report.hash {
        if let Ok(version_id) = schema::version::table
            .filter(
                schema::version::branch_id
                    .eq(branch_id)
                    .and(schema::version::hash.eq(&hash)),
            )
            .select(schema::version::id)
            .first::<i32>(&*conn)
        {
            version_id
        } else {
            InsertVersion::increment(&*conn, branch_id, Some(hash))?
        }
    } else {
        InsertVersion::increment(&*conn, branch_id, None)?
    };

    let insert_report = InsertReport::new(
        &*conn,
        user_id,
        version_id,
        testbed_id,
        &json_report.adapter,
        &json_report.start_time,
        &json_report.end_time,
    )?;

    diesel::insert_into(schema::report::table)
        .values(&insert_report)
        .execute(&*conn)
        .map_err(|_| http_error!("Failed to create report."))?;

    let query_report = schema::report::table
        .filter(schema::report::uuid.eq(&insert_report.uuid))
        .first::<QueryReport>(&*conn)
        .map_err(|_| http_error!("Failed to create report."))?;

    // For each benchmark try to see if it already exists for the project.
    // Otherwise, create it.
    for (name, perf) in json_report.benchmarks {
        let benchmark_id =
            if let Ok(query) = QueryBenchmark::get_id_from_name(&*conn, project_id, &name) {
                query
            } else {
                let insert_benchmark = InsertBenchmark::new(project_id, name);
                diesel::insert_into(schema::benchmark::table)
                    .values(&insert_benchmark)
                    .execute(&*conn)
                    .map_err(|_| http_error!("Failed to create benchmark."))?;

                schema::benchmark::table
                    .filter(schema::benchmark::uuid.eq(&insert_benchmark.uuid))
                    .select(schema::benchmark::id)
                    .first::<i32>(&*conn)
                    .map_err(|_| http_error!("Failed to create benchmark."))?
            };

        let insert_perf = InsertPerf {
            uuid: Uuid::new_v4().to_string(),
            report_id: query_report.id,
            benchmark_id,
            latency_id: InsertLatency::map_json(&*conn, perf.latency)?,
            throughput_id: InsertThroughput::map_json(&*conn, perf.throughput)?,
            compute_id: InsertMinMaxAvg::map_json(&*conn, perf.compute)?,
            memory_id: InsertMinMaxAvg::map_json(&*conn, perf.memory)?,
            storage_id: InsertMinMaxAvg::map_json(&*conn, perf.storage)?,
        };
        diesel::insert_into(schema::perf::table)
            .values(&insert_perf)
            .execute(&*conn)
            .map_err(|_| http_error!("Failed to create benchmark data."))?;

        schema::perf::table
            .filter(schema::perf::uuid.eq(&insert_perf.uuid))
            .select(schema::perf::id)
            .first::<i32>(&*conn)
            .map_err(|_| http_error!("Failed to create benchmark data."))?;
    }

    // TODO add benchmarks to JSON
    let json = query_report.to_json(&*conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(json),
        CorsHeaders::new_auth("POST".into()),
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct GetOneParams {
    pub project:     ResourceId,
    pub report_uuid: Uuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/reports/{report_uuid}",
    tags = ["projects", "reports"]
}]
pub async fn get_one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetOneParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/reports/{report_uuid}",
    tags = ["projects", "reports"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonReport>, CorsHeaders>, HttpError> {
    let user_uuid = get_token(&rqctx).await?;
    let db_connection = rqctx.context();
    let path_params = path_params.into_inner();
    let report_uuid = path_params.report_uuid.to_string();

    let conn = db_connection.lock().await;
    // Verify that the user has access to the project
    let query_project = QueryProject::from_resource_id(&*conn, &path_params.project)?;
    QueryUser::has_access(&*conn, query_project.id, user_uuid)?;

    let query = if let Ok(query) = schema::report::table
        .left_join(schema::testbed::table.on(schema::report::testbed_id.eq(schema::testbed::id)))
        .filter(
            schema::testbed::project_id
                .eq(query_project.id)
                .and(schema::report::uuid.eq(report_uuid)),
        )
        .select((
            schema::report::id,
            schema::report::uuid,
            schema::report::user_id,
            schema::report::version_id,
            schema::report::testbed_id,
            schema::report::adapter_id,
            schema::report::start_time,
            schema::report::end_time,
        ))
        .first::<QueryReport>(&*conn)
    {
        Ok(query)
    } else {
        Err(http_error!("Failed to get report."))
    }?;
    let json = query.to_json(&*conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}
