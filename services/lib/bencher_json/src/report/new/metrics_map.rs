use std::collections::BTreeMap;

use super::{
    benchmarks::{JsonBenchmarks, JsonBenchmarksMap},
    latency::JsonLatency,
    median::Median,
    metrics::JsonMetrics,
    resource::JsonResource,
    throughput::JsonThroughput,
};

#[derive(Default)]
pub struct JsonMetricsMap {
    pub inner: BTreeMap<String, JsonMetricsList>,
}

impl From<JsonBenchmarks> for JsonMetricsMap {
    fn from(benchmarks: JsonBenchmarks) -> Self {
        let mut perf_list_map = Self::default();
        for benchmarks_map in benchmarks.inner.into_iter() {
            perf_list_map.append(benchmarks_map);
        }
        perf_list_map
    }
}

impl JsonMetricsMap {
    fn append(&mut self, benchmarks_map: JsonBenchmarksMap) {
        for (benchmark_name, metrics) in benchmarks_map.inner.into_iter() {
            let JsonMetrics {
                latency,
                throughput,
                compute,
                memory,
                storage,
            } = metrics;
            if let Some(metricss) = self.inner.get_mut(&benchmark_name) {
                metricss.latency.push(latency);
                metricss.throughput.push(throughput);
                metricss.compute.push(compute);
                metricss.memory.push(memory);
                metricss.storage.push(storage);
            } else {
                self.inner.insert(
                    benchmark_name,
                    JsonMetricsList {
                        latency: vec![latency],
                        throughput: vec![throughput],
                        compute: vec![compute],
                        memory: vec![memory],
                        storage: vec![storage],
                    },
                );
            }
        }
    }
}

pub struct JsonMetricsList {
    pub latency: Vec<Option<JsonLatency>>,
    pub throughput: Vec<Option<JsonThroughput>>,
    pub compute: Vec<Option<JsonResource>>,
    pub memory: Vec<Option<JsonResource>>,
    pub storage: Vec<Option<JsonResource>>,
}

impl JsonMetricsList {
    pub(crate) fn median(self) -> JsonMetrics {
        let Self {
            latency,
            throughput,
            compute,
            memory,
            storage,
        } = self;
        JsonMetrics {
            latency: JsonLatency::median(latency),
            throughput: JsonThroughput::median(throughput),
            compute: JsonResource::median(compute),
            memory: JsonResource::median(memory),
            storage: JsonResource::median(storage),
        }
    }
}
