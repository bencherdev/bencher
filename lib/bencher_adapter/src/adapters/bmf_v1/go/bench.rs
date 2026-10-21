use std::collections::BTreeMap;

use bencher_json::{
    BenchmarkName, MeasureNameId, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Allocations, BytesThroughput, Latency},
            dotnet::Allocated,
        },
        report::Adapter,
    },
};
use serde::Deserialize;

use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::{
            leaf::{LeafResults, Variant},
            runs::insert_runs,
        },
        util::Units,
    },
    results::adapter_results::AdapterResults,
};

/// Go's benchmarks at version 1: `go test -json`, then the text of `go test -bench`.
pub struct AdapterGoBenchV1;

impl Adaptable for AdapterGoBenchV1 {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterGoBenchJson::parse(input, settings)
            .or_else(|| AdapterGoBenchText::parse(input, settings))
    }
}

/// The output events of `go test -json`, which interlace across packages, each package's
/// output joined back into its text.
pub struct AdapterGoBenchJson;

impl Adaptable for AdapterGoBenchJson {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut packages: Vec<(String, String)> = Vec::new();
        for line in input.lines() {
            let Ok(Event {
                action: Action::Output,
                package: Some(package),
                output: Some(output),
            }) = serde_json::from_str(line.trim())
            else {
                continue;
            };
            if let Some((_, text)) = packages.iter_mut().find(|(name, _)| *name == package) {
                text.push_str(&output);
            } else {
                packages.push((package, output));
            }
        }
        let mut runs = Runs::default();
        for (_, text) in &packages {
            runs.read(text);
        }
        runs.finish(Adapter::GoBenchJson)
    }
}

/// The text of `go test -bench`: `pkg:` lines, then `Name-N iterations value unit...` lines.
pub struct AdapterGoBenchText;

impl Adaptable for AdapterGoBenchText {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut runs = Runs::default();
        runs.read(input);
        runs.finish(Adapter::GoBenchText)
    }
}

/// Every run of each variant, so that the runs of `-count` aggregate.
#[derive(Default)]
struct Runs {
    results: LeafResults,
    variants: BTreeMap<(BenchmarkName, ParameterSet), Quantities>,
}

impl Runs {
    /// The benchmark lines of `text`, each named for the package of its block's `pkg:` line.
    fn read(&mut self, text: &str) {
        let mut package = None;
        for line in text.lines() {
            if line.starts_with("goos:") {
                package = None;
            } else if let Some(pkg) = line.strip_prefix("pkg:") {
                package = Some(pkg.trim());
            } else if let Some((name, values)) = parse_line(line) {
                self.insert(package, name, values);
            }
        }
    }

    /// A line's first time per operation is latency, which is `ns/op` whenever the line has it,
    /// as Go prints `ns/op` first, and any other is a measure of its own.
    fn insert(&mut self, package: Option<&str>, name: &str, values: Values<'_>) {
        let (name, parameters, skipped) = name_and_parameters(name);
        let name = match package {
            Some(package) => format!("{package}.{name}"),
            None => name,
        };
        let Ok(benchmark_name) = name.parse() else {
            self.results.skip_benchmark();
            return;
        };
        let Ok(parameters) = serde_json::from_value(serde_json::Value::Object(parameters)) else {
            self.results.skip_variant();
            return;
        };
        for _ in 0..skipped {
            self.results.skip_variant();
        }
        let latency = values
            .iter()
            .map(|(_, unit)| *unit)
            .find(|unit| nanos_per_op(unit).is_some());
        let quantities = self
            .variants
            .entry((benchmark_name, parameters))
            .or_default();
        for (value, unit) in values {
            if Some(unit) == latency
                && let Some(nanos) = nanos_per_op(unit)
            {
                quantities
                    .entry(Quantity::Latency)
                    .or_default()
                    .push(value.map(|value| value * nanos));
            } else {
                quantities
                    .entry(Quantity::Unit(unit.to_owned()))
                    .or_default()
                    .push(value);
            }
        }
    }

