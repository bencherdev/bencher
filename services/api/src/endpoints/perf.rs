use std::{
    str::FromStr,
    sync::Arc,
    time::Duration,
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
    RequestContext,
    TypedBody,
};
use uuid::Uuid;

use crate::{
    db::{
        model::DATETIME_FORMAT,
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
    method = PUT,
    path =  "/v0/perf",
    tags = ["perf"]
}]
pub async fn put(
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

    let conn = db_connection.lock().await;
    let mut data = Vec::new();
    for branch_uuid in &branches {
        for testbed_uuid in &testbeds {
            for benchmark_uuid in &benchmarks {
                let query = schema::perf::table
                    .left_join(
                        schema::benchmark::table
                            .on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
                    )
                    .filter(schema::benchmark::uuid.eq(benchmark_uuid.to_string()))
                    .inner_join(
                        schema::report::table.on(schema::perf::report_id.eq(schema::report::id)),
                    )
                    .left_join(
                        schema::testbed::table
                            .on(schema::report::testbed_id.eq(schema::testbed::id)),
                    )
                    .filter(schema::testbed::uuid.eq(testbed_uuid.to_string()))
                    .inner_join(
                        schema::version::table
                            .on(schema::report::version_id.eq(schema::version::id)),
                    )
                    .left_join(
                        schema::branch::table.on(schema::version::branch_id.eq(schema::branch::id)),
                    )
                    .filter(schema::branch::uuid.eq(branch_uuid.to_string()));

                let query_perf_data: Vec<QueryPerfDatum> = match kind {
                    JsonPerfKind::Latency => query
                        .inner_join(
                            schema::latency::table
                                .on(schema::perf::latency_id.eq(schema::latency::id.nullable())),
                        )
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
                        .map_err(|_| http_error!(PERF_ERROR))?
                        .into_iter()
                        .map(
                            |(
                                perf_uuid,
                                start_time,
                                end_time,
                                version_number,
                                version_hash,
                                lower_variance,
                                upper_variance,
                                duration,
                            )| {
                                let datum = QueryPerfDatumKind::Latency(QueryLatency {
                                    lower_variance,
                                    upper_variance,
                                    duration,
                                });
                                QueryPerfDatum {
                                    perf_uuid,
                                    start_time,
                                    end_time,
                                    version_number,
                                    version_hash,
                                    datum,
                                }
                            },
                        )
                        .collect(),
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

                let json_perf_data = to_json(
                    branch_uuid.clone(),
                    testbed_uuid.clone(),
                    benchmark_uuid.clone(),
                    query_perf_data,
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
        CorsHeaders::new_auth("PUT".into()),
    ))
}

fn to_json(
    branch_uuid: Uuid,
    testbed_uuid: Uuid,
    benchmark_uuid: Uuid,
    perf_data: Vec<QueryPerfDatum>,
) -> Result<JsonPerfData, HttpError> {
    let mut data = Vec::new();
    for perf_datum in perf_data {
        data.push(QueryPerfDatum::to_json(perf_datum)?)
    }
    Ok(JsonPerfData {
        branch_uuid,
        testbed_uuid,
        benchmark_uuid,
        data,
    })
}

#[derive(Debug)]
pub struct QueryPerfDatum {
    pub perf_uuid:      String,
    pub start_time:     String,
    pub end_time:       String,
    pub version_number: i32,
    pub version_hash:   Option<String>,
    pub datum:          QueryPerfDatumKind,
}

impl QueryPerfDatum {
    fn to_json(self) -> Result<JsonPerfDatum, HttpError> {
        let Self {
            perf_uuid,
            start_time,
            end_time,
            version_number,
            version_hash,
            datum,
        } = self;
        Ok(JsonPerfDatum {
            perf_uuid: Uuid::from_str(&perf_uuid).map_err(|_| http_error!(PERF_ERROR))?,
            start_time: NaiveDateTime::parse_from_str(&start_time, DATETIME_FORMAT)
                .map_err(|_| http_error!(PERF_ERROR))?,
            end_time: NaiveDateTime::parse_from_str(&end_time, DATETIME_FORMAT)
                .map_err(|_| http_error!(PERF_ERROR))?,
            version_number: version_number as u32,
            version_hash,
            datum: QueryPerfDatumKind::to_json(datum)?,
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
            lower_variance: Duration::from_nanos(lower_variance as u64),
            upper_variance: Duration::from_nanos(upper_variance as u64),
            duration:       Duration::from_nanos(duration as u64),
        })
    }
}

#[derive(Debug)]
pub struct QueryThroughput {
    pub lower_events: f64,
    pub upper_events: f64,
    pub unit_time:    i64,
}

impl QueryThroughput {
    fn to_json(self) -> Result<JsonThroughput, HttpError> {
        let Self {
            lower_events,
            upper_events,
            unit_time,
        } = self;
        Ok(JsonThroughput {
            lower_events,
            upper_events,
            unit_time: Duration::from_nanos(unit_time as u64),
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
        JsonMinMaxAvg { min, max, avg }
    }
}
