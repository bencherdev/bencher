use bencher_json::{BenchmarkName, JsonNewMetric, project::report::JsonAverage};
use ordered_float::OrderedFloat;
use serde::Deserialize;

use crate::{
    Adaptable, Settings,
    adapters::util::{Units, latency_as_nanos},
    results::adapter_results::AdapterResults,
};

pub struct AdapterShellHyperfineV2;

impl Adaptable for AdapterShellHyperfineV2 {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        serde_json::from_str::<Hyperfine>(input)
            .ok()?
            .convert(settings)
    }
}

// https://github.com/sharkdp/hyperfine/blob/v2.0.0/src/export/json.rs
// https://github.com/sharkdp/hyperfine/blob/v2.0.0/src/benchmark/benchmark_result.rs
#[derive(Debug, Clone, Deserialize)]
pub struct Hyperfine {
    pub results: Vec<HyperfineResult>,
}

impl Hyperfine {
    fn convert(self, settings: Settings) -> Option<AdapterResults> {
        let results = self.results;
        let mut benchmark_metrics = Vec::with_capacity(results.len());
        for result in results {
            let HyperfineResult {
                command,
                name,
                summary:
                    HyperfineSummary {
                        time_wall_clock:
                            HyperfineStats {
                                mean,
                                stddev,
                                median,
                                min,
                                max,
                            },
                    },
            } = result;

            // hyperfine 1.x exported the custom name as the command
            let benchmark_name = if let Some(name) = name {
                name
            } else {
                BenchmarkName::try_from(command).ok()?
            };

            // JSON output is always in seconds
            let units = Units::Sec;
            let (average, spread) = match settings.average.unwrap_or_default() {
                JsonAverage::Mean => (mean, stddev.map(|stddev| (mean - stddev, mean + stddev))),
                JsonAverage::Median => (median, Some((min, max))),
            };
            let value = latency_as_nanos(average, units);
            let (lower_value, upper_value) = spread.map_or((None, None), |(lower, upper)| {
                (
                    Some(latency_as_nanos(lower, units)),
                    Some(latency_as_nanos(upper, units)),
                )
            });
            let json_metric = JsonNewMetric {
                value,
                lower_value,
                upper_value,
            };

            benchmark_metrics.push((benchmark_name, json_metric));
        }

        AdapterResults::new_latency(benchmark_metrics)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct HyperfineResult {
    pub command: String,
    pub name: Option<BenchmarkName>,
    pub summary: HyperfineSummary,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HyperfineSummary {
    pub time_wall_clock: HyperfineStats,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HyperfineStats {
    pub mean: OrderedFloat<f64>,
    pub stddev: Option<OrderedFloat<f64>>,
    pub median: OrderedFloat<f64>,
    pub min: OrderedFloat<f64>,
    pub max: OrderedFloat<f64>,
}

#[cfg(test)]
pub(crate) mod test_shell_hyperfine_v2 {
    use bencher_json::BmfVersion;
    use bencher_json::project::report::JsonAverage;
    use pretty_assertions::assert_eq;

    use crate::{
        AdapterResults, Settings,
        adapters::test_util::{
            convert_file_path, convert_file_path_median, opt_convert_file_path, validate_latency,
        },
    };

    use super::AdapterShellHyperfineV2;

    fn convert_shell_hyperfine_v2(suffix: &str) -> AdapterResults {
        let file_path = file_path(suffix);
        convert_file_path::<AdapterShellHyperfineV2>(&file_path)
    }

    fn convert_shell_hyperfine_v2_median(suffix: &str) -> AdapterResults {
        let file_path = file_path(suffix);
        convert_file_path_median::<AdapterShellHyperfineV2>(&file_path)
    }

    fn file_path(suffix: &str) -> String {
        format!("./tool_output/shell/hyperfine/v2_{suffix}.json")
    }

    #[test]
    fn adapter_shell_hyperfine_v2_two() {
        let two = "two";
        let file_path = file_path(two);

        let results = convert_shell_hyperfine_v2(two);
        validate_adapter_shell_hyperfine_v2(&results);

        let results = opt_convert_file_path::<AdapterShellHyperfineV2>(
            &file_path,
            Settings::new(Some(JsonAverage::Mean), BmfVersion::default()),
        )
        .unwrap();
        validate_adapter_shell_hyperfine_v2(&results);

        let results = convert_shell_hyperfine_v2_median(two);
        validate_adapter_shell_hyperfine_v2_median(&results);
    }

    pub fn validate_adapter_shell_hyperfine_v2(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("sleep 0.1").unwrap();
        validate_latency(
            metrics,
            101_041_439.666_666_67,
            Some(100_851_314.420_046_63),
            Some(101_231_564.913_286_72),
        );

        let metrics = results.get("sleep 0.2").unwrap();
        validate_latency(
            metrics,
            201_079_106.0,
            Some(200_822_663.051_321_36),
            Some(201_335_548.948_678_67),
        );
    }

    fn validate_adapter_shell_hyperfine_v2_median(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("sleep 0.1").unwrap();
        validate_latency(
            metrics,
            101_084_345.0,
            Some(100_833_528.0),
            Some(101_206_446.0),
        );

        let metrics = results.get("sleep 0.2").unwrap();
        validate_latency(
            metrics,
            200_952_703.0,
            Some(200_910_403.000_000_03),
            Some(201_374_212.000_000_03),
        );
    }

    #[test]
    fn adapter_shell_hyperfine_v2_one() {
        let results = convert_shell_hyperfine_v2("one");
        assert_eq!(results.inner.len(), 1);

        let metrics = results.get("sleep 0.01").unwrap();
        validate_latency(
            metrics,
            10_751_696.666_666_666,
            Some(10_643_943.654_716_495),
            Some(10_859_449.678_616_837),
        );
    }

    #[test]
    fn adapter_shell_hyperfine_v2_one_median() {
        let results = convert_shell_hyperfine_v2_median("one");
        assert_eq!(results.inner.len(), 1);

        let metrics = results.get("sleep 0.01").unwrap();
        validate_latency(
            metrics,
            10_719_436.0,
            Some(10_663_759.0),
            Some(10_871_895.000_000_002),
        );
    }

    #[test]
    fn adapter_shell_hyperfine_v2_one_run() {
        let results = convert_shell_hyperfine_v2("one_run");
        assert_eq!(results.inner.len(), 1);

        let metrics = results.get("sleep 0.01").unwrap();
        validate_latency(metrics, 10_550_239.000_000_002, None, None);
    }

    #[test]
    fn adapter_shell_hyperfine_v2_one_run_median() {
        let results = convert_shell_hyperfine_v2_median("one_run");
        assert_eq!(results.inner.len(), 1);

        let metrics = results.get("sleep 0.01").unwrap();
        validate_latency(
            metrics,
            10_550_239.000_000_002,
            Some(10_550_239.000_000_002),
            Some(10_550_239.000_000_002),
        );
    }

    #[test]
    fn adapter_shell_hyperfine_v2_named() {
        let results = convert_shell_hyperfine_v2("named");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("fast").unwrap();
        validate_latency(
            metrics,
            10_936_027.666_666_668,
            Some(10_832_704.328_497_92),
            Some(11_039_351.004_835_414),
        );

        let metrics = results.get("sleep 0.02").unwrap();
        validate_latency(
            metrics,
            20_908_905.666_666_67,
            Some(20_763_329.866_901_282),
            Some(21_054_481.466_432_065),
        );
    }

    #[test]
    fn adapter_shell_hyperfine_v2_named_median() {
        let results = convert_shell_hyperfine_v2_median("named");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("fast").unwrap();
        validate_latency(
            metrics,
            10_879_109.000_000_002,
            Some(10_873_680.0),
            Some(11_055_294.0),
        );

        let metrics = results.get("sleep 0.02").unwrap();
        validate_latency(
            metrics,
            20_858_387.000_000_004,
            Some(20_795_319.0),
            Some(21_073_011.000_000_004),
        );
    }

    #[test]
    fn adapter_shell_hyperfine_v2_parameters() {
        let results = convert_shell_hyperfine_v2("parameters");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("sleep 10ms").unwrap();
        validate_latency(
            metrics,
            10_918_503.666_666_668,
            Some(10_837_508.132_830_948),
            Some(10_999_499.200_502_388),
        );

        let metrics = results.get("sleep 20ms").unwrap();
        validate_latency(
            metrics,
            20_896_575.666_666_668,
            Some(20_875_640.390_926_104),
            Some(20_917_510.942_407_228),
        );
    }

    #[test]
    fn adapter_shell_hyperfine_v2_parameters_median() {
        let results = convert_shell_hyperfine_v2_median("parameters");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("sleep 10ms").unwrap();
        validate_latency(
            metrics,
            10_933_093.000_000_002,
            Some(10_831_205.0),
            Some(10_991_213.000_000_002),
        );

        let metrics = results.get("sleep 20ms").unwrap();
        validate_latency(
            metrics,
            20_899_630.000_000_004,
            Some(20_874_281.0),
            Some(20_915_816.0),
        );
    }
}