    /// One run's number is a mean when Go reports it per operation or as a rate, else a single
    /// value, and the runs of `-count` aggregate; a value that is not finite is skipped and
    /// counted.
    fn finish(self, adapter: Adapter) -> Option<AdapterResults> {
        let Self {
            mut results,
            variants,
        } = self;
        for ((benchmark_name, parameters), quantities) in variants {
            let mut variant = results.variant(benchmark_name, parameters);
            for (quantity, values) in quantities {
                let finite = values.iter().flatten().copied().collect::<Vec<_>>();
                for _ in finite.len()..values.len() {
                    variant.skip_metric();
                }
                if finite.is_empty() {
                    continue;
                }
                let (measure, single, factor) = match quantity {
                    Quantity::Latency => (Latency::name_id(), "mean", 1.0),
                    Quantity::Unit(unit) => {
                        let Some((measure, factor)) = measure_of(&mut variant, &unit) else {
                            continue;
                        };
                        let single = if unit.ends_with("/op") || unit == MB_PER_SECOND {
                            "mean"
                        } else {
                            "value"
                        };
                        (measure, single, factor)
                    },
                };
                let values = finite.into_iter().map(|value| value * factor).collect();
                insert_runs(&mut variant, measure, single, values);
            }
        }
        results.finish(adapter)
    }
}

/// Each run's value of each quantity a variant reports, `None` where it is not finite.
type Quantities = BTreeMap<Quantity, Vec<Option<f64>>>;

/// What a value of a line measures: latency in nanoseconds, or a unit as Go writes it.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Quantity {
    Latency,
    Unit(String),
}

/// The values of one line, each with its unit, `None` where it is not finite.
type Values<'a> = Vec<(Option<f64>, &'a str)>;

/// A benchmark line's name and each value with its unit, `None` for a value that is not finite,
/// or `None` for any other line.
fn parse_line(line: &str) -> Option<(&str, Values<'_>)> {
    let mut fields = line.split_whitespace();
    let name = fields.next().filter(|name| name.starts_with("Benchmark"))?;
    fields.next()?.parse::<u64>().ok()?;
    let fields = fields.collect::<Vec<_>>();
    if fields.is_empty() {
        return None;
    }
    let values = fields
        .chunks(2)
        .map(|pair| {
            let [value, unit] = pair else {
                return None;
            };
            let value = value.parse::<f64>().ok()?;
            Some((value.is_finite().then_some(value), *unit))
        })
        .collect::<Option<Vec<_>>>()?;
    Some((name, values))
}

/// The name without its `-N` GOMAXPROCS suffix and its `key=value` parts, which become
/// parameters: `@gomaxprocs`, 1 when Go prints no suffix, and each key with a value that is a
/// JSON number as a number, with the number of parts skipped. Adapter keys start with `@`,
/// following JSON-LD's convention for keywords, and a part with the key `@gomaxprocs` is skipped.
fn name_and_parameters(name: &str) -> (String, serde_json::Map<String, serde_json::Value>, usize) {
    let mut parameters = serde_json::Map::new();
    let name = if let Some((name, procs)) = name.rsplit_once('-')
        && !procs.is_empty()
        && procs.bytes().all(|byte| byte.is_ascii_digit())
        && let Ok(procs) = procs.parse::<u64>()
    {
        parameters.insert(GOMAXPROCS.to_owned(), procs.into());
        name
    } else {
        parameters.insert(GOMAXPROCS.to_owned(), 1.into());
        name
    };
    let mut parts = Vec::new();
    let mut skipped = 0;
    for (index, part) in name.split('/').enumerate() {
        if index > 0
            && let Some((key, value)) = part.split_once('=')
            && !key.is_empty()
        {
            if key == GOMAXPROCS {
                skipped += 1;
                continue;
            }
            let value = serde_json::from_str::<serde_json::Number>(value)
                .map_or_else(|_| value.into(), serde_json::Value::Number);
            parameters.insert(key.to_owned(), value);
        } else {
            parts.push(part);
        }
    }
    (parts.join("/"), parameters, skipped)
}

const GOMAXPROCS: &str = "@gomaxprocs";

/// The measure a Go unit other than latency picks and the factor to its unit: `B/op` allocated
/// bytes, `allocs/op` allocations, `MB/s` bytes per second, and any other unit a measure of its
/// own name in that unit, or `None`, counted as skipped, when the unit makes no measure name.
fn measure_of(variant: &mut Variant<'_>, unit: &str) -> Option<(MeasureNameId, f64)> {
    match unit {
        "B/op" => Some((
            variant.named(Allocated::SLUG_STR, Allocated::UNITS_STR)?,
            1.0,
        )),
        "allocs/op" => Some((Allocations::name_id(), 1.0)),
        MB_PER_SECOND => Some((BytesThroughput::name_id(), BYTES_PER_MB)),
        _ => Some((variant.named(unit, unit)?, 1.0)),
    }
}

