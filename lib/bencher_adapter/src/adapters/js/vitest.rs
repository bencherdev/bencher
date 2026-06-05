use bencher_json::{BenchmarkName, JsonNewMetric, project::report::JsonAverage};
use serde::Deserialize;

use crate::{
    Adaptable, Settings,
    adapters::util::{Units, latency_as_nanos, throughput_as_secs},
    results::adapter_results::{AdapterMeasure, AdapterResults},
};

pub struct AdapterJsVitest;

impl Adaptable for AdapterJsVitest {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        serde_json::from_str::<Vitest>(input)
            .ok()?
            .convert(settings)
    }
}

/// The JSON report emitted by `vitest bench --outputJson <file>`.
/// See `createBenchmarkJsonReport` in Vitest's `benchmark/json-formatter.ts`.
#[derive(Debug, Clone, Deserialize)]
struct Vitest {
    files: Vec<File>,
}

#[derive(Debug, Clone, Deserialize)]
struct File {
    // `filepath` is intentionally ignored: it is an absolute, machine-specific
    // path and therefore not portable across CI runners. Vitest's `fullName`
    // already begins with the file's project-relative path (see `getNames` in
    // `@vitest/runner`), so benchmark names stay unique across files without it.
    groups: Vec<Group>,
}

#[derive(Debug, Clone, Deserialize)]
struct Group {
    #[serde(rename = "fullName")]
    full_name: String,
    benchmarks: Vec<Benchmark>,
}

/// A single benchmark result. This is Tinybench's `TaskResult` plus the
/// `name`/`median` fields that Vitest adds. Unused fields are ignored.
#[derive(Debug, Clone, Deserialize)]
struct Benchmark {
    name: BenchmarkName,
    /// Operations per second (throughput).
    hz: f64,
    /// Average time per operation in milliseconds (latency).
    mean: f64,
    /// Median time per operation in milliseconds (latency).
    median: f64,
    /// Standard deviation of the samples in milliseconds.
    sd: f64,
    /// Relative margin of error as a percentage.
    rme: f64,
}

impl Vitest {
    fn convert(self, settings: Settings) -> Option<AdapterResults> {
        let mut benchmark_metrics = Vec::new();
        for file in self.files {
            for group in file.groups {
                for benchmark in group.benchmarks {
                    let Benchmark {
                        name,
                        hz,
                        mean,
                        median,
                        sd,
                        rme,
                    } = benchmark;
                    let benchmark_name = combine_name(&group.full_name, name);

                    // Latency: milliseconds -> nanoseconds.
                    // For the mean, the spread is the standard deviation.
                    // Vitest does not provide an interquartile range, so the
                    // median is reported without a lower or upper value.
                    let latency = match settings.average.unwrap_or_default() {
                        JsonAverage::Mean => {
                            let value = latency_as_nanos(mean, Units::Milli);
                            let spread = latency_as_nanos(sd, Units::Milli);
                            JsonNewMetric {
                                value,
                                lower_value: Some(value - spread),
                                upper_value: Some(value + spread),
                            }
                        },
                        JsonAverage::Median => JsonNewMetric {
                            value: latency_as_nanos(median, Units::Milli),
                            lower_value: None,
                            upper_value: None,
                        },
                    };

                    // Throughput: operations per second.
                    // The bounds are derived from the relative margin of error.
                    let value = throughput_as_secs(hz, Units::Sec);
                    let error = value * (rme / 100.0);
                    let throughput = JsonNewMetric {
                        value,
                        lower_value: Some(value - error),
                        upper_value: Some(value + error),
                    };

                    benchmark_metrics.push((
                        benchmark_name,
                        vec![
                            AdapterMeasure::Latency(latency),
                            AdapterMeasure::Throughput(throughput),
                        ],
                    ));
                }
            }
        }

        AdapterResults::new_measures(benchmark_metrics)
    }
}

/// Join the suite `full_name` and the benchmark `name` into a single benchmark
/// name (e.g. `"math > fibonacci > fib(10)"`), matching the `" > "` separator
/// Vitest itself uses within `fullName`. Falls back to the leaf `name` when the
/// suite name is empty or the combined name would exceed `BenchmarkName::MAX_LEN`.
fn combine_name(full_name: &str, name: BenchmarkName) -> BenchmarkName {
    if full_name.is_empty() {
        name
    } else {
        format!("{full_name} > {name}")
            .parse::<BenchmarkName>()
            .unwrap_or(name)
    }
}

#[cfg(test)]
pub(crate) mod test_js_vitest {
    use bencher_json::project::report::JsonAverage;
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use crate::{
        AdapterResults, Settings,
        adapters::test_util::{convert_file_path, convert_file_path_median, opt_convert_file_path},
        results::adapter_metrics::AdapterMetrics,
    };

    use super::AdapterJsVitest;

