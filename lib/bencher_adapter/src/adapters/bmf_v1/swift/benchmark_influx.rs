use bencher_json::project::report::Adapter;

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        leaf::LeafResults,
        swift::benchmark::{Group, Release, insert},
    },
    results::adapter_results::AdapterResults,
};

/// Swift package-benchmark's `--format influx` export at version 1, as a file or on stdout after
/// the tool's own lines, from 1.29.3, which adds the `percentile` column.
///
/// Each value is the harness's own percentile in base units, per outer iteration of a benchmark
/// with a scaling factor. A line after the header that is not one of the harness's rows refuses
/// the input, so a second export never reads as part of the first.
pub struct AdapterSwiftBenchmarkInflux;

impl Adaptable for AdapterSwiftBenchmarkInflux {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut lines = input.lines().skip_while(|line| *line != DATATYPE);
        lines.next()?;
        if lines.next()? != COLUMNS {
            return None;
        }
        let rows = rows(lines)?;
        let mut results = LeafResults::default();
        let mut benchmarks: Vec<(&str, &str, Vec<Group>)> = Vec::new();
        for rows in chunks(&rows) {
            let Some((target, test, group)) = read(rows) else {
                results.skip_measure();
                continue;
            };
            if let Some((current_target, current_test, groups)) = benchmarks.last_mut()
                && (*current_target, *current_test) == (target, test)
            {
                groups.push(group);
            } else {
                benchmarks.push((target, test, vec![group]));
            }
        }
        let release = Release::of(benchmarks.iter().flat_map(|(_, _, groups)| groups));
        for (target, test, groups) in benchmarks {
            insert(&mut results, target, test, groups, release);
        }
        results.finish_with_skips(Adapter::SwiftBenchmarkInflux)
    }
}

/// The export's first 2 lines (`BenchmarkTool+Export+InfluxCSVFormatter.swift:57-61`).
const DATATYPE: &str =
    "#datatype tag,tag,tag,tag,tag,tag,tag,tag,tag,double,double,double,long,long,dateTime";
const COLUMNS: &str = "measurement,hostName,processoryType,processors,memory,kernelVersion,metric,unit,test,percentile,value,test_average,iterations,warmup_iterations,time";

/// Every row after the header, or `None` for a line that is not one. The export ends in a blank
/// line, and nothing follows it.
fn rows<'a>(lines: impl Iterator<Item = &'a str>) -> Option<Vec<Row<'a>>> {
    let mut rows = Vec::new();
    let mut ended = false;
    for line in lines {
        if line.is_empty() {
            ended = true;
        } else if ended {
            return None;
        } else {
            rows.push(Row::parse(line)?);
        }
    }
    Some(rows)
}

/// The rows of each metric of each benchmark, which the harness writes one after another.
fn chunks<'r, 'a>(rows: &'r [Row<'a>]) -> Vec<&'r [Row<'a>]> {
    let mut chunks = Vec::new();
    let mut rest = rows;
    while let [first, tail @ ..] = rest {
        let length = 1 + tail
            .iter()
            .take_while(|row| row.key() == first.key())
            .count();
        let (chunk, tail) = rest.split_at(length);
        chunks.push(chunk);
        rest = tail;
    }
    chunks
}

/// The target, the test, and the metric of one metric's rows, or `None` when they are not its 7
/// percentiles in order, or when the test holds a comma outside the quotes the harness puts
/// around a name with one (`BenchmarkTool+Export+InfluxCSVFormatter.swift:154-158`), as when a
/// comma inside a custom metric's name moves the fields.
fn read<'a>(rows: &[Row<'a>]) -> Option<(&'a str, &'a str, Group)> {
    let [first, ..] = rows else {
        return None;
    };
    let test = if first.test.contains(',') {
        first.test.strip_prefix('"')?.strip_suffix('"')?
    } else {
        first.test
    };
    if !rows
        .iter()
        .zip(PERCENTILES)
        .all(|(row, percentile)| row.percentile == percentile)
    {
        return None;
    }
    let values = rows
        .iter()
        .map(|row| row.value)
        .collect::<Vec<_>>()
        .try_into()
        .ok()?;
    Some((
        first.target,
        test,
        Group {
            label: first.metric.to_owned(),
            values,
        },
    ))
}

/// `Statistics.defaultPercentilesToCalculate` as Influx writes it (`Statistics.swift:19`).
const PERCENTILES: [&str; 7] = ["0.0", "25.0", "50.0", "75.0", "90.0", "99.0", "100.0"];

