use nom::IResult;

pub mod c_sharp;
pub mod cpp;
pub mod go;
pub mod java;
pub mod js;
pub mod json;
pub mod magic;
pub mod python;
pub mod ruby;
pub mod rust;
pub mod shell;
mod util;

#[allow(dead_code, clippy::print_stdout, clippy::unnecessary_wraps)]
fn print_ln(input: &str) -> IResult<&str, ()> {
    println!("--- START ---");
    println!("{input}");
    println!("---  END  ---");
    Ok((input, ()))
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
pub(crate) mod test_util {
    use bencher_json::project::{
        measure::defs::{
            generic::{Latency, Throughput},
            MeasureDefinition,
        },
        report::JsonAverage,
    };
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use crate::{
        results::{adapter_metrics::AdapterMetrics, adapter_results::AdapterResults},
        Adaptable, Settings,
    };

    pub fn convert_file_path<A>(file_path: &str) -> AdapterResults
    where
        A: Adaptable,
    {
        opt_convert_file_path::<A>(file_path, Settings::default())
            .unwrap_or_else(|| panic!("Failed to convert contents of {file_path}"))
    }

    pub fn convert_file_path_median<A>(file_path: &str) -> AdapterResults
    where
        A: Adaptable,
    {
        let settings = Settings {
            average: Some(JsonAverage::Median),
        };
        opt_convert_file_path::<A>(file_path, settings)
            .unwrap_or_else(|| panic!("Failed to convert contents of {file_path}"))
    }

    pub fn opt_convert_file_path<A>(file_path: &str, settings: Settings) -> Option<AdapterResults>
    where
        A: Adaptable,
    {
        let contents = std::fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("Failed to read test file {file_path}: {e}"));
        A::parse(&contents, settings)
    }

    pub fn validate_latency(
        metrics: &AdapterMetrics,
        value: f64,
        lower_value: Option<f64>,
        upper_value: Option<f64>,
    ) {
        validate_metric(metrics, Latency::SLUG_STR, value, lower_value, upper_value);
    }

    pub fn validate_throughput(
        metrics: &AdapterMetrics,
        value: f64,
        lower_value: Option<f64>,
        upper_value: Option<f64>,
    ) {
        validate_metric(
            metrics,
            Throughput::SLUG_STR,
            value,
            lower_value,
            upper_value,
        );
    }

    pub fn validate_metric(
        metrics: &AdapterMetrics,
        key: &str,
        value: f64,
        lower_value: Option<f64>,
        upper_value: Option<f64>,
    ) {
        assert_eq!(metrics.inner.len(), 1);
        let metric = metrics.get(key).unwrap();
        assert_eq!(metric.value, OrderedFloat::from(value));
        assert_eq!(metric.lower_value, lower_value.map(OrderedFloat::from));
        assert_eq!(metric.upper_value, upper_value.map(OrderedFloat::from));
    }
}
