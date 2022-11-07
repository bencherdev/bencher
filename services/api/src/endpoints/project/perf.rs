use std::{str::FromStr, sync::Arc};

use bencher_json::{
    project::{
        perf::{JsonPerfData, JsonPerfDatum},
        report::JsonMetric,
    },
    JsonPerf, JsonPerfQuery, ResourceId,
};
use bencher_rbac::project::Permission;
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl};
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
        branch::QueryBranch, perf::metric_kind::QueryMetricKind, report::to_date_time,
        testbed::QueryTestbed, QueryProject,
    },
    model::user::auth::AuthUser,
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        map_http_error,
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

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/perf",
    tags = ["projects", "perf"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
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
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<DirPath>,
    body: TypedBody<JsonPerfQuery>,
) -> Result<ResponseAccepted<JsonPerf>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(PERF_RESOURCE, Method::Put);

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

async fn post_inner(
    context: &Context,
    path_params: DirPath,
    json_perf_query: JsonPerfQuery,
    auth_user: Option<&AuthUser>,
) -> Result<JsonPerf, ApiError> {
    let api_context = &mut *context.lock().await;

    // If there is an `AuthUser` then validate access
    // Otherwise, check to see if the project is public
    let project_id = if let Some(auth_user) = auth_user {
        // Verify that the user is allowed
        QueryProject::is_allowed_resource_id(
            api_context,
            &path_params.project,
            auth_user,
            Permission::View,
        )?
        .id
    } else {
        let project =
            QueryProject::from_resource_id(&mut api_context.database, &path_params.project)?;
        if project.public {
            project.id
        } else {
            return Err(ApiError::PrivateProject(project.id));
        }
    };

    let conn = &mut api_context.database;
    let JsonPerfQuery {
        branches,
        testbeds,
        benchmarks,
        metric_kind,
        start_time,
        end_time,
    } = json_perf_query;

    let metric_kind = QueryMetricKind::from_resource_id(conn, &metric_kind)?;
    let start_time_nanos = start_time.as_ref().map(|t| t.timestamp_nanos());
    let end_time_nanos = end_time.as_ref().map(|t| t.timestamp_nanos());

    let mut data = Vec::new();
    for branch in &branches {
        let branch_id = if let Ok(id) = QueryBranch::get_id(conn, branch) {
            id
        } else {
            continue;
        };
        for testbed in &testbeds {
            let testbed_id = if let Ok(id) = QueryTestbed::get_id(conn, testbed) {
                id
            } else {
                continue;
            };
            for benchmark in &benchmarks {
                // Verify that the branch and testbed are part of the same project
                SameProject::validate_ids(conn, project_id, branch_id, testbed_id)?;

                let mut query = schema::perf::table
                    .left_join(
                        schema::benchmark::table
                            .on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
                    )
                    .filter(schema::benchmark::uuid.eq(benchmark.to_string()))
                    .filter(schema::benchmark::project_id.eq(project_id))
                    .inner_join(
                        schema::report::table.on(schema::perf::report_id.eq(schema::report::id)),
                    )
                    .into_boxed();

                if let Some(start_time) = start_time_nanos {
                    query = query.filter(schema::report::start_time.ge(start_time));
                }
                if let Some(end_time) = end_time_nanos {
                    query = query.filter(schema::report::end_time.le(end_time));
                }

                let query_data = query
                    .inner_join(
                        schema::version::table
                            .on(schema::report::version_id.eq(schema::version::id)),
                    )
                    .filter(schema::version::branch_id.eq(branch_id))
                    .filter(schema::report::testbed_id.eq(testbed_id))
                    .inner_join(
                        schema::metric::table.on(schema::perf::id.eq(schema::metric::perf_id)),
                    )
                    .filter(schema::metric::metric_kind_id.eq(metric_kind.id))
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
                    .load::<(
                        String,
                        i32,
                        i64,
                        i64,
                        i32,
                        Option<String>,
                        f64,
                        Option<f64>,
                        Option<f64>,
                    )>(conn)
                    .map_err(api_error!())?
                    .into_iter()
                    .map(
                        |(
                            uuid,
                            iteration,
                            start_time,
                            end_time,
                            version_number,
                            version_hash,
                            value,
                            lower_bound,
                            upper_bound,
                        )| {
                            let metrics = JsonMetric {
                                value: value.into(),
                                lower_bound: lower_bound.map(Into::into),
                                upper_bound: upper_bound.map(Into::into),
                            };
                            QueryPerfDatum {
                                uuid,
                                iteration,
                                start_time,
                                end_time,
                                version_number,
                                version_hash,
                                metrics,
                            }
                        },
                    )
                    .collect();

                let json_perf_data = into_json(*branch, *testbed, *benchmark, query_data)?;

                data.push(json_perf_data);
            }
        }
    }

    Ok(JsonPerf {
        metric_kind: Uuid::from_str(&metric_kind.uuid).map_err(api_error!())?,
        start_time,
        end_time,
        benchmarks: data,
    })
}

fn into_json(
    branch: Uuid,
    testbed: Uuid,
    benchmark: Uuid,
    query_data: Vec<QueryPerfDatum>,
) -> Result<JsonPerfData, HttpError> {
    let mut data = Vec::new();
    for query_datum in query_data {
        data.push(QueryPerfDatum::into_json(query_datum)?)
    }
    Ok(JsonPerfData {
        branch,
        testbed,
        benchmark,
        data,
    })
}

#[derive(Debug)]
pub struct QueryPerfDatum {
    pub uuid: String,
    pub iteration: i32,
    pub start_time: i64,
    pub end_time: i64,
    pub version_number: i32,
    pub version_hash: Option<String>,
    pub metrics: JsonMetric,
}

impl QueryPerfDatum {
    fn into_json(self) -> Result<JsonPerfDatum, HttpError> {
        let Self {
            uuid,
            iteration,
            start_time,
            end_time,
            version_number,
            version_hash,
            metrics,
        } = self;
        Ok(JsonPerfDatum {
            uuid: Uuid::from_str(&uuid)
                .map_err(map_http_error!("Failed to get benchmark data."))?,
            iteration: iteration as u32,
            start_time: to_date_time(start_time)?,
            end_time: to_date_time(end_time)?,
            version_number: version_number as u32,
            version_hash,
            metrics,
        })
    }
}
