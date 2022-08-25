use std::{
    str::FromStr,
    sync::Arc,
};

use bencher_json::{
    perf::{
        JsonPerfData,
        JsonPerfDatum,
        JsonPerfDatumKind,
        JsonPerfKind,
    },
    report::{
        JsonLatency,
        JsonMinMaxAvg,
        JsonThroughput,
    },
    JsonPerf,
    JsonPerfQuery,
};
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
    RequestContext,
    TypedBody,
};
use uuid::Uuid;

use crate::{
    db::{
        model::report::to_date_time,
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

// TODO figure out why this doesn't work as a PUT
#[endpoint {
    method = OPTIONS,
    path =  "/v0/perf",
    tags = ["perf"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path =  "/v0/perf",
    tags = ["perf"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonPerfQuery>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonPerf>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();
    let JsonPerfQuery {
        branches,
        testbeds,
        benchmarks,
        kind,
        start_time,
        end_time,
    } = body.into_inner();

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

    let conn = db_connection.lock().await;
    let mut data = Vec::new();
    for branch in &branches {
        for testbed in &testbeds {
            for benchmark in &benchmarks {
                let query = schema::perf::table
                    .left_join(
                        schema::benchmark::table
                            .on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
                    )
                    .filter(schema::benchmark::uuid.eq(benchmark.to_string()))
                    .inner_join(
                        schema::report::table.on(schema::perf::report_id.eq(schema::report::id)),
                    )
                    .filter(schema::report::start_time.ge(start_time_nanos))
                    .filter(schema::report::end_time.le(end_time_nanos))
                    .left_join(
                        schema::testbed::table
                            .on(schema::report::testbed_id.eq(schema::testbed::id)),
                    )
                    .filter(schema::testbed::uuid.eq(testbed.to_string()))
                    .inner_join(
                        schema::version::table
                            .on(schema::report::version_id.eq(schema::version::id)),
                    )
                    .left_join(
                        schema::branch::table.on(schema::version::branch_id.eq(schema::branch::id)),
                    )
                    .filter(schema::branch::uuid.eq(branch.to_string()));

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
                            schema::latency::lower_variance,
                            schema::latency::upper_variance,
                            schema::latency::duration,
                        ))
                        .order(&order_by)
                        .load::<(String, i32, i64, i64, i32, Option<String>, i64, i64, i64)>(&*conn)
                        .map_err(|_| http_error!(PERF_ERROR))?
                        .into_iter()
                        .map(
                            |(
                                uuid,
                                iteration,
                                start_time,
                                end_time,
                                version_number,
                                version_hash,
                                lower_variance,
                                upper_variance,
                                duration,
                            )| {
                                let perf = QueryPerfDatumKind::Latency(QueryLatency {
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
                                    perf,
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
                                f64,
                                f64,
                                f64,
                                i64,
                            )>(&*conn)
                            .map_err(|_| http_error!(PERF_ERROR))?
                            .into_iter()
                            .map(
                                |(
                                    uuid,
                                    iteration,
                                    start_time,
                                    end_time,
                                    version_number,
                                    version_hash,
                                    lower_variance,
                                    upper_variance,
                                    events,
                                    unit_time,
                                )| {
                                    let perf = QueryPerfDatumKind::Throughput(QueryThroughput {
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
                                        perf,
                                    }
                                },
                            )
                            .collect()
                    },
                    JsonPerfKind::Compute => {
                        query
                            .inner_join(schema::min_max_avg::table.on(
                                schema::perf::compute_id.eq(schema::min_max_avg::id.nullable()),
                            ))
                            .select((
                                schema::perf::uuid,
                                schema::perf::iteration,
                                schema::report::start_time,
                                schema::report::end_time,
                                schema::version::number,
                                schema::version::hash,
                                schema::min_max_avg::min,
                                schema::min_max_avg::max,
                                schema::min_max_avg::avg,
                            ))
                            .order(&order_by)
                            .load::<(String, i32, i64, i64, i32, Option<String>, f64, f64, f64)>(
                                &*conn,
                            )
                            .map_err(|_| http_error!(PERF_ERROR))?
                            .into_iter()
                            .map(
                                |(
                                    uuid,
                                    iteration,
                                    start_time,
                                    end_time,
                                    version_number,
                                    version_hash,
                                    min,
                                    max,
                                    avg,
                                )| {
                                    let perf = QueryPerfDatumKind::Compute(QueryMinMaxAvg {
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
                                        perf,
                                    }
                                },
                            )
                            .collect()
                    },
                    JsonPerfKind::Memory => query
                        .inner_join(
                            schema::min_max_avg::table
                                .on(schema::perf::memory_id.eq(schema::min_max_avg::id.nullable())),
                        )
                        .select((
                            schema::perf::uuid,
                            schema::perf::iteration,
                            schema::report::start_time,
                            schema::report::end_time,
                            schema::version::number,
                            schema::version::hash,
                            schema::min_max_avg::min,
                            schema::min_max_avg::max,
                            schema::min_max_avg::avg,
                        ))
                        .order(&order_by)
                        .load::<(String, i32, i64, i64, i32, Option<String>, f64, f64, f64)>(&*conn)
                        .map_err(|_| http_error!(PERF_ERROR))?
                        .into_iter()
                        .map(
                            |(
                                uuid,
                                iteration,
                                start_time,
                                end_time,
                                version_number,
                                version_hash,
                                min,
                                max,
                                avg,
                            )| {
                                let perf =
                                    QueryPerfDatumKind::Memory(QueryMinMaxAvg { min, max, avg });
                                QueryPerfDatum {
                                    uuid,
                                    iteration,
                                    start_time,
                                    end_time,
                                    version_number,
                                    version_hash,
                                    perf,
                                }
                            },
                        )
                        .collect(),
                    JsonPerfKind::Storage => {
                        query
                            .inner_join(schema::min_max_avg::table.on(
                                schema::perf::storage_id.eq(schema::min_max_avg::id.nullable()),
                            ))
                            .select((
                                schema::perf::uuid,
                                schema::perf::iteration,
                                schema::report::start_time,
                                schema::report::end_time,
                                schema::version::number,
                                schema::version::hash,
                                schema::min_max_avg::min,
                                schema::min_max_avg::max,
                                schema::min_max_avg::avg,
                            ))
                            .order(&order_by)
                            .load::<(String, i32, i64, i64, i32, Option<String>, f64, f64, f64)>(
                                &*conn,
                            )
                            .map_err(|_| http_error!(PERF_ERROR))?
                            .into_iter()
                            .map(
                                |(
                                    uuid,
                                    iteration,
                                    start_time,
                                    end_time,
                                    version_number,
                                    version_hash,
                                    min,
                                    max,
                                    avg,
                                )| {
                                    let perf = QueryPerfDatumKind::Storage(QueryMinMaxAvg {
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
                                        perf,
                                    }
                                },
                            )
                            .collect()
                    },
                };

                let json_perf_data = to_json(
                    branch.clone(),
                    testbed.clone(),
                    benchmark.clone(),
                    query_data,
                )?;

                data.push(json_perf_data);
            }
        }
    }

    let json_perf = JsonPerf {
        kind,
        start_time,
        end_time,
        data,
    };

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json_perf),
        CorsHeaders::new_auth("POST".into()),
    ))
}

fn to_json(
    branch: Uuid,
    testbed: Uuid,
    benchmark: Uuid,
    query_data: Vec<QueryPerfDatum>,
) -> Result<JsonPerfData, HttpError> {
    let mut perfs = Vec::new();
    for query_datum in query_data {
        perfs.push(QueryPerfDatum::to_json(query_datum)?)
    }
    Ok(JsonPerfData {
        branch,
        testbed,
        benchmark,
        perfs,
    })
}

#[derive(Debug)]
pub struct QueryPerfDatum {
    pub uuid:           String,
    pub iteration:      i32,
    pub start_time:     i64,
    pub end_time:       i64,
    pub version_number: i32,
    pub version_hash:   Option<String>,
    pub perf:           QueryPerfDatumKind,
}

impl QueryPerfDatum {
    fn to_json(self) -> Result<JsonPerfDatum, HttpError> {
        let Self {
            uuid,
            iteration,
            start_time,
            end_time,
            version_number,
            version_hash,
            perf,
        } = self;
        Ok(JsonPerfDatum {
            uuid: Uuid::from_str(&uuid).map_err(|_| http_error!(PERF_ERROR))?,
            iteration: iteration as u32,
            start_time: to_date_time(start_time)?,
            end_time: to_date_time(end_time)?,
            version_number: version_number as u32,
            version_hash,
            perf: QueryPerfDatumKind::to_json(perf)?,
        })
    }
}

#[derive(Debug)]
pub enum QueryPerfDatumKind {
    Latency(QueryLatency),
    Throughput(QueryThroughput),
    Compute(QueryMinMaxAvg),
    Memory(QueryMinMaxAvg),
    Storage(QueryMinMaxAvg),
}

impl QueryPerfDatumKind {
    fn to_json(self) -> Result<JsonPerfDatumKind, HttpError> {
        Ok(match self {
            Self::Latency(latency) => JsonPerfDatumKind::Latency(QueryLatency::to_json(latency)?),
            Self::Throughput(throughput) => {
                JsonPerfDatumKind::Throughput(QueryThroughput::to_json(throughput)?)
            },
            Self::Compute(min_max_avg) => {
                JsonPerfDatumKind::Compute(QueryMinMaxAvg::to_json(min_max_avg))
            },
            Self::Memory(min_max_avg) => {
                JsonPerfDatumKind::Memory(QueryMinMaxAvg::to_json(min_max_avg))
            },
            Self::Storage(min_max_avg) => {
                JsonPerfDatumKind::Storage(QueryMinMaxAvg::to_json(min_max_avg))
            },
        })
    }
}

#[derive(Debug)]
pub struct QueryLatency {
    pub lower_variance: i64,
    pub upper_variance: i64,
    pub duration:       i64,
}

impl QueryLatency {
    fn to_json(self) -> Result<JsonLatency, HttpError> {
        let Self {
            lower_variance,
            upper_variance,
            duration,
        } = self;
        Ok(JsonLatency {
            lower_variance: lower_variance as u64,
            upper_variance: upper_variance as u64,
            duration:       duration as u64,
        })
    }
}

#[derive(Debug)]
pub struct QueryThroughput {
    pub lower_variance: f64,
    pub upper_variance: f64,
    pub events:         f64,
    pub unit_time:      i64,
}

impl QueryThroughput {
    fn to_json(self) -> Result<JsonThroughput, HttpError> {
        let Self {
            lower_variance,
            upper_variance,
            events,
            unit_time,
        } = self;
        Ok(JsonThroughput {
            lower_variance: lower_variance.into(),
            upper_variance: upper_variance.into(),
            events:         events.into(),
            unit_time:      unit_time as u64,
        })
    }
}

#[derive(Debug)]
pub struct QueryMinMaxAvg {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
}

impl QueryMinMaxAvg {
    fn to_json(self) -> JsonMinMaxAvg {
        let Self { min, max, avg } = self;
        JsonMinMaxAvg {
            min: min.into(),
            max: max.into(),
            avg: avg.into(),
        }
    }
}
