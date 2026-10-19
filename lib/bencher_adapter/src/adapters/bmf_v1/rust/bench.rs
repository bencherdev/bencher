use bencher_json::{
    BenchmarkName, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{BytesThroughput, Latency},
        },
        report::Adapter,
    },
};
use nom::{
    IResult, Parser as _,
    bytes::complete::tag,
    combinator::{eof, opt},
    sequence::delimited,
};
use serde::Deserialize;

use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::leaf::LeafResults, rust::bench::parse_cargo_line, util::parse_number_as_f64,
    },
    results::adapter_results::AdapterResults,
};

/// libtest's bench harness at version 1: its JSON events, then its text.
pub struct AdapterRustBenchV1;

impl Adaptable for AdapterRustBenchV1 {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterRustBenchJson::parse(input, settings)
            .or_else(|| AdapterRustBenchText::parse(input, settings))
    }
}

/// The bench events of nightly libtest's `--format json`.
pub struct AdapterRustBenchJson;

impl Adaptable for AdapterRustBenchJson {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut results = LeafResults::default();
        for line in input.lines() {
            let Ok(Event::Bench {
                name,
                median,
                deviation,
                mib_per_second,
            }) = serde_json::from_str(line.trim())
            else {
                continue;
            };
            let Ok(benchmark_name) = name.parse() else {
                results.skip_benchmark();
                continue;
            };
            insert(
                &mut results,
                benchmark_name,
                median,
                deviation,
                mib_per_second,
            );
        }
        results.finish(Adapter::RustBenchJson)
    }
}

/// libtest's bench lines, `test NAME ... bench: N ns/iter (+/- R)`, with
/// ` = M MB/s` when the benchmark sets its bytes.
pub struct AdapterRustBenchText;

impl Adaptable for AdapterRustBenchText {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut results = LeafResults::default();
        for line in input.lines() {
            if let Ok((_, ((benchmark_name, median, range), mb_per_second, _))) =
                (parse_cargo_line, opt(parse_mb_per_second), eof).parse(line)
            {
                insert(
                    &mut results,
                    benchmark_name,
                    median.into_inner(),
                    range.into_inner(),
                    mb_per_second,
                );
            }
        }
        results.finish(Adapter::RustBenchText)
    }
}

/// libtest's spread is the maximum less the minimum of its samples, so a range,
/// and its rate is the bytes per iteration over the median, in decimal megabytes,
/// whichever name the JSON gives it.
fn insert(
    results: &mut LeafResults,
    benchmark_name: BenchmarkName,
    median: f64,
    range: f64,
    mb_per_second: Option<f64>,
) {
    let mut variant = results.variant(benchmark_name, ParameterSet::default());
    variant.insert(Latency::name_id(), "median", median);
    variant.insert(Latency::name_id(), "range", range);
    if let Some(mb_per_second) = mb_per_second {
        variant.insert(
            BytesThroughput::name_id(),
            "median",
            mb_per_second * BYTES_PER_MB,
        );
    }
}

const BYTES_PER_MB: f64 = 1_000_000.0;