/// One row: 8 fields before the test, which may hold commas, and 6 after it.
struct Row<'a> {
    target: &'a str,
    metric: &'a str,
    test: &'a str,
    percentile: &'a str,
    value: f64,
}

impl<'a> Row<'a> {
    fn parse(line: &'a str) -> Option<Self> {
        let right = line.rsplitn(7, ',').collect::<Vec<_>>();
        let [time, warmup, iterations, average, value, percentile, rest] = right.as_slice() else {
            return None;
        };
        let left = rest.splitn(9, ',').collect::<Vec<_>>();
        let [target, _, _, _, _, _, metric, _, test] = left.as_slice() else {
            return None;
        };
        let shaped = !target.is_empty()
            && !time.is_empty()
            && PERCENTILES.contains(percentile)
            && [*value, *iterations, *warmup]
                .iter()
                .all(|count| !count.is_empty() && count.bytes().all(|byte| byte.is_ascii_digit()))
            && average.parse::<f64>().is_ok();
        if !shaped {
            return None;
        }
        Some(Self {
            target,
            metric,
            test,
            percentile,
            value: value.parse().ok()?,
        })
    }

    fn key(&self) -> (&'a str, &'a str, &'a str) {
        (self.target, self.metric, self.test)
    }
}

#[cfg(test)]
mod test_swift_benchmark_influx {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{
                    Allocations, CpuTime, Latency, PeakMemory, SystemTime, Throughput, UserTime,
                },
                divan::Deallocations,
                dotnet::Allocated,
                gungraun::SysCount,
                swift::{
                    LargeAllocations, LogicalIo, ObjectAllocations, PeakMemoryDelta, PeakThreads,
                    PeakVirtualMemory, PhysicalIo, Releases, Retains, SmallAllocations,
                    UnfreedAllocations, UnfreedBytes, UnreleasedReferences,
                },
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::{
            swift::benchmark::test_swift_benchmark::{larger, smaller},
            test_util::{assert_variant, convert_v1, value, variants},
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const INSERT: &str = "swift/benchmark/influx_insert.csv";
    const DIRECT: &str = "swift/benchmark/influx.csv";

    fn read(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/{suffix}"))
            .expect("Failed to read a fixture")
    }

    fn convert(input: &str) -> Option<AdapterResults> {
        Adapter::SwiftBenchmarkInflux.convert(input, Settings::new(None, BmfVersion::V1))
    }

    /// `input` with `edit` applied to the first of target Alpha's rows that holds `row`, which
    /// `edit` replaces, or removes when it returns `None`.
    fn edited<F: FnOnce(&str) -> Option<String>>(input: &str, row: &str, edit: F) -> String {
        let mut edit = Some(edit);
        let mut output = String::new();
        for line in input.lines() {
            let line = if line.starts_with("Alpha,") && line.contains(row) {
                edit.take()
                    .map_or_else(|| Some(line.to_owned()), |edit| edit(line))
            } else {
                Some(line.to_owned())
            };
            if let Some(line) = line {
                output.push_str(&line);
                output.push('\n');
            }
        }
        assert!(edit.is_none(), "no row {row}");
        output
    }

    /// Kills a name without its target or joined by another delimiter, tags left in the name or
    /// keyed with `@`, throughput named as a metric that prefers smaller values, the `unit`
    /// column applied as a factor, and a leaf that records no leaf.
    #[test]
    fn adapter_swift_benchmark_influx_tags() {
        let results = convert_v1(Adapter::SwiftBenchmarkInflux, INSERT);
        assert_eq!(
            results.resolved_adapter,
            Some(Adapter::SwiftBenchmarkInflux)
        );
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(variants(&results), 6);
        assert_variant(
            &results,
            "Alpha:Insert",
            r#"{"count":"1","kind":"array"}"#,
            &[
                smaller(Allocations::SLUG_STR, [1.0; 7]),
                larger(
                    Throughput::SLUG_STR,
                    [
                        14_492_754.0,
                        14_286_847.0,
                        12_500_991.0,
                        12_500_991.0,
                        12_500_991.0,
                        7_143_423.0,
                        157_480.0,
                    ],
                ),
                smaller(
                    Latency::SLUG_STR,
                    [69.0, 70.0, 80.0, 80.0, 80.0, 140.0, 6_350.0],
                ),
            ],
        );
    }

    /// Kills a metric the harness defines in the wrong measure, or left out of the plain variant.
    #[test]
    fn adapter_swift_benchmark_influx_every_metric() {
        let results = convert_v1(Adapter::SwiftBenchmarkInflux, DIRECT);
        assert_eq!(results.skipped, Skipped::default());
        let rising = |low: f64, high: f64| [low, low, low, low, low, low, high];
        assert_variant(
            &results,
            "Beta:All metrics",
            "{}",
            &[
                smaller(UnreleasedReferences::SLUG_STR, rising(27.0, 32.0)),
                smaller(Deallocations::SLUG_STR, rising(17.0, 21.0)),
                smaller(
                    Allocated::SLUG_STR,
                    [
                        35_560.0, 35_583.0, 35_583.0, 35_583.0, 35_583.0, 35_583.0, 44_487.0,
                    ],
                ),
                smaller(LargeAllocations::SLUG_STR, [0.0; 7]),
                smaller(SmallAllocations::SLUG_STR, rising(17.0, 20.0)),
                smaller(Allocations::SLUG_STR, rising(17.0, 20.0)),
                smaller(UnfreedBytes::SLUG_STR, rising(0.0, 8_272.0)),
                smaller(UnfreedAllocations::SLUG_STR, rising(0.0, 2.0)),
                smaller(PeakMemory::SLUG_STR, [28_704_768.0; 7]),
                smaller(PeakVirtualMemory::SLUG_STR, [326_295_552.0; 7]),
                smaller(PeakMemoryDelta::SLUG_STR, [0.0; 7]),
                smaller(ObjectAllocations::SLUG_STR, rising(5.0, 6.0)),
                smaller(Releases::SLUG_STR, rising(34.0, 39.0)),
                smaller(Retains::SLUG_STR, [2.0; 7]),
                smaller(PeakThreads::SLUG_STR, [5.0; 7]),
                larger(
                    Throughput::SLUG_STR,
                    [
                        7_692_308.0,
                        7_143_423.0,
                        6_713_343.0,
                        6_668_287.0,
                        6_623_231.0,
                        4_546_559.0,
                        185_598.0,
                    ],
                ),
                smaller(SystemTime::SLUG_STR, rising(0.0, 10_000_000.0)),
                smaller(CpuTime::SLUG_STR, rising(0.0, 10_000_000.0)),
                smaller(UserTime::SLUG_STR, rising(0.0, 10_000_000.0)),
                smaller(
                    Latency::SLUG_STR,
                    [130.0, 140.0, 149.0, 150.0, 151.0, 211.0, 5_388.0],
                ),
            ],
        );
    }

    /// Kills a direction's metrics in the plain variant or in one variant for both directions, a
    /// custom metric dropped, in another unit, or named by its polarity's opposite, and a quoted
    /// name read with its quotes.
    #[test]
    fn adapter_swift_benchmark_influx_directions_and_custom_metrics() {
        let results = convert_v1(Adapter::SwiftBenchmarkInflux, DIRECT);
        let rising = |low: f64, high: f64| [low, low, low, low, low, low, high];
        assert_variant(
            &results,
            "Beta:All metrics",
            r#"{"@direction":"read"}"#,
            &[
                smaller(LogicalIo::SLUG_STR, [0.0, 5.0, 5.0, 5.0, 5.0, 5.0, 314.0]),
                smaller(PhysicalIo::SLUG_STR, [0.0; 7]),
                smaller(SysCount::SLUG_STR, rising(0.0, 1.0)),
            ],
        );
        assert_variant(
            &results,
            "Beta:All metrics",
            r#"{"@direction":"write"}"#,
            &[
                smaller(LogicalIo::SLUG_STR, [0.0; 7]),
                smaller(PhysicalIo::SLUG_STR, [0.0; 7]),
                smaller(SysCount::SLUG_STR, [0.0; 7]),
            ],
        );
        assert_variant(
            &results,
            "Alpha:Custom metrics",
            "{}",
            &[
                larger(
                    "CacheHits",
                    [1_000.0, 976.0, 953.0, 928.0, 912.0, 900.0, 900.0],
                ),
                smaller("QueueDepth", [1.0, 4.0, 6.0, 10.0, 11.0, 12.0, 12.0]),
                smaller(
                    Latency::SLUG_STR,
                    [570.0, 580.0, 581.0, 590.0, 592.0, 800.0, 2_894.0],
                ),
            ],
        );
        assert_eq!(
            results
                .units
                .get(&"CacheHits".parse().expect("Failed to parse a measure")),
            Some(&"#".parse().expect("Failed to parse a unit"))
        );
        assert_eq!(
            value(
                &results,
                r#"Alpha:Name, with "quotes""#,
                "{}",
                Throughput::SLUG_STR,
                "max"
            ),
            Some(16_666_667.0)
        );
    }

    /// Kills the tool's own lines before the header left unskipped.
    #[test]
    fn adapter_swift_benchmark_influx_stdout() {
        let results = convert_v1(
            Adapter::SwiftBenchmarkInflux,
            "swift/benchmark/influx_stdout.txt",
        );
        assert_eq!(results.skipped, Skipped::default());
        assert_variant(
            &results,
            "Alpha:Custom metrics",
            "{}",
            &[
                larger(
                    "CacheHits",
                    [999.0, 972.0, 956.0, 928.0, 909.0, 900.0, 900.0],
                ),
                smaller("QueueDepth", [1.0, 4.0, 6.0, 9.0, 11.0, 12.0, 12.0]),
                smaller(
                    Latency::SLUG_STR,
                    [561.0, 571.0, 581.0, 581.0, 821.0, 931.0, 2_214.0],
                ),
            ],
        );
    }

    /// A second export after the first, as `baseline read` of 2 baselines prints on stdout, a
    /// line of the tool's own or a blank line inside the export, as a cut and merged log holds,
    /// and a row at a percentile the harness never writes refuse the input.
    ///
    /// The capture is edited because the harness writes only its 7 percentiles: one row's `90.0`
    /// becomes `95.0`.
    ///
    /// Kills: a second export read into the first, a line that is not a row skipped, and a row
    /// at another percentile taken as a row.
    #[test]
    fn adapter_swift_benchmark_influx_one_export() {
        let insert = read(INSERT);
        assert!(convert(&insert).is_some(), "the capture reads");
        assert_eq!(convert(&format!("{insert}{insert}")), None);
        for interruption in ["Building Alpha", ""] {
            let interrupted = edited(&insert, "\"Insert (count: 1, kind: array)\"", |line| {
                Some(format!("{interruption}\n{line}"))
            });
            assert_eq!(convert(&interrupted), None, "{interruption:?}");
        }
        let percentile = edited(&insert, ",90.0,", |line| {
            Some(line.replacen(",90.0,", ",95.0,", 1))
        });
        assert_eq!(convert(&percentile), None);
    }

    /// A metric cut short of its 7 rows, rows that are not one distribution in their polarity's
    /// order, a custom metric whose name holds a comma, which shifts its fields, and a second
    /// metric of one label are each skipped and counted, and the benchmark's other metrics still
    /// read.
    ///
    /// The capture is edited because the harness writes every metric whole and in order: one row
    /// is removed as a cut file loses it, a flat metric's row is moved after the next one, a
    /// throughput row is set above the maximum, a custom metric's name gains a comma, as a
    /// user's `.custom("Cache,Hits")` writes it, and `QueueDepth` becomes `CacheMisses` followed
    /// by `CacheHits`, as custom metrics `Cache Hits`, `Cache Misses`, and `CacheHits` write
    /// them, the label `CacheHits` twice with another metric between.
    ///
    /// Kills: a short metric or rows out of order read under the wrong names, a polarity taken
    /// from the order alone for a metric the harness defines, a shifted row read, a repeated
    /// label read over the first, and a skip not counted.
    #[test]
    fn adapter_swift_benchmark_influx_skips_metrics() {
        let input = read(DIRECT);
        let cut = edited(&input, ",Time(wallclock),ns,Sum array,25.0,", |_| None);
        let raised = edited(&input, ",Throughput(#/s),K,Sum array,50.0,", |line| {
            Some(line.replacen(",50.0,1447935,", ",50.0,9999999,", 1))
        });
        let row = ",Memory(residentpeak),M,Sum array,25.0,";
        let moved = input
            .lines()
            .find(|line| line.starts_with("Alpha,") && line.contains(row))
            .expect("Failed to find a row")
            .to_owned();
        let reordered = edited(
            &edited(&input, row, |_| None),
            ",Memory(residentpeak),M,Sum array,50.0,",
            |line| Some(format!("{line}\n{moved}")),
        );
        let comma = input.replace(",CacheHits,,", ",Cache,Hits,,");
        for (edited, benchmark, measure) in [
            (cut, "Alpha:Sum array", Latency::SLUG_STR),
            (reordered, "Alpha:Sum array", PeakMemory::SLUG_STR),
            (raised, "Alpha:Sum array", Throughput::SLUG_STR),
            (comma, "Alpha:Custom metrics", "CacheHits"),
        ] {
            assert!(edited != input, "{measure}");
            let results = convert(&edited).unwrap_or_else(|| panic!("{measure}"));
            assert_eq!(
                results.skipped,
                Skipped {
                    measures: 1,
                    ..Skipped::default()
                },
                "{measure}"
            );
            assert_eq!(
                value(&results, benchmark, "{}", measure, "median"),
                None,
                "{measure}"
            );
            let other = if measure == Latency::SLUG_STR {
                Throughput::SLUG_STR
            } else {
                Latency::SLUG_STR
            };
            assert!(
                value(&results, benchmark, "{}", other, "median").is_some(),
                "{measure}"
            );
        }
        let queue_depth = input
            .lines()
            .filter(|line| line.contains(",QueueDepth,,Custom metrics,"))
            .collect::<Vec<_>>();
        let renamed = |label: &str| {
            queue_depth
                .iter()
                .map(|line| line.replacen(",QueueDepth,", label, 1))
                .collect::<Vec<_>>()
        };
        let repeated = input.replacen(
            &queue_depth.join("\n"),
            &[renamed(",CacheMisses,"), renamed(",CacheHits,")]
                .concat()
                .join("\n"),
            1,
        );
        let results = convert(&repeated).expect("Failed to read the repeated metric");
        assert_eq!(
            results.skipped,
            Skipped {
                measures: 1,
                ..Skipped::default()
            }
        );
        assert_variant(
            &results,
            "Alpha:Custom metrics",
            "{}",
            &[
                larger(
                    "CacheHits",
                    [1_000.0, 976.0, 953.0, 928.0, 912.0, 900.0, 900.0],
                ),
                smaller("CacheMisses", [1.0, 4.0, 6.0, 10.0, 11.0, 12.0, 12.0]),
                smaller(
                    Latency::SLUG_STR,
                    [570.0, 580.0, 581.0, 590.0, 592.0, 800.0, 2_894.0],
                ),
            ],
        );
    }

    /// `Malloc / free Δ` from 1.34.1, beside no label that 1.35.0 first writes, is skipped and
    /// counted while the benchmark's other metrics read, and from 1.38.1, in a run that holds
    /// `Free (total)`, it reads as unfreed allocations, also in a benchmark with no other metric.
    ///
    /// The 1.38.1 capture is edited because its every benchmark with the label also holds a label
    /// that 1.35.0 first writes: `Sum array`'s `Malloc/freeΔ` rows are copied as a benchmark
    /// `Leak only` ahead of every other, as a hand-picked metric list writes it.
    ///
    /// Kills: the check removed, a check that any other label passes, and a check of each
    /// benchmark on its own or of the benchmarks before it.
    #[test]
    fn adapter_swift_benchmark_influx_malloc_free_delta() {
        let old = convert_v1(
            Adapter::SwiftBenchmarkInflux,
            "swift/benchmark/influx_1_34_1.csv",
        );
        assert_eq!(
            old.skipped,
            Skipped {
                measures: 1,
                ..Skipped::default()
            }
        );
        let linked = |measure| value(&old, "Alpha:Linked list", "{}", measure, "max");
        assert_eq!(linked(UnfreedAllocations::SLUG_STR), None);
        assert_eq!(linked(Allocations::SLUG_STR), Some(274.0));
        let input = read(DIRECT);
        let leak = input
            .lines()
            .filter(|line| line.starts_with("Alpha,") && line.contains(",Malloc/freeΔ,,Sum array,"))
            .map(|line| line.replacen(",Sum array,", ",Leak only,", 1))
            .collect::<Vec<_>>()
            .join("\n");
        let (header, rows) = input
            .match_indices('\n')
            .nth(1)
            .map(|(end, _)| input.split_at(end + 1))
            .expect("Failed to find the header");
        let new = convert(&format!("{header}{leak}\n{rows}")).expect("Failed to read the run");
        assert_eq!(new.skipped, Skipped::default());
        for benchmark in ["Alpha:Leak only", "Alpha:Sum array"] {
            assert_eq!(
                value(&new, benchmark, "{}", UnfreedAllocations::SLUG_STR, "max"),
                Some(2.0),
                "{benchmark}"
            );
        }
    }
}
