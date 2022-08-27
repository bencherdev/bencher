
struct Perf {
    pub id: i32,
    pub latency_id: Option<i32>,
    pub throughput_id: Option<i32>,
    pub compute_id: Option<i32>,
    pub memory_id: Option<i32>,
    pub storage_id: Option<i32>,
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


fn json_min_max_avg(conn: &SqliteConnection, id: i32) -> Option<JsonMinMaxAvg> {
    schema::min_max_avg::table
        .filter(schema::min_max_avg::id.eq(id))
        .first::<QueryMinMaxAvg>(conn)
        .map(|query| query.to_json())
        .ok()
}