/// The nanoseconds in one unit of a time per operation.
fn nanos_per_op(unit: &str) -> Option<f64> {
    unit.strip_suffix("/op")?
        .parse::<Units>()
        .ok()
        .map(Units::as_nanos)
}

const MB_PER_SECOND: &str = "MB/s";
const BYTES_PER_MB: f64 = 1_000_000.0;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Event {
    action: Action,
    package: Option<String>,
    output: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Action {
    Output,
    #[serde(other)]
    Other,
}

#[cfg(test)]
mod test_go_bench_v1 {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Allocations, BytesThroughput, Latency},
                dotnet::Allocated,
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;
    use std::f64::consts::SQRT_2;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_units, assert_variant, convert_v1, variants,
        },
    };

    use super::AdapterGoBenchText;

    fn latency(mean: f64) -> Expected {
        (Latency::SLUG_STR, vec![("mean", mean)])
    }

    fn mean(measure: &'static str, mean: f64) -> Expected {
        (measure, vec![("mean", mean)])
    }

    fn aggregate(measure: &'static str, [mean, median, min, max, std_dev]: [f64; 5]) -> Expected {
        (
            measure,
            vec![
                ("mean", mean),
                ("median", median),
                ("min", min),
                ("max", max),
                ("std_dev", std_dev),
            ],
        )
    }

    /// Kills a text leaf that keeps the `-N` suffix or a `key=value` part in the name, drops the
    /// package, reads a `key=value` number as a string or a non-number as a number, drops any
    /// unit past `ns/op` (#548), reads `MB/s` as anything but decimal megabytes, promotes a
    /// statistic to `value`, calls a unit that is not per operation a mean, drops a line whose
    /// `ns/op` is suppressed, or records no leaf.
    #[test]
    fn adapter_go_bench_v1_text_packages() {
        for adapter in [Adapter::GoBench, Adapter::GoBenchText] {
            let results = convert_v1(adapter, "go/bench/packages.txt");
            assert_eq!(
                results.resolved_adapter,
                Some(Adapter::GoBenchText),
                "{adapter}"
            );
            assert_eq!(results.inner.len(), 7, "{adapter}");
            assert_eq!(variants(&results), 9, "{adapter}");
            assert_variant(
                &results,
                "example.com/bench/copy.BenchmarkCopy",
                r#"{"@gomaxprocs":16}"#,
                &[
                    latency(780.6),
                    mean(BytesThroughput::SLUG_STR, 5_247_390_000.0),
                    mean(Allocated::SLUG_STR, 4_098.0),
                    mean(Allocations::SLUG_STR, 1.0),
                ],
            );
            assert_variant(
                &results,
                "example.com/bench/copy.BenchmarkAlloc",
                r#"{"@gomaxprocs":16}"#,
                &[latency(1.377)],
            );
            assert_variant(
                &results,
                "example.com/bench/fib.BenchmarkFib10",
                r#"{"@gomaxprocs":16}"#,
                &[latency(159.9)],
            );
            assert_variant(
                &results,
                "example.com/bench/fib.BenchmarkFib",
                r#"{"@gomaxprocs":16,"n":10}"#,
                &[latency(159.2)],
            );
            assert_variant(
                &results,
                "example.com/bench/fib.BenchmarkFib",
                r#"{"@gomaxprocs":16,"n":15}"#,
                &[latency(1_848.0)],
            );
            assert_variant(
                &results,
                "example.com/bench/fib.BenchmarkFib/label",
                r#"{"@gomaxprocs":16,"n":5}"#,
                &[latency(13.53)],
            );
            assert_variant(
                &results,
                "example.com/bench/fib.BenchmarkFib/label",
                r#"{"@gomaxprocs":16,"n":"5#01"}"#,
                &[latency(17.93)],
            );
            assert_variant(
                &results,
                "example.com/bench/fib.BenchmarkCustom",
                r#"{"@gomaxprocs":16}"#,
                &[
                    latency(164.3),
                    mean("calls/op", 177.0),
                    ("depth", vec![("value", 3.0)]),
                ],
            );
            assert_variant(
                &results,
                "example.com/bench/fib.BenchmarkSuppressed",
                r#"{"@gomaxprocs":16}"#,
                &[mean("widgets/op", 42.0)],
            );
            assert_units(
                &results,
                &[
                    (Allocated::SLUG_STR, Allocated::UNITS_STR),
                    ("calls/op", "calls/op"),
                    ("depth", "depth"),
                    ("widgets/op", "widgets/op"),
                ],
            );
        }
    }

    /// Kills a leaf that drops `-benchmem`'s zero allocations.
    #[test]
    fn adapter_go_bench_v1_text_benchmem() {
        let results = convert_v1(Adapter::GoBenchText, "go/bench/benchmem.txt");
        assert_eq!(results.inner.len(), 2);
        assert_variant(
            &results,
            "example.com/bench/copy.BenchmarkAlloc",
            r#"{"@gomaxprocs":16}"#,
            &[
                latency(1.427),
                mean(Allocated::SLUG_STR, 0.0),
                mean(Allocations::SLUG_STR, 0.0),
            ],
        );
    }

    /// Kills a leaf that keeps only one run of `-count`, names an aggregate inaccurately, takes
    /// the population standard deviation, or aggregates across packages.
    #[test]
    fn adapter_go_bench_v1_text_count() {
        let results = convert_v1(Adapter::GoBenchText, "go/bench/count.txt");
        assert_eq!(results.inner.len(), 2);
        assert_eq!(variants(&results), 2);
        assert_variant(
            &results,
            "example.com/bench/copy.BenchmarkCopy",
            r#"{"@gomaxprocs":16}"#,
            &[
                aggregate(
                    Latency::SLUG_STR,
                    [851.3, 832.5, 824.4, 897.0, 39.784_042_026_923_316],
                ),
                aggregate(
                    BytesThroughput::SLUG_STR,
                    [
                        4_818_426_666.666_667,
                        4_920_400_000.0,
                        4_566_300_000.0,
                        4_968_580_000.0,
                        219_672_984.532_311_9,
                    ],
                ),
                aggregate(
                    Allocated::SLUG_STR,
                    [
                        4_096.666_666_666_667,
                        4_096.0,
                        4_096.0,
                        4_098.0,
                        1.154_700_538_379_251_5,
                    ],
                ),
                aggregate(Allocations::SLUG_STR, [1.0, 1.0, 1.0, 1.0, 0.0]),
            ],
        );
        assert_variant(
            &results,
            "example.com/bench/fib.BenchmarkFib10",
            r#"{"@gomaxprocs":16}"#,
            &[aggregate(
                Latency::SLUG_STR,
                [162.1, 163.0, 149.2, 174.1, 12.474_373_731_775_078],
            )],
        );
    }

    /// Kills a leaf that gives the absent suffix of GOMAXPROCS 1 no parameter, or keeps `-cpu`'s
    /// runs as separate benchmarks.
    #[test]
    fn adapter_go_bench_v1_text_cpu() {
        let results = convert_v1(Adapter::GoBenchText, "go/bench/cpu.txt");
        assert_eq!(results.inner.len(), 1);
        assert_eq!(variants(&results), 3);
        let name = "example.com/bench/fib.BenchmarkFib10";
        assert_variant(&results, name, r#"{"@gomaxprocs":1}"#, &[latency(163.8)]);
        assert_variant(&results, name, r#"{"@gomaxprocs":2}"#, &[latency(164.1)]);
        assert_variant(&results, name, r#"{"@gomaxprocs":4}"#, &[latency(151.6)]);
    }

    /// Kills a leaf that needs a `pkg:` line, or splits a name part without `=`.
    #[test]
    fn adapter_go_bench_v1_text_no_package() {
        let results = convert_v1(Adapter::GoBenchText, "go/bench/five.txt");
        assert_eq!(results.inner.len(), 5);
        assert_variant(
            &results,
            "BenchmarkFib10",
            r#"{"@gomaxprocs":8}"#,
            &[latency(325.0)],
        );
        assert_variant(
            &results,
            "BenchmarkFib20",
            r#"{"@gomaxprocs":1}"#,
            &[latency(40_537.123)],
        );
        assert_variant(
            &results,
            "BenchmarkFib/my_tabled_benchmark_-_10",
            r#"{"@gomaxprocs":8}"#,
            &[latency(325.0)],
        );
        assert_variant(
            &results,
            "BenchmarkFib/my/tabled/benchmark_-_20",
            r#"{"@gomaxprocs":1}"#,
            &[latency(40_537.456)],
        );
    }

    /// Kills a leaf that reads a second time per operation into latency beside `ns/op`, misses
    /// latency in another time unit when `ns/op` is suppressed, drops a line for one `NaN`
    /// value, or skips that value uncounted.
    #[test]
    fn adapter_go_bench_v1_text_units() {
        let results = convert_v1(Adapter::GoBenchText, "go/bench/units.txt");
        assert_eq!(results.inner.len(), 3);
        assert_eq!(results.skipped.metrics, 1);
        let parameters = r#"{"@gomaxprocs":16}"#;
        assert_variant(
            &results,
            "example.com/bench/units.BenchmarkTwoTimes",
            parameters,
            &[latency(18.99), mean("us/op", 3.0)],
        );
        assert_variant(
            &results,
            "example.com/bench/units.BenchmarkMicroOnly",
            parameters,
            &[latency(2_000.0)],
        );
        assert_variant(
            &results,
            "example.com/bench/units.BenchmarkUndefined",
            parameters,
            &[latency(38.36), mean("hits/op", 4.0)],
        );
        assert_units(&results, &[("us/op", "us/op"), ("hits/op", "hits/op")]);
    }

    /// Kills a leaf that drops a `ReportMetric` unit.
    #[test]
    fn adapter_go_bench_v1_text_report_metric() {
        let results = convert_v1(Adapter::GoBenchText, "go/bench/three.txt");
        assert_eq!(results.inner.len(), 3);
        assert_variant(
            &results,
            "bencher.dev.BenchmarkFib20WithAuxMetric",
            r#"{"@gomaxprocs":8}"#,
            &[latency(25_829.0), ("auxMetricUnits", vec![("value", 4.0)])],
        );
        assert_units(&results, &[("auxMetricUnits", "auxMetricUnits")]);
    }

    /// Kills a JSON leaf that drops a result split across output events, or records no leaf.
    #[test]
    fn adapter_go_bench_v1_json() {
        for adapter in [Adapter::GoBench, Adapter::GoBenchJson] {
            let results = convert_v1(adapter, "go/bench/json.txt");
            assert_eq!(
                results.resolved_adapter,
                Some(Adapter::GoBenchJson),
                "{adapter}"
            );
            assert_eq!(results.inner.len(), 7, "{adapter}");
            assert_eq!(variants(&results), 9, "{adapter}");
            assert_variant(
                &results,
                "example.com/bench/copy.BenchmarkCopy",
                r#"{"@gomaxprocs":16}"#,
                &[
                    latency(776.8),
                    mean(BytesThroughput::SLUG_STR, 5_273_230_000.0),
                    mean(Allocated::SLUG_STR, 4_101.0),
                    mean(Allocations::SLUG_STR, 1.0),
                ],
            );
            assert_variant(
                &results,
                "example.com/bench/fib.BenchmarkFib/label",
                r#"{"@gomaxprocs":16,"n":"5#01"}"#,
                &[
                    latency(13.55),
                    mean(Allocated::SLUG_STR, 0.0),
                    mean(Allocations::SLUG_STR, 0.0),
                ],
            );
            assert_variant(
                &results,
                "example.com/bench/fib.BenchmarkSuppressed",
                r#"{"@gomaxprocs":16}"#,
                &[
                    mean("widgets/op", 42.0),
                    mean(Allocated::SLUG_STR, 0.0),
                    mean(Allocations::SLUG_STR, 0.0),
                ],
            );
        }
    }

    /// Kills a JSON leaf that keeps only one run of `-count`.
    #[test]
    fn adapter_go_bench_v1_json_count() {
        let results = convert_v1(Adapter::GoBenchJson, "go/bench/json_count.txt");
        assert_eq!(results.inner.len(), 2);
        assert_variant(
            &results,
            "example.com/bench/copy.BenchmarkCopy",
            r#"{"@gomaxprocs":16}"#,
            &[
                aggregate(
                    Latency::SLUG_STR,
                    [663.1, 663.1, 516.3, 809.9, 207.606_550_956_370_37],
                ),
                aggregate(
                    BytesThroughput::SLUG_STR,
                    [
                        6_495_645_000.0,
                        6_495_645_000.0,
                        5_057_520_000.0,
                        7_933_770_000.0,
                        2_033_815_879.387_807_4,
                    ],
                ),
                aggregate(
                    Allocated::SLUG_STR,
                    [4_097.0, 4_097.0, 4_096.0, 4_098.0, SQRT_2],
                ),
                aggregate(Allocations::SLUG_STR, [1.0, 1.0, 1.0, 1.0, 0.0]),
            ],
        );
        assert_variant(
            &results,
            "example.com/bench/fib.BenchmarkFib10",
            r#"{"@gomaxprocs":16}"#,
            &[aggregate(
                Latency::SLUG_STR,
                [166.8, 166.8, 164.4, 169.2, 3.394_112_549_695_416],
            )],
        );
    }

    /// Kills a JSON leaf that joins the output of interlaced packages in the order it arrives.
    #[test]
    fn adapter_go_bench_v1_json_interlaced() {
        let input = std::fs::read_to_string("./tool_output/go/bench/json.txt")
            .expect("Failed to read a fixture");
        let (copy, fib): (Vec<_>, Vec<_>) = input
            .lines()
            .partition(|line| line.contains(r#""Package":"example.com/bench/copy""#));
        let mut interlaced = Vec::<&str>::new();
        for index in 0..copy.len().max(fib.len()) {
            interlaced.extend(copy.get(index));
            interlaced.extend(fib.get(index));
        }
        let interlaced = interlaced.join("\n");
        let settings = Settings::new(None, BmfVersion::V1);
        assert_eq!(
            Adapter::GoBenchJson.convert(&interlaced, settings),
            Adapter::GoBenchJson.convert(&input, settings)
        );
    }

    /// Kills a node that tries the text leaf before the JSON leaf.
    #[test]
    fn adapter_go_bench_v1_tries_json_first() {
        let input = ["json.txt", "packages.txt"]
            .map(|fixture| {
                std::fs::read_to_string(format!("./tool_output/go/bench/{fixture}"))
                    .expect("Failed to read a fixture")
            })
            .join("\n");
        let results = Adapter::GoBench
            .convert(&input, Settings::new(None, BmfVersion::V1))
            .expect("Failed to parse");
        assert_eq!(results.resolved_adapter, Some(Adapter::GoBenchJson));
    }

    /// Kills a text leaf that skips a variant with too many parameters without counting it.
    #[test]
    fn adapter_go_bench_v1_text_too_many_parameters() {
        let results = AdapterGoBenchText::parse(
            concat!(
                "BenchmarkMany/a=1/b=2/c=3/d=4/e=5/f=6/g=7/h=8-4 \t 100 \t 10 ns/op\n",
                "BenchmarkFew/a=1-4 \t 100 \t 10 ns/op\n",
            ),
            Settings::new(None, BmfVersion::V1),
        )
        .expect("Failed to parse");
        assert_eq!(results.inner.len(), 1);
        assert_eq!(results.skipped.variants, 1);
    }

    /// Ruling 90: `@gomaxprocs` from the suffix wins over a part with that key, which is skipped
    /// and counted, and a part named `gomaxprocs` stays the user's. Kills a text leaf that lets
    /// the part win, skips it uncounted, counts it once per line or once per variant, or keeps
    /// the bare key.
    #[test]
    fn adapter_go_bench_v1_text_adapter_key_wins() {
        let results = AdapterGoBenchText::parse(
            "BenchmarkKeys/gomaxprocs=2/@gomaxprocs=3/@gomaxprocs=5-4 \t 100 \t 10 ns/op\n\
             BenchmarkKeys/gomaxprocs=2/@gomaxprocs=3/@gomaxprocs=5-4 \t 100 \t 12 ns/op\n",
            Settings::new(None, BmfVersion::V1),
        )
        .expect("Failed to parse");
        assert_eq!(results.skipped.variants, 4);
        assert_variant(
            &results,
            "BenchmarkKeys",
            r#"{"@gomaxprocs":4,"gomaxprocs":2}"#,
            &[aggregate(
                Latency::SLUG_STR,
                [11.0, 11.0, 10.0, 12.0, SQRT_2],
            )],
        );
    }

    /// Kills a text leaf that reads the average setting.
    #[test]
    fn adapter_go_bench_v1_text_ignores_average() {
        for suffix in ["go/bench/packages.txt", "go/bench/count.txt"] {
            assert_ignores_average(Adapter::GoBenchText, suffix);
        }
    }

    /// Kills a JSON leaf that reads the average setting.
    #[test]
    fn adapter_go_bench_v1_json_ignores_average() {
        assert_ignores_average(Adapter::GoBenchJson, "go/bench/json.txt");
    }
}
