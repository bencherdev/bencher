use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonAny, JsonNewMetric};
use ordered_float::OrderedFloat;
use serde::Deserialize;

use crate::{
    adapters::util::{latency_as_nanos, Units},
    results::adapter_results::AdapterResults,
    Adaptable, AdapterError, Settings,
};

pub struct AdapterShellHyperfine;

impl Adaptable for AdapterShellHyperfine {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        serde_json::from_str::<Hyperfine>(input)
            .ok()?
            .convert(settings)
            .ok()?
    }
}

// https://github.com/sharkdp/hyperfine/blob/ef4049f8f897d4adc4c47a07e60e39d9760fb9ed/src/benchmark/benchmark_result.rs#L11
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Hyperfine {
    pub results: Vec<HyperfineResult>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HyperfineResult {
    pub command: BenchmarkName,
    pub mean: OrderedFloat<f64>,
    pub stddev: Option<OrderedFloat<f64>>,
    pub median: OrderedFloat<f64>,
    pub user: f64,
    pub system: f64,
    pub min: OrderedFloat<f64>,
    pub max: OrderedFloat<f64>,
    pub times: Option<Vec<f64>>,
    pub exit_codes: Vec<Option<i32>>,
    pub parameters: Option<JsonAny>,
}

impl Hyperfine {
    #[allow(clippy::unnecessary_wraps)]
    fn convert(self, settings: Settings) -> Result<Option<AdapterResults>, AdapterError> {
        let results = self.results;
        let mut benchmark_metrics = Vec::with_capacity(results.len());
        for result in results {
            let HyperfineResult {
                command,
                mean,
                stddev,
                median,
                min,
                max,
                ..
            } = result;

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

            benchmark_metrics.push((command, json_metric));
        }

        Ok(AdapterResults::new_latency(benchmark_metrics))
    }
}

#[cfg(test)]
pub(crate) mod test_shell_hyperfine {
    use bencher_json::project::report::JsonAverage;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{
            convert_file_path, convert_file_path_median, opt_convert_file_path, validate_latency,
        },
        AdapterResults, Settings,
    };

    use super::AdapterShellHyperfine;

    fn convert_shell_hyperfine(suffix: &str) -> AdapterResults {
        let file_path = file_path(suffix);
        convert_file_path::<AdapterShellHyperfine>(&file_path)
    }

    fn convert_shell_hyperfine_median(suffix: &str) -> AdapterResults {
        let file_path = file_path(suffix);
        convert_file_path_median::<AdapterShellHyperfine>(&file_path)
    }

    fn file_path(suffix: &str) -> String {
        format!("./tool_output/shell/hyperfine/{suffix}.json")
    }

    #[test]
    fn test_adapter_shell_hyperfine_two() {
        let two = "two";
        let file_path = file_path(two);

        let results = convert_shell_hyperfine(two);
        validate_adapter_shell_hyperfine(&results);

        let results = opt_convert_file_path::<AdapterShellHyperfine>(
            &file_path,
            Settings {
                average: Some(JsonAverage::Mean),
            },
        )
        .unwrap();
        validate_adapter_shell_hyperfine(&results);

        let results = convert_shell_hyperfine_median(two);
        validate_adapter_shell_hyperfine_median(&results);
    }

    pub fn validate_adapter_shell_hyperfine(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("sleep 0.1").unwrap();
        validate_latency(
            metrics,
            107_534_464.423_703_72,
            Some(104_316_587.308_651_45),
            Some(110_752_341.538_755_98),
        );

        let metrics = results.get("sleep 0.2").unwrap();
        validate_latency(
            metrics,
            208_513_999.104_615_43,
            Some(204_785_557.656_151_62),
            Some(212_242_440.553_079_25),
        );
    }

    fn validate_adapter_shell_hyperfine_median(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("sleep 0.1").unwrap();
        validate_latency(
            metrics,
            106_525_351.72,
            Some(102_474_685.72),
            Some(115_336_892.72),
        );

        let metrics = results.get("sleep 0.2").unwrap();
        validate_latency(
            metrics,
            208_661_518.72,
            Some(201_824_142.72),
            Some(214_128_684.72),
        );
    }

    #[test]
    fn test_adapter_shell_hyperfine_one() {
        let results = convert_shell_hyperfine("one");
        assert_eq!(results.inner.len(), 1);

        let metrics = results.get("sleep 0.01").unwrap();
        validate_latency(
            metrics,
            13_317_239.025_420_565,
            Some(12_317_546.734_914_13),
            Some(14_316_931.315_926_999),
        );
    }

    #[test]
    fn test_adapter_shell_hyperfine_one_median() {
        let results = convert_shell_hyperfine_median("one");
        assert_eq!(results.inner.len(), 1);

        let metrics = results.get("sleep 0.01").unwrap();
        validate_latency(
            metrics,
            13_251_329.96,
            Some(10_165_892.459_999_999),
            Some(21_347_058.459_999_997),
        );
    }
}