    fn convert_js_vitest(suffix: &str) -> AdapterResults {
        let file_path = file_path(suffix);
        convert_file_path::<AdapterJsVitest>(&file_path)
    }

    fn convert_js_vitest_median(suffix: &str) -> AdapterResults {
        let file_path = file_path(suffix);
        convert_file_path_median::<AdapterJsVitest>(&file_path)
    }

    fn file_path(suffix: &str) -> String {
        format!("./tool_output/js/vitest/{suffix}.json")
    }

    fn validate_measure(
        metrics: &AdapterMetrics,
        slug: &str,
        value: f64,
        lower_value: Option<f64>,
        upper_value: Option<f64>,
    ) {
        let metric = metrics.get(slug).unwrap();
        assert_eq!(metric.value, OrderedFloat::from(value));
        assert_eq!(metric.lower_value, lower_value.map(OrderedFloat::from));
        assert_eq!(metric.upper_value, upper_value.map(OrderedFloat::from));
    }

    #[test]
    fn adapter_js_vitest_two() {
        let results = convert_js_vitest("two");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("math.bench.ts > math > add").unwrap();
        assert_eq!(metrics.inner.len(), 2);
        validate_measure(
            metrics,
            "latency",
            1_000_000.0,
            Some(750_000.0),
            Some(1_250_000.0),
        );
        validate_measure(metrics, "throughput", 1_000.0, Some(750.0), Some(1_250.0));

        let metrics = results.get("math.bench.ts > math > subtract").unwrap();
        assert_eq!(metrics.inner.len(), 2);
        validate_measure(
            metrics,
            "latency",
            2_000_000.0,
            Some(1_500_000.0),
            Some(2_500_000.0),
        );
        validate_measure(metrics, "throughput", 500.0, Some(375.0), Some(625.0));
    }

    #[test]
    fn adapter_js_vitest_four() {
        let four = "four";
        let file_path = file_path(four);

        let results = convert_js_vitest(four);
        validate_adapter_js_vitest(&results);

        // An explicit mean average matches the default.
        let results = opt_convert_file_path::<AdapterJsVitest>(
            &file_path,
            Settings {
                average: Some(JsonAverage::Mean),
            },
        )
        .unwrap();
        validate_adapter_js_vitest(&results);

        let results = convert_js_vitest_median(four);
        validate_adapter_js_vitest_median(&results);
    }

    pub fn validate_adapter_js_vitest(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 4);

        let metrics = results.get("fib.bench.ts > fibonacci > fib(10)").unwrap();
        assert_eq!(metrics.inner.len(), 2);
        validate_measure(
            metrics,
            "latency",
            1_000_000.0,
            Some(750_000.0),
            Some(1_250_000.0),
        );
        validate_measure(metrics, "throughput", 1_000.0, Some(750.0), Some(1_250.0));

        let metrics = results.get("fib.bench.ts > fibonacci > fib(20)").unwrap();
        validate_measure(
            metrics,
            "latency",
            2_000_000.0,
            Some(1_500_000.0),
            Some(2_500_000.0),
        );
        validate_measure(metrics, "throughput", 500.0, Some(375.0), Some(625.0));

        let metrics = results.get("sort.bench.ts > sorting > normal").unwrap();
        validate_measure(
            metrics,
            "latency",
            5_000_000.0,
            Some(3_750_000.0),
            Some(6_250_000.0),
        );
        validate_measure(metrics, "throughput", 200.0, Some(150.0), Some(250.0));

        let metrics = results.get("sort.bench.ts > sorting > reverse").unwrap();
        validate_measure(
            metrics,
            "latency",
            10_000_000.0,
            Some(7_500_000.0),
            Some(12_500_000.0),
        );
        validate_measure(metrics, "throughput", 100.0, Some(75.0), Some(125.0));
    }

    fn validate_adapter_js_vitest_median(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 4);

        // Latency uses the median with no spread (Vitest provides no IQR);
        // throughput is unaffected by the average.
        let metrics = results.get("fib.bench.ts > fibonacci > fib(10)").unwrap();
        validate_measure(metrics, "latency", 750_000.0, None, None);
        validate_measure(metrics, "throughput", 1_000.0, Some(750.0), Some(1_250.0));

        let metrics = results.get("fib.bench.ts > fibonacci > fib(20)").unwrap();
        validate_measure(metrics, "latency", 1_500_000.0, None, None);
        validate_measure(metrics, "throughput", 500.0, Some(375.0), Some(625.0));

        let metrics = results.get("sort.bench.ts > sorting > normal").unwrap();
        validate_measure(metrics, "latency", 4_000_000.0, None, None);
        validate_measure(metrics, "throughput", 200.0, Some(150.0), Some(250.0));

        let metrics = results.get("sort.bench.ts > sorting > reverse").unwrap();
        validate_measure(metrics, "latency", 8_000_000.0, None, None);
        validate_measure(metrics, "throughput", 100.0, Some(75.0), Some(125.0));
    }
}
