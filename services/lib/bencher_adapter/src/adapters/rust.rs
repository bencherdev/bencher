use std::{collections::HashMap, str::FromStr, time::Duration};

use bencher_json::project::report::{
    metric_kind::LATENCY_SLUG,
    new::{JsonBenchmarksMap, JsonMetrics},
    JsonMetric,
};
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until1},
    character::complete::{anychar, digit1, line_ending, space1},
    combinator::{map, success},
    multi::{fold_many1, many0, many1, many_till},
    sequence::{delimited, tuple},
    IResult,
};

use crate::{Adapter, AdapterError};

pub struct AdapterRust;

impl Adapter for AdapterRust {
    fn convert(input: &str) -> Result<JsonBenchmarksMap, AdapterError> {
        parse_rust_bench(input)
            .map(|(_, benchmarks)| benchmarks)
            .map_err(|err| AdapterError::Nom(err.map_input(Into::into)))
    }
}

enum Test {
    Ignored,
    Failed,
    Bench(JsonMetric),
    Ok(JsonMetric),
}

fn parse_rust_bench(input: &str) -> IResult<&str, JsonBenchmarksMap> {
    fold_many1(parse_running, HashMap::new, |benchmarks, new_benchmarks| {
        benchmarks.into_iter().chain(new_benchmarks).collect()
    })(input)
    .map(|(remainder, benchmarks)| (remainder, benchmarks.into()))
}

fn parse_running(input: &str) -> IResult<&str, HashMap<String, JsonMetrics>> {
    map(
        tuple((
            alt((
                // A non-initial multi-target run
                map(
                    tuple((
                        many0(line_ending),
                        space1,
                        alt((tag("Doc-tests"), tag("Running"))),
                        many_till(anychar, line_ending),
                        many0(line_ending),
                    )),
                    |_| (),
                ),
                // The start of a run
                map(many0(line_ending), |_| ()),
            )),
            // running X test(s)
            tuple((
                tag("running"),
                space1,
                digit1,
                space1,
                alt((tag("tests"), tag("test"))),
                line_ending,
            )),
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
                    map(tag("FAILED"), |_| Test::Failed),
                    map(parse_bench, Test::Bench),
                    map(parse_ok, Test::Ok),
                )),
                line_ending,
            ))),
            tuple((
                many0(line_ending),
                tag("test result:"),
                many_till(anychar, line_ending),
                many0(line_ending),
            )),
        )),
        |(_, _, benches, _)| {
            let mut benchmarks = HashMap::new();
            for bench in benches {
                if let Some((benchmark, latency)) = to_latency(bench) {
                    let mut inner = HashMap::new();
                    inner.insert(LATENCY_SLUG.into(), latency);
                    benchmarks.insert(benchmark, JsonMetrics { inner });
                }
            }
            benchmarks
        },
    )(input)
}

