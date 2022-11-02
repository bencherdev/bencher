use std::{str::FromStr, sync::Arc};

use bencher_json::{
    project::perf::{JsonPerfData, JsonPerfDatum, JsonPerfDatumKind, JsonPerfKind},
    JsonPerf, JsonPerfQuery, ResourceId,
};
use bencher_rbac::project::Permission;
use diesel::{ExpressionMethods, JoinOnDsl, NullableExpressionMethods, QueryDsl, RunQueryDsl};
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
        branch::QueryBranch,
        perf::{latency::QueryLatency, resource::QueryResource, throughput::QueryThroughput},
        report::to_date_time,
        testbed::QueryTestbed,
        QueryProject,
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
        kind,
        start_time,
        end_time,
    } = json_perf_query;

    // In order to make the type system happy, always query a start and end time.
    // If either is missing then just default to the extremes: zero and max.
    let start_time_nanos = start_time
        .as_ref()
        .map(|t| t.timestamp_nanos())
        .unwrap_or_default();
    let end_time_nanos = end_time
        .as_ref()
        .map(|t| t.timestamp_nanos())
        .unwrap_or(i64::MAX);

    let order_by = (
        schema::version::number,
        schema::report::start_time,
        schema::perf::iteration,
    );

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

                let query = schema::perf::table
                    .left_join(
                        schema::benchmark::table
                            .on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
                    )
                    .filter(schema::benchmark::uuid.eq(benchmark.to_string()))
                    .filter(schema::benchmark::project_id.eq(project_id))
                    .inner_join(
                        schema::report::table.on(schema::perf::report_id.eq(schema::report::id)),
                    )
                    .filter(schema::report::start_time.ge(start_time_nanos))
                    .filter(schema::report::end_time.le(end_time_nanos))
                    .inner_join(
                        schema::version::table
                            .on(schema::report::version_id.eq(schema::version::id)),
                    )
                    .filter(schema::version::branch_id.eq(branch_id))
                    .filter(schema::report::testbed_id.eq(testbed_id));

                let query_data: Vec<QueryPerfDatum> = match kind {
                    JsonPerfKind::Latency => query
                        .inner_join(
                            schema::latency::table
                                .on(schema::perf::latency_id.eq(schema::latency::id.nullable())),
                        )
                        .select((
                            schema::perf::uuid,
                            schema::perf::iteration,
                            schema::report::start_time,
                            schema::report::end_time,
                            schema::version::number,
                            schema::version::hash,
                            schema::latency::id,
                            schema::latency::uuid,
                            schema::latency::lower_variance,
                            schema::latency::upper_variance,
                            schema::latency::duration,
                        ))
                        .order(&order_by)
                        .load::<(
                            String,
                            i32,
                            i64,
                            i64,
                            i32,
                            Option<String>,
                            i32,
                            String,
                            i64,
                            i64,
                            i64,
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
                                latency_id,
                                latency_uuid,
                                lower_variance,
                                upper_variance,
                                duration,
                            )| {
                                let metrics = QueryPerfMetrics::Latency(QueryLatency {
                                    id: latency_id,
                                    uuid: latency_uuid,
                                    lower_variance,
                                    upper_variance,
                                    duration,
                                });
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
                        .collect(),
                    JsonPerfKind::Throughput => {
                        query
                            .inner_join(schema::throughput::table.on(
                                schema::perf::throughput_id.eq(schema::throughput::id.nullable()),
                            ))
                            .select((
                                schema::perf::uuid,
                                schema::perf::iteration,
                                schema::report::start_time,
                                schema::report::end_time,
                                schema::version::number,
                                schema::version::hash,
                                schema::throughput::id,
                                schema::throughput::uuid,
                                schema::throughput::lower_variance,
                                schema::throughput::upper_variance,
                                schema::throughput::events,
                                schema::throughput::unit_time,
                            ))
                            .order(&order_by)
                            .load::<(
                                String,
                                i32,
                                i64,
                                i64,
                                i32,
                                Option<String>,
                                i32,
                                String,
                                f64,
                                f64,
                                f64,
                                i64,
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
                                    throughput_id,
                                    throughput_uuid,
                                    lower_variance,
                                    upper_variance,
                                    events,
                                    unit_time,
                                )| {
                                    let metrics = QueryPerfMetrics::Throughput(QueryThroughput {
                                        id: throughput_id,
                                        uuid: throughput_uuid,
                                        lower_variance,
                                        upper_variance,
                                        events,
                                        unit_time,
                                    });
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
                            .collect()
                    },
                    JsonPerfKind::Compute => query
                        .inner_join(
                            schema::resource::table
                                .on(schema::perf::compute_id.eq(schema::resource::id.nullable())),
                        )
                        .select((
                            schema::perf::uuid,
                            schema::perf::iteration,
                            schema::report::start_time,
                            schema::report::end_time,
                            schema::version::number,
                            schema::version::hash,
                            schema::resource::id,
                            schema::resource::uuid,
                            schema::resource::min,
                            schema::resource::max,
                            schema::resource::avg,
                        ))
                        .order(&order_by)
                        .load::<(
                            String,
                            i32,
                            i64,
                            i64,
                            i32,
                            Option<String>,
                            i32,
                            String,
                            f64,
                            f64,
                            f64,
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
                                mma_id,
                                mma_uuid,
                                min,
                                max,
                                avg,
                            )| {
                                let metrics = QueryPerfMetrics::Compute(QueryResource {
                                    id: mma_id,
                                    uuid: mma_uuid,
                                    min,
                                    max,
                                    avg,
                                });
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
                        .collect(),
                    JsonPerfKind::Memory => query
                        .inner_join(
                            schema::resource::table
                                .on(schema::perf::memory_id.eq(schema::resource::id.nullable())),
                        )
                        .select((
                            schema::perf::uuid,
                            schema::perf::iteration,
                            schema::report::start_time,
                            schema::report::end_time,
                            schema::version::number,
                            schema::version::hash,
                            schema::resource::id,
                            schema::resource::uuid,
                            schema::resource::min,
                            schema::resource::max,
                            schema::resource::avg,
                        ))
                        .order(&order_by)
                        .load::<(
                            String,
                            i32,
                            i64,
                            i64,
                            i32,
                            Option<String>,
                            i32,
                            String,
                            f64,
                            f64,
                            f64,
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
                                mma_id,
                                mma_uuid,
                                min,
                                max,
                                avg,
                            )| {
                                let metrics = QueryPerfMetrics::Memory(QueryResource {
                                    id: mma_id,
                                    uuid: mma_uuid,
                                    min,
                                    max,
                                    avg,
                                });
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
                        .collect(),
                    JsonPerfKind::Storage => query
                        .inner_join(
                            schema::resource::table
                                .on(schema::perf::storage_id.eq(schema::resource::id.nullable())),
                        )
                        .select((
                            schema::perf::uuid,
                            schema::perf::iteration,
                            schema::report::start_time,
                            schema::report::end_time,
                            schema::version::number,
                            schema::version::hash,
                            schema::resource::id,
                            schema::resource::uuid,
                            schema::resource::min,
                            schema::resource::max,
                            schema::resource::avg,
                        ))
                        .order(&order_by)
                        .load::<(
                            String,
                            i32,
                            i64,
                            i64,
                            i32,
                            Option<String>,
                            i32,
                            String,
                            f64,
                            f64,
                            f64,
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
                                mma_id,
                                mma_uuid,
                                min,
                                max,
                                avg,
                            )| {
                                let metrics = QueryPerfMetrics::Storage(QueryResource {
                                    id: mma_id,
                                    uuid: mma_uuid,
                                    min,
                                    max,
                                    avg,
                                });
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
                        .collect(),
                };

                let json_perf_data = into_json(*branch, *testbed, *benchmark, query_data)?;

                data.push(json_perf_data);
            }
        }
    }

    Ok(JsonPerf {
        kind,
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
    pub metrics: QueryPerfMetrics,
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
            metrics: QueryPerfMetrics::into_json(metrics)?,
        })
    }
}

#[derive(Debug)]
pub enum QueryPerfMetrics {
    Latency(QueryLatency),
    Throughput(QueryThroughput),
    Compute(QueryResource),
    Memory(QueryResource),
    Storage(QueryResource),
}

impl QueryPerfMetrics {
    fn into_json(self) -> Result<JsonPerfDatumKind, HttpError> {
        Ok(match self {
            Self::Latency(latency) => JsonPerfDatumKind::Latency(QueryLatency::into_json(latency)?),
            Self::Throughput(throughput) => {
                JsonPerfDatumKind::Throughput(QueryThroughput::into_json(throughput)?)
            },
            Self::Compute(resource) => {
                JsonPerfDatumKind::Compute(QueryResource::into_json(resource))
            },
            Self::Memory(resource) => JsonPerfDatumKind::Memory(QueryResource::into_json(resource)),
            Self::Storage(resource) => {
                JsonPerfDatumKind::Storage(QueryResource::into_json(resource))
            },
        })
    }
}