fn parse_mb_per_second(input: &str) -> IResult<&str, f64> {
    delimited(tag(" = "), parse_number_as_f64, tag(" MB/s")).parse(input)
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event {
    Bench {
        name: String,
        median: f64,
        deviation: f64,
        mib_per_second: Option<f64>,
    },
    #[serde(other)]
    Other,
}

#[cfg(test)]
mod test_rust_bench_v1 {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{BytesThroughput, Latency},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{assert_ignores_average, assert_variant, convert_v1},
    };

    use super::AdapterRustBenchJson;

    fn latency(median: f64, range: f64) -> (&'static str, Vec<(&'static str, f64)>) {
        (
            Latency::SLUG_STR,
            vec![("median", median), ("range", range)],
        )
    }

    fn bytes(median: f64) -> (&'static str, Vec<(&'static str, f64)>) {
        (BytesThroughput::SLUG_STR, vec![("median", median)])
    }

    /// Kills a leaf that drops a line with a rate, promotes the median to `value`,
    /// keeps the version 0 band, reads the rate as mebibytes, or records no leaf.
    #[test]
    fn adapter_rust_bench_v1_text_bytes() {
        for adapter in [Adapter::RustBench, Adapter::RustBenchText] {
            let results = convert_v1(adapter, "rust/bench/bytes.txt");
            assert_eq!(
                results.resolved_adapter,
                Some(Adapter::RustBenchText),
                "{adapter}"
            );
            assert_eq!(results.inner.len(), 3, "{adapter}");
            assert_variant(
                &results,
                "checksum_64_kib",
                "{}",
                &[latency(3_562.86, 230.89), bytes(18_398_000_000.0)],
            );
            assert_variant(
                &results,
                "copy_4_kib",
                "{}",
                &[latency(62.5, 1.22), bytes(66_064_000_000.0)],
            );
            assert_variant(&results, "fib_10", "{}", &[latency(92.99, 9.81)]);
        }
    }

    /// Kills a leaf that misreads libtest's thousands separators.
    #[test]
    fn adapter_rust_bench_v1_text_many() {
        let results = convert_v1(Adapter::RustBenchText, "rust/bench/many.txt");
        assert_eq!(results.inner.len(), 6);
        assert_variant(
            &results,
            "tests::three_digit",
            "{}",
            &[latency(333_000_000_000.0, 333_000_000_000.0)],
        );
        assert_variant(
            &results,
            "tests::benchmark",
            "{}",
            &[latency(3_161.0, 975.0)],
        );
    }

    /// Kills a JSON leaf that reads `deviation` as anything but a range, drops
    /// `mib_per_second`, reads it as mebibytes, or records no leaf.
    #[test]
    fn adapter_rust_bench_v1_json() {
        for adapter in [Adapter::RustBench, Adapter::RustBenchJson] {
            let results = convert_v1(adapter, "rust/bench/json.txt");
            assert_eq!(
                results.resolved_adapter,
                Some(Adapter::RustBenchJson),
                "{adapter}"
            );
            assert_eq!(results.inner.len(), 3, "{adapter}");
            assert_variant(
                &results,
                "checksum_64_kib",
                "{}",
                &[
                    latency(3_550.331_834_532_374, 320.343_291_366_906_2),
                    bytes(18_460_000_000.0),
                ],
            );
            assert_variant(
                &results,
                "copy_4_kib",
                "{}",
                &[
                    latency(61.746_931_362_725_45, 0.816_561_873_747_495_8),
                    bytes(67_147_000_000.0),
                ],
            );
            assert_variant(
                &results,
                "fib_10",
                "{}",
                &[latency(93.609_721_847_150_95, 15.895_594_788_009_703)],
            );
        }
    }

    /// Kills a node that tries the text leaf before the JSON leaf.
    #[test]
    fn adapter_rust_bench_v1_tries_json_first() {
        let results = Adapter::RustBench
            .convert(&concatenated(["json.txt", "bytes.txt"]), v1())
            .expect("Failed to parse");
        assert_eq!(results.resolved_adapter, Some(Adapter::RustBenchJson));
    }

    /// Kills a JSON leaf that skips a name that is not a valid benchmark name without
    /// counting it.
    #[test]
    fn adapter_rust_bench_v1_json_invalid_name() {
        let results = AdapterRustBenchJson::parse(
            concat!(
                r#"{ "type": "bench", "name": "", "median": 1.0, "deviation": 0.0 }"#,
                "\n",
                r#"{ "type": "bench", "name": "fib_10", "median": 2.0, "deviation": 0.0 }"#,
            ),
            v1(),
        )
        .expect("Failed to parse");
        assert_eq!(results.inner.len(), 1);
        assert_eq!(results.skipped.benchmarks, 1);
    }

    fn concatenated(fixtures: [&str; 2]) -> String {
        fixtures
            .map(|fixture| {
                std::fs::read_to_string(format!("./tool_output/rust/bench/{fixture}"))
                    .expect("Failed to read a fixture")
            })
            .join("\n")
    }

    fn v1() -> Settings {
        Settings::new(None, BmfVersion::V1)
    }

    /// Kills a text leaf that reads the average setting.
    #[test]
    fn adapter_rust_bench_v1_text_ignores_average() {
        for suffix in ["rust/bench/bytes.txt", "rust/bench/many.txt"] {
            assert_ignores_average(Adapter::RustBenchText, suffix);
        }
    }

    /// Kills a JSON leaf that reads the average setting.
    #[test]
    fn adapter_rust_bench_v1_json_ignores_average() {
        assert_ignores_average(Adapter::RustBenchJson, "rust/bench/json.txt");
    }
}
