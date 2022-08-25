use bencher_json::report::{
    JsonLatency,
    JsonMinMaxAvg,
    JsonReportAlert,
    JsonReportAlerts,
    JsonThroughput,
};
use chrono::offset::Utc;
use diesel::{
    expression_methods::BoolExpressionMethods,
    JoinOnDsl,
    QueryDsl,
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::HttpError;
use uuid::Uuid;

use super::{
    latency::QueryLatency,
    min_max_avg::QueryMinMaxAvg,
    throughput::QueryThroughput,
};
use crate::{
    db::{
        model::threshold::{
            alert::InsertAlert,
            statistic::StatisticKind,
        },
        schema,
        schema::statistic as statistic_table,
    },
    diesel::ExpressionMethods,
    util::http_error,
};

const PERF_ERROR: &str = "Failed to create perf statistic.";

pub struct PerfThreshold {
    pub branch_id:    i32,
    pub testbed_id:   i32,
    pub threshold_id: i32,
    pub statistic:    Statistic,
}

pub struct Statistic {
    pub id:          i32,
    pub uuid:        String,
    pub kind:        StatisticKind,
    pub sample_size: i64,
    pub window:      i64,
    pub left_side:   Option<f32>,
    pub right_side:  Option<f32>,
}

struct Perf {
    pub id: i32,
    pub latency_id: Option<i32>,
    pub throughput_id: Option<i32>,
    pub compute_id: Option<i32>,
    pub memory_id: Option<i32>,
    pub storage_id: Option<i32>,
}

impl PerfThreshold {
    pub fn new(
        conn: &SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
    ) -> Result<Self, HttpError> {
        let perf_threshold = schema::statistic::table
            .inner_join(
                schema::threshold::table
                    .on(schema::statistic::id.eq(schema::threshold::statistic_id)),
            )
            .filter(
                schema::threshold::branch_id
                    .eq(branch_id)
                    .and(schema::threshold::testbed_id.eq(testbed_id)),
            )
            .select((
                schema::threshold::id,
                schema::statistic::id,
                schema::statistic::uuid,
                schema::statistic::kind,
                schema::statistic::sample_size,
                schema::statistic::window,
                schema::statistic::left_side,
                schema::statistic::right_side,
            ))
            .first::<(
                i32,
                i32,
                String,
                i32,
                Option<i64>,
                Option<i64>,
                Option<f32>,
                Option<f32>,
            )>(conn)
            .map(
                |(threshold_id, id, uuid, kind, sample_size, window, left_side, right_side)| -> Result<PerfThreshold, HttpError> {
                    let statistic = Statistic {
                        id,
                        uuid,
                        kind: kind.try_into()?,
                        sample_size: unwrap_sample_size(sample_size),
                        window: unwrap_window(window),
                        left_side,
                        right_side,
                    };
                    Ok(Self {
                        branch_id,
                        testbed_id,
                        threshold_id,
                        statistic,
                    })
                },
            )
            .map_err(|_| http_error!(PERF_ERROR))??;

        Ok(perf_threshold)
    }

    pub fn alerts(
        &self,
        conn: &SqliteConnection,
        benchmark_id: i32,
    ) -> Result<PerfAlerts, HttpError> {
        let alerts = PerfAlerts::new();

        let perfs: Vec<Perf> = schema::perf::table
            .left_join(
                schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
            )
            .filter(schema::benchmark::id.eq(benchmark_id))
            .left_join(schema::report::table.on(schema::perf::report_id.eq(schema::report::id)))
            .filter(schema::report::start_time.ge(self.statistic.window))
            .left_join(
                schema::testbed::table.on(schema::report::testbed_id.eq(schema::testbed::id)),
            )
            .filter(schema::testbed::id.eq(self.testbed_id))
            .left_join(
                schema::version::table.on(schema::report::version_id.eq(schema::version::id)),
            )
            .left_join(schema::branch::table.on(schema::version::branch_id.eq(schema::branch::id)))
            .filter(schema::branch::id.eq(self.branch_id))
            .select((
                schema::perf::id,
                schema::perf::latency_id,
                schema::perf::throughput_id,
                schema::perf::compute_id,
                schema::perf::memory_id,
                schema::perf::storage_id,
            ))
            .order((
                schema::version::number.desc(),
                schema::report::start_time.desc(),
                schema::perf::iteration.desc(),
            ))
            .limit(self.statistic.sample_size)
            .load::<(
                i32,
                Option<i32>,
                Option<i32>,
                Option<i32>,
                Option<i32>,
                Option<i32>,
            )>(conn)
            .map_err(|_| http_error!(PERF_ERROR))?
            .into_iter()
            .map(
                |(id, latency_id, throughput_id, compute_id, memory_id, storage_id)| Perf {
                    id,
                    latency_id,
                    throughput_id,
                    compute_id,
                    memory_id,
                    storage_id,
                },
            )
            .collect();

        let mut perf_json = PerfJson::default();
        for perf in &perfs {
            perf_json.push(conn, perf);
        }

        // TODO use perf_json value to calculate the standard deviation and threshold
        // bounds. Then use those to generate alert(s)

        Ok(alerts)
    }
}

fn unwrap_sample_size(sample_size: Option<i64>) -> i64 {
    sample_size.unwrap_or(i64::MAX)
}

fn unwrap_window(window: Option<i64>) -> i64 {
    window
        .map(|window| {
            let now = Utc::now().timestamp_nanos();
            now - window
        })
        .unwrap_or_default()
}

fn json_min_max_avg(conn: &SqliteConnection, id: i32) -> Option<JsonMinMaxAvg> {
    schema::min_max_avg::table
        .filter(schema::min_max_avg::id.eq(id))
        .first::<QueryMinMaxAvg>(conn)
        .map(|query| query.to_json())
        .ok()
}

#[derive(Default)]
struct PerfJson {
    pub latency:    Vec<JsonLatency>,
    pub throughput: Vec<JsonThroughput>,
    pub compute:    Vec<JsonMinMaxAvg>,
    pub memory:     Vec<JsonMinMaxAvg>,
    pub storage:    Vec<JsonMinMaxAvg>,
}

impl PerfJson {
    fn push(&mut self, conn: &SqliteConnection, perf: &Perf) {
        if let Some(id) = perf.latency_id {
            if let Ok(Ok(json)) = schema::latency::table
                .filter(schema::latency::id.eq(id))
                .first::<QueryLatency>(conn)
                .map(|query| query.to_json())
            {
                self.latency.push(json);
            }
        }
        if let Some(id) = perf.throughput_id {
            if let Ok(Ok(json)) = schema::throughput::table
                .filter(schema::throughput::id.eq(id))
                .first::<QueryThroughput>(conn)
                .map(|query| query.to_json())
            {
                self.throughput.push(json);
            }
        }
        if let Some(id) = perf.compute_id {
            if let Some(json) = json_min_max_avg(conn, id) {
                self.compute.push(json);
            }
        }
        if let Some(id) = perf.memory_id {
            if let Some(json) = json_min_max_avg(conn, id) {
                self.memory.push(json);
            }
        }
        if let Some(id) = perf.storage_id {
            if let Some(json) = json_min_max_avg(conn, id) {
                self.storage.push(json);
            }
        }
    }
}

pub type PerfAlerts = Vec<PerfAlert>;

pub struct PerfAlert {
    pub threshold_id: i32,
    pub statistic_id: i32,
    pub side:         bool,
    pub boundary:     f64,
    pub outlier:      f64,
}

impl PerfAlert {
    pub fn into_report_alert(
        self,
        conn: &SqliteConnection,
        perf_id: i32,
    ) -> Result<JsonReportAlert, HttpError> {
        let Self {
            threshold_id,
            statistic_id,
            side,
            boundary,
            outlier,
        } = self;
        let uuid = Uuid::new_v4();
        let insert_alert = InsertAlert {
            uuid: uuid.to_string(),
            perf_id,
            threshold_id,
            statistic_id,
            side,
            boundary,
            outlier,
        };

        diesel::insert_into(schema::alert::table)
            .values(&insert_alert)
            .execute(conn)
            .map_err(|_| http_error!(PERF_ERROR))?;

        Ok(uuid.into())
    }
}