fn to_latency(
    bench: (&str, &str, &str, &str, &str, &str, Test, &str),
) -> Option<(String, JsonMetric)> {
    let (_, _, key, _, _, _, test, _) = bench;
    match test {
        Test::Ignored => None,
        // TODO add an error on failure feature
        Test::Failed => None,
        Test::Ok(metric) | Test::Bench(metric) => Some((key.into(), metric)),
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

impl Units {
    fn as_nanos(&self) -> usize {
        match self {
            Self::Nano => 1,
            Self::Micro => 1_000,
            Self::Milli => 1_000_000,
            Self::Sec => 1_000_000_000,
        }
    }
}

// cargo bench
// TODO cargo test -- -Z unstable-options --format json
fn parse_bench(input: &str) -> IResult<&str, JsonMetric> {
    map(
        tuple((
            tag("bench:"),
            space1,
            parse_int,
            space1,
            take_until1("/"),
            tag("/iter"),
            space1,
            delimited(tag("("), tuple((tag("+/-"), space1, parse_int)), tag(")")),
        )),
        |(_, _, duration, _, units, _, _, (_, _, variance))| {
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

fn parse_int(input: &str) -> IResult<&str, Vec<(&str, &str)>> {
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

// cargo test -- -Z unstable-options --report-time
// TODO cargo test -- -Z unstable-options --format json --report-time
fn parse_ok(input: &str) -> IResult<&str, JsonMetric> {
    map(
        tuple((
            tag("ok"),
            space1,
            delimited(tag("<"), tuple((parse_float, take_until1(">"))), tag(">")),
        )),
        |(_, _, (duration, units))| {
            let units = Units::from(units);
            let value = to_f64(duration) * units.as_nanos() as f64;
            JsonMetric {
                value: value.into(),
                lower_bound: None,
                upper_bound: None,
            }
        },
    )(input)
}

fn parse_float(input: &str) -> IResult<&str, Vec<&str>> {
    fold_many1(
        alt((digit1, tag("."))),
        Vec::new,
        |mut float_chars, float_char| {
            float_chars.push(float_char);
            float_chars
        },
    )(input)
}

fn to_f64(input: Vec<&str>) -> f64 {
    let mut number = String::new();
    for floating_point in input {
        number.push_str(floating_point);
    }
    f64::from_str(&number).unwrap()
}

#[cfg(test)]
mod test {
    use bencher_json::project::report::{
        metric_kind::LATENCY_SLUG,
        new::{JsonBenchmarksMap, JsonMetrics},
    };
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use super::AdapterRust;
    use crate::Adapter;

    fn convert_rust_bench(suffix: &str) -> JsonBenchmarksMap {
        let file_path = format!("./tool_output/rust/cargo_bench_{}.txt", suffix);
        convert_file_path::<AdapterRust>(&file_path)
    }

    fn convert_rust_test(suffix: &str) -> JsonBenchmarksMap {
        let file_path = format!("./tool_output/rust/cargo_test_{}.txt", suffix);
        convert_file_path::<AdapterRust>(&file_path)
    }

    fn convert_file_path<A>(file_path: &str) -> JsonBenchmarksMap
    where
        A: Adapter,
    {
        let contents = std::fs::read_to_string(file_path)
            .expect(&format!("Failed to read test file: {file_path}"));
        A::convert(&contents).expect(&format!("Failed to convert contents: {contents}"))
    }

    fn validate_defacto_metrics(benchmarks_map: &JsonBenchmarksMap, key: &str) {
        let metrics = benchmarks_map.inner.get(key).unwrap();
        validate_metrics(metrics, 3_161.0, Some(975.0), Some(975.0));
    }

    fn validate_metrics(
        metrics: &JsonMetrics,
        value: f64,
        lower_bound: Option<f64>,
        upper_bound: Option<f64>,
    ) {
        assert_eq!(metrics.inner.len(), 1);
        let metric = metrics.inner.get(LATENCY_SLUG).unwrap();
        assert_eq!(metric.value, OrderedFloat::from(value));
        assert_eq!(metric.lower_bound, lower_bound.map(OrderedFloat::from));
        assert_eq!(metric.upper_bound, upper_bound.map(OrderedFloat::from));
    }

    #[test]
    fn test_adapter_rust_zero() {
        let benchmarks_map = convert_rust_bench("zero");
        assert_eq!(benchmarks_map.inner.len(), 0);
    }

    #[test]
    fn test_adapter_rust_one() {
        let benchmarks_map = convert_rust_bench("one");
        assert_eq!(benchmarks_map.inner.len(), 1);
        validate_defacto_metrics(&benchmarks_map, "tests::benchmark");
    }

    #[test]
    fn test_adapter_rust_ignore() {
        let benchmarks_map = convert_rust_bench("ignore");
        assert_eq!(benchmarks_map.inner.len(), 1);
        validate_defacto_metrics(&benchmarks_map, "tests::benchmark");
    }

    #[test]
    fn test_adapter_rust_many() {
        let benchmarks_map = convert_rust_bench("many");
        assert_eq!(benchmarks_map.inner.len(), 6);
        validate_defacto_metrics(&benchmarks_map, "tests::benchmark");
        validate_defacto_metrics(&benchmarks_map, "tests::other_benchmark");
        validate_defacto_metrics(&benchmarks_map, "tests::last_benchmark");

        let number = 1_000.0;
        let metrics = benchmarks_map.inner.get("tests::one_digit").unwrap();
        validate_metrics(metrics, number, Some(number), Some(number));

        let number = 22_000_000.0;
        let metrics = benchmarks_map.inner.get("tests::two_digit").unwrap();
        validate_metrics(metrics, number, Some(number), Some(number));

        let number = 333_000_000_000.0;
        let metrics = benchmarks_map.inner.get("tests::three_digit").unwrap();
        validate_metrics(metrics, number, Some(number), Some(number));
    }

    #[test]
    fn test_adapter_rust_multi_target() {
        let benchmarks_map = convert_rust_bench("multi_target");
        assert_eq!(benchmarks_map.inner.len(), 2);
        validate_defacto_metrics(&benchmarks_map, "tests::benchmark");
        validate_defacto_metrics(&benchmarks_map, "tests::other_benchmark");
    }

    #[test]
    fn test_adapter_rust_report_time() {
        let benchmarks_map = convert_rust_test("report_time");
        assert_eq!(benchmarks_map.inner.len(), 3);

        let metrics = benchmarks_map.inner.get("tests::unit_test").unwrap();
        validate_metrics(metrics, 0.0, None, None);

        let metrics = benchmarks_map.inner.get("tests::other_test").unwrap();
        validate_metrics(metrics, 1_000_000.0, None, None);

        let metrics = benchmarks_map.inner.get("tests::last_test").unwrap();
        validate_metrics(metrics, 2_000_000.0, None, None);
    }
}
