use std::{collections::HashMap, str::FromStr, time::Duration};

use bencher_json::project::report::{
    metric_kind::LATENCY_SLUG,
    new::{JsonBenchmarksMap, JsonMetrics},
    JsonMetric,
};
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until1},
    character::complete::{digit1, line_ending, space1},
    combinator::{map, success},
    multi::{many0, many1},
    sequence::tuple,
    IResult,
};

use crate::{Adapter, AdapterError};

pub struct AdapterRustBench;

impl Adapter for AdapterRustBench {
    fn convert(input: &str) -> Result<JsonBenchmarksMap, AdapterError> {
        parse_rust_bench(input)
            .map(|(_, benchmarks)| benchmarks)
            .map_err(|err| AdapterError::Nom(err.map_input(Into::into)))
    }
}

enum Test {
    Ignored,
    Bench(JsonMetric),
}

fn parse_rust_bench(input: &str) -> IResult<&str, JsonBenchmarksMap> {
    map(
        tuple((
            line_ending,
            // running X test(s)
            tag("running"),
            space1,
            digit1,
            space1,
            alt((tag("tests"), tag("test"))),
            line_ending,
            // test rust::mod::path::to_test ... ignored/Y ns/iter (+/- Z)
            many0(tuple((
                tag("test"),
                space1,
                take_until1(" "),
                space1,
                tag("..."),
                space1,
                alt((
                    map(tag("ignored"), |_| Test::Ignored),
                    map(parse_bench, Test::Bench),
                )),
                line_ending,
            ))),
            line_ending,
        )),
        |(_, _, _, _, _, _, _, benches, _)| {
            let mut benchmarks = HashMap::new();
            for bench in benches {
                if let Some((benchmark, latency)) = to_latency(bench) {
                    let mut inner = HashMap::new();
                    inner.insert(LATENCY_SLUG.into(), latency);
                    benchmarks.insert(benchmark, JsonMetrics { inner });
                }
            }
            benchmarks.into()
        },
    )(input)
}

fn to_latency(
    bench: (&str, &str, &str, &str, &str, &str, Test, &str),
) -> Option<(String, JsonMetric)> {
    let (_, _, key, _, _, _, test, _) = bench;
    match test {
        Test::Ignored => None,
        Test::Bench(metric) => Some((key.into(), metric)),
    }
}

pub enum Units {
    Nano,
    Micro,
    Milli,
    Sec,
}

impl From<&str> for Units {
    fn from(time: &str) -> Self {
        match time {
            "ns" => Self::Nano,
            "μs" => Self::Micro,
            "ms" => Self::Milli,
            "s" => Self::Sec,
            _ => panic!("Unexpected time abbreviation"),
        }
    }
}

fn parse_bench(input: &str) -> IResult<&str, JsonMetric> {
    map(
        tuple((
            tag("bench:"),
            space1,
            parse_number,
            space1,
            take_until1("/"),
            tag("/iter"),
            space1,
            tag("(+/-"),
            space1,
            parse_number,
            tag(")"),
        )),
        |(_, _, duration, _, units, _, _, _, _, variance, _)| {
            let units = Units::from(units);
            let value = (to_duration(to_u64(duration), &units).as_nanos() as f64).into();
            let variance = Some((to_duration(to_u64(variance), &units).as_nanos() as f64).into());
            JsonMetric {
                value,
                lower_bound: variance,
                upper_bound: variance,
            }
        },
    )(input)
}

fn parse_number(input: &str) -> IResult<&str, Vec<(&str, &str)>> {
    many1(tuple((digit1, alt((tag(","), success(" "))))))(input)
}

fn to_u64(input: Vec<(&str, &str)>) -> u64 {
    let mut number = String::new();
    for (digit, _) in input {
        number.push_str(digit);
    }
    u64::from_str(&number).unwrap()
}

fn to_duration(time: u64, units: &Units) -> Duration {
    match units {
        Units::Nano => Duration::from_nanos(time),
        Units::Micro => Duration::from_micros(time),
        Units::Milli => Duration::from_millis(time),
        Units::Sec => Duration::from_secs(time),
    }
}

#[cfg(test)]
mod test {
    use bencher_json::project::report::{
        metric_kind::LATENCY_SLUG,
        new::{JsonBenchmarksMap, JsonMetrics},
    };
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use super::AdapterRustBench;
    use crate::Adapter;

    fn convert_rust_bench(suffix: &str) -> JsonBenchmarksMap {
        let file_path = format!("./tool_output/rust/cargo_bench_{}.txt", suffix);
        convert_file_path::<AdapterRustBench>(&file_path)
    }

    fn convert_file_path<A>(file_path: &str) -> JsonBenchmarksMap
    where
        A: Adapter,
    {
        let contents = std::fs::read_to_string(file_path)
            .expect(&format!("Failed to read test file: {file_path}"));
        A::convert(&contents).expect(&format!("Failed to convert contents: {contents}"))
    }

    fn validate_metrics(benchmarks_map: &JsonBenchmarksMap, key: &str) {
        let metrics = benchmarks_map.inner.get(key).unwrap();
        validate_metrics_inner(metrics, 3_161.0, 975.0, 975.0);
    }

    fn validate_metrics_inner(
        metrics: &JsonMetrics,
        value: f64,
        lower_bound: f64,
        upper_bound: f64,
    ) {
        assert_eq!(metrics.inner.len(), 1);
        let metric = metrics.inner.get(LATENCY_SLUG).unwrap();
        assert_eq!(metric.value, OrderedFloat::from(value));
        assert_eq!(metric.lower_bound, Some(OrderedFloat::from(lower_bound)));
        assert_eq!(metric.upper_bound, Some(OrderedFloat::from(upper_bound)));
    }

    #[test]
    fn test_adapter_rust_zero() {
        let benchmarks_map = convert_rust_bench("0");
        assert_eq!(benchmarks_map.inner.len(), 0);
    }

    #[test]
    fn test_adapter_rust_one() {
        let benchmarks_map = convert_rust_bench("1");
        assert_eq!(benchmarks_map.inner.len(), 1);
        validate_metrics(&benchmarks_map, "tests::benchmark");
    }

    #[test]
    fn test_adapter_rust_two() {
        let benchmarks_map = convert_rust_bench("2");
        assert_eq!(benchmarks_map.inner.len(), 1);
        validate_metrics(&benchmarks_map, "tests::benchmark");
    }

    #[test]
    fn test_adapter_rust_three() {
        let benchmarks_map = convert_rust_bench("3");
        assert_eq!(benchmarks_map.inner.len(), 3);
        validate_metrics(&benchmarks_map, "tests::benchmark");
        validate_metrics(&benchmarks_map, "tests::other_benchmark");
        validate_metrics(&benchmarks_map, "tests::last_benchmark");
    }

    #[test]
    fn test_adapter_rust_four() {
        let benchmarks_map = convert_rust_bench("4");
        assert_eq!(benchmarks_map.inner.len(), 2);
        validate_metrics(&benchmarks_map, "tests::benchmark");
        validate_metrics(&benchmarks_map, "tests::other_benchmark");
    }
}
