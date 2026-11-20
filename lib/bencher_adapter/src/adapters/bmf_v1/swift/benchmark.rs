use std::collections::{BTreeMap, HashSet};

use bencher_json::{
    BenchmarkName,
    project::measure::built_in::{
        BuiltInMeasure,
        default::{Allocations, CpuTime, Latency, PeakMemory, SystemTime, Throughput, UserTime},
        divan::Deallocations,
        dotnet::Allocated,
        gungraun::SysCount,
        iai::Instructions,
        swift::{
            AllocatedResidentMemory, LargeAllocations, LeakedResidentMemory, LogicalIo,
            ObjectAllocations, PeakMemoryDelta, PeakThreads, PeakVirtualMemory, PhysicalIo,
            Releases, Retains, RunningThreads, SmallAllocations, UnfreedAllocations, UnfreedBytes,
            UnreleasedReferences,
        },
        systems::ContextSwitches,
    },
};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        leaf::{LeafResults, parameter_set},
        swift::{
            benchmark_influx::AdapterSwiftBenchmarkInflux,
            benchmark_text::AdapterSwiftBenchmarkText,
        },
    },
    results::adapter_results::AdapterResults,
};

/// Swift package-benchmark at version 1: its `--format influx` export, then its default text
/// table.
pub struct AdapterSwiftBenchmark;

impl Adaptable for AdapterSwiftBenchmark {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterSwiftBenchmarkInflux::parse(input, settings)
            .or_else(|| AdapterSwiftBenchmarkText::parse(input, settings))
    }
}

/// One metric of a benchmark: its label, the metric's description with every space removed as
/// Influx writes it, and its 7 percentile rows in the harness's order, in base units.
pub(super) struct Group {
    pub label: String,
    pub values: [f64; 7],
}

/// The benchmark `name` of `target` with its metrics, each in the variant of its direction, from
/// a run whose `release` the leaf found over every benchmark of its input.
///
/// A name Bencher cannot hold skips the benchmark; a repeated metric, one whose rows are not one
/// distribution in the order its polarity gives, or a `Malloc / free Δ` in a run with no label
/// that 1.35.0 first writes skips the metric's measure; a variant past the key cap or already
/// read is skipped. Each is counted, as is a tag that takes the adapter's own `@direction` key.
pub(super) fn insert(
    results: &mut LeafResults,
    target: &str,
    name: &str,
    groups: Vec<Group>,
    release: Release,
) {
    let (base, tags) = tags(name);
    let Ok(benchmark) = format!("{target}:{base}").parse::<BenchmarkName>() else {
        results.skip_benchmark();
        return;
    };
    let mut tag_parameters = serde_json::Map::new();
    for (key, value) in tags {
        if key == DIRECTION {
            results.skip_variant();
        } else {
            tag_parameters.insert(key.to_owned(), value.into());
        }
    }
    let mut seen = HashSet::new();
    let mut variants = BTreeMap::<Option<Direction>, Vec<_>>::new();
    for Group { label, values } in groups {
        if label == MALLOC_FREE_DELTA_LABEL && release == Release::Unknown {
            results.skip_measure();
            continue;
        }
        let metric = METRICS
            .iter()
            .find(|(known, _)| *known == label)
            .map(|(_, metric)| metric);
        let Some(names) = names(metric.map(|metric| metric.polarity), &values) else {
            results.skip_measure();
            continue;
        };
        if !seen.insert(label.clone()) {
            results.skip_measure();
            continue;
        }
        variants
            .entry(metric.and_then(|metric| metric.direction))
            .or_default()
            .push((metric, label, names, values));
    }
    for (direction, groups) in variants {
        let mut parameters = tag_parameters.clone();
        if let Some(direction) = direction {
            parameters.insert(DIRECTION.to_owned(), direction.as_str().into());
        }
        let Some(parameters) = parameter_set(parameters) else {
            results.skip_variant();
            continue;
        };
        let Some(mut variant) = results.new_variant(benchmark.clone(), parameters) else {
            continue;
        };
        for (metric, label, names, values) in groups {
            let measure = if let Some(metric) = metric {
                variant.named(metric.slug, metric.units)
            } else {
                variant.named(&label, CUSTOM_UNITS)
            };
            if let Some(measure) = measure {
                for (name, value) in names.iter().zip(values) {
                    variant.insert(measure.clone(), name, value);
                }
            }
        }
    }
}

/// What a run shows of the release that wrote it, which no export writes: one run has one
/// release, so a label that 1.35.0 first writes in any benchmark places every benchmark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Release {
    Unknown,
    Since1_35,
}

impl Release {
    pub(super) fn of<'g>(groups: impl IntoIterator<Item = &'g Group>) -> Self {
        if groups
            .into_iter()
            .any(|group| FIRST_IN_1_35.contains(&group.label.as_str()))
        {
            Self::Since1_35
        } else {
            Self::Unknown
        }
    }
}

/// The labels 1.35.0 first writes, each with its spaces removed, as both leaves name a metric
/// (`BenchmarkMetric.swift:219`, `225`, `227`, and `265` at 1.35.0, none of them at 1.34.1).
const FIRST_IN_1_35: [&str; 4] = [
    "Malloc(bytestotal)",
    "Memoryleaked(resident)",
    "Malloc/freeΔ(bytes)",
    "Free(total)",
];

/// Resident bytes leaked before 1.35.0 (`BenchmarkMetric.swift:181` at 1.34.1), and a count of
/// unfreed allocations from it (`BenchmarkMetric.swift:221` at 1.35.0), with no version written.
const MALLOC_FREE_DELTA_LABEL: &str = "Malloc/freeΔ";

pub(super) const THROUGHPUT_LABEL: &str = "Throughput(#/s)";

/// The base name and the tags `Benchmark.swift:129-142` appends to it, ` (key: value, ...)` with
/// the keys in ascending order, or the whole name when its end does not parse so.
fn tags(name: &str) -> (&str, Vec<(&str, &str)>) {
    name.strip_suffix(')')
        .and_then(|rest| rest.rsplit_once(" ("))
        .and_then(|(base, tags)| {
            let pairs = tags
                .split(", ")
                .map(|pair| pair.split_once(": "))
                .collect::<Option<Vec<_>>>()?;
            pairs
                .iter()
                .zip(pairs.iter().skip(1))
                .all(|((key, _), (next, _))| key < next)
                .then_some((base, pairs))
        })
        .unwrap_or((name, Vec::new()))
}

/// The names of a metric's rows, which run from percentile 0, the exact minimum, to 100, the
/// exact maximum (hdrhistogram-swift 0.2.0 `Histogram.swift:418-424`), except that a metric that
/// prefers larger values asks for `100 - p` (`Statistics.swift:103-106`). A custom metric's
/// polarity is not written, so its order tells it; `None` for rows that are not one
/// distribution in such an order.
fn names(polarity: Option<Polarity>, values: &[f64; 7]) -> Option<&'static [&'static str; 7]> {
    let pairs = || values.iter().zip(values.iter().skip(1));
    let ascending = pairs().all(|(value, next)| value <= next);
    let descending = pairs().all(|(value, next)| value >= next);
    match polarity {
        Some(Polarity::PrefersSmaller) => ascending.then_some(&PREFERS_SMALLER),
        Some(Polarity::PrefersLarger) => descending.then_some(&PREFERS_LARGER),
        None => {
            if ascending {
                Some(&PREFERS_SMALLER)
            } else {
                descending.then_some(&PREFERS_LARGER)
            }
        },
    }
}

const PREFERS_SMALLER: [&str; 7] = ["min", "p25", "median", "p75", "p90", "p99", "max"];
const PREFERS_LARGER: [&str; 7] = ["max", "p75", "median", "p25", "p10", "p1", "min"];

/// The unit of a custom metric, a count, as the harness writes it
/// (`BenchmarkResult.swift:105-106`).
const CUSTOM_UNITS: &str = "#";
const DIRECTION: &str = "@direction";

/// A metric the harness defines (`BenchmarkMetric.swift:194-266`), by its Influx label.
const METRICS: [(&str, Metric); 32] = [
    ("Time(userCPU)", Metric::of::<UserTime>()),
    ("Time(systemCPU)", Metric::of::<SystemTime>()),
    ("Time(totalCPU)", Metric::of::<CpuTime>()),
    ("Time(wallclock)", Metric::of::<Latency>()),
    (
        THROUGHPUT_LABEL,
        Metric {
            polarity: Polarity::PrefersLarger,
            ..Metric::of::<Throughput>()
        },
    ),
    ("Memory(residentpeak)", Metric::of::<PeakMemory>()),
    ("MemoryΔ(residentpeak)", Metric::of::<PeakMemoryDelta>()),
    ("Memory(virtualpeak)", Metric::of::<PeakVirtualMemory>()),
    ("Malloc(small)", Metric::of::<SmallAllocations>()),
    ("Malloc(large)", Metric::of::<LargeAllocations>()),
    ("Malloc(total)", Metric::of::<Allocations>()),
    ("Free(total)", Metric::of::<Deallocations>()),
    ("Malloc(bytestotal)", Metric::of::<Allocated>()),
    (MALLOC_FREE_DELTA_LABEL, Metric::of::<UnfreedAllocations>()),
    (
        "Memory(allocatedresident)",
        Metric::of::<AllocatedResidentMemory>(),
    ),
    (
        "Memoryleaked(resident)",
        Metric::of::<LeakedResidentMemory>(),
    ),
    ("Malloc/freeΔ(bytes)", Metric::of::<UnfreedBytes>()),
    ("Syscalls(total)", Metric::of::<SysCount>()),
    ("Contextswitches", Metric::of::<ContextSwitches>()),
    ("Threads(peak)", Metric::of::<PeakThreads>()),
    ("Threads(running)", Metric::of::<RunningThreads>()),
    (
        "Syscalls(read)",
        Metric::directed::<SysCount>(Direction::Read),
    ),
    (
        "Syscalls(write)",
        Metric::directed::<SysCount>(Direction::Write),
    ),
    (
        "Bytes(readlogical)",
        Metric::directed::<LogicalIo>(Direction::Read),
    ),
    (
        "Bytes(writelogical)",
        Metric::directed::<LogicalIo>(Direction::Write),
    ),
    (
        "Bytes(readphysical)",
        Metric::directed::<PhysicalIo>(Direction::Read),
    ),
    (
        "Bytes(writephysical)",
        Metric::directed::<PhysicalIo>(Direction::Write),
    ),
    ("Instructions", Metric::of::<Instructions>()),
    ("Objectallocs", Metric::of::<ObjectAllocations>()),
    ("Retains", Metric::of::<Retains>()),
    ("Releases", Metric::of::<Releases>()),
    (
        "(Alloc+Retain)-ReleaseΔ",
        Metric::of::<UnreleasedReferences>(),
    ),
];

/// The built-in measure a metric the harness defines goes to, and how its rows read.
struct Metric {
    slug: &'static str,
    units: &'static str,
    polarity: Polarity,
    direction: Option<Direction>,
}

impl Metric {
    const fn of<M: BuiltInMeasure>() -> Self {
        Self {
            slug: M::SLUG_STR,
            units: M::UNITS_STR,
            polarity: Polarity::PrefersSmaller,
            direction: None,
        }
    }

    const fn directed<M: BuiltInMeasure>(direction: Direction) -> Self {
        Self {
            direction: Some(direction),
            ..Self::of::<M>()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Polarity {
    PrefersSmaller,
    PrefersLarger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Direction {
    Read,
    Write,
}

impl Direction {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

#[cfg(test)]
pub(super) mod test_swift_benchmark {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{BuiltInMeasure as _, default::Throughput, gungraun::SysCount},
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use super::{Group, Release, insert, tags};
    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::{
            leaf::LeafResults,
            test_util::{Expected, assert_variant, variants},
        },
        results::adapter_results::Skipped,
    };

    /// The 7 rows of a metric that prefers smaller values, by their names.
    pub(crate) fn smaller(measure: &'static str, values: [f64; 7]) -> Expected {
        let names = ["min", "p25", "median", "p75", "p90", "p99", "max"];
        (measure, names.into_iter().zip(values).collect())
    }

    /// The 7 rows of a metric that prefers larger values, whose first row is the maximum.
    pub(crate) fn larger(measure: &'static str, values: [f64; 7]) -> Expected {
        let names = ["max", "p75", "median", "p25", "p10", "p1", "min"];
        (measure, names.into_iter().zip(values).collect())
    }

    fn group(label: &str, values: [f64; 7]) -> Group {
        Group {
            label: label.to_owned(),
            values,
        }
    }

    /// Kills tags parsed with their keys out of `Benchmark.swift`'s order, a pair with no `: `
    /// read, and the base name cut at its first ` (` rather than its last.
    #[test]
    fn swift_benchmark_tags() {
        for (name, expected) in [
            (
                "Insert (count: 64, kind: set)",
                ("Insert", vec![("count", "64"), ("kind", "set")]),
            ),
            ("Sum (fast) (n: 8)", ("Sum (fast)", vec![("n", "8")])),
            (
                "Insert (kind: set, count: 64)",
                ("Insert (kind: set, count: 64)", Vec::new()),
            ),
            ("Sum (fast)", ("Sum (fast)", Vec::new())),
            ("Sum array", ("Sum array", Vec::new())),
        ] {
            assert_eq!(tags(name), expected, "{name}");
        }
    }

    /// A metric the harness defines keeps its polarity when its rows are flat, while a custom
    /// metric's flat rows take the names of one that prefers smaller values, a tag keyed
    /// `@direction` yields to the adapter's key and is counted, and a second name that so reads
    /// as a variant already read is skipped and counted.
    ///
    /// Kills: throughput's polarity read from its order alone, a user's `@direction` kept
    /// beside or over the adapter's, and a repeated variant read over the first.
    #[test]
    fn swift_benchmark_flat_rows_and_direction_tag() {
        let mut results = LeafResults::default();
        insert(
            &mut results,
            "Beta",
            "All metrics (@direction: up, count: 1)",
            vec![
                group("Throughput(#/s)", [5.0; 7]),
                group("CacheHits", [3.0; 7]),
                group("Syscalls(read)", [1.0; 7]),
            ],
            Release::Unknown,
        );
        insert(
            &mut results,
            "Beta",
            "All metrics (@direction: down, count: 1)",
            vec![group("Throughput(#/s)", [7.0; 7])],
            Release::Unknown,
        );
        let results = results
            .finish(Adapter::SwiftBenchmarkInflux)
            .expect("Failed to read the benchmark");
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 3,
                ..Skipped::default()
            }
        );
        assert_eq!(variants(&results), 2);
        assert_variant(
            &results,
            "Beta:All metrics",
            r#"{"count":"1"}"#,
            &[
                larger(Throughput::SLUG_STR, [5.0; 7]),
                smaller("CacheHits", [3.0; 7]),
            ],
        );
        assert_variant(
            &results,
            "Beta:All metrics",
            r#"{"@direction":"read","count":"1"}"#,
            &[smaller(SysCount::SLUG_STR, [1.0; 7])],
        );
    }

    /// A name with 16 tags reads its plain variant, while its directed variant, which would hold
    /// 17 keys, is skipped and counted.
    ///
    /// Kills: a variant past the key cap skipped without a count.
    #[test]
    fn swift_benchmark_key_cap() {
        let keys = 1..=16;
        let tags = keys
            .clone()
            .map(|key| format!("k{key:02}: 1"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut results = LeafResults::default();
        insert(
            &mut results,
            "Beta",
            &format!("Many ({tags})"),
            vec![
                group("Throughput(#/s)", [5.0; 7]),
                group("Syscalls(read)", [1.0; 7]),
            ],
            Release::Unknown,
        );
        let results = results
            .finish(Adapter::SwiftBenchmarkInflux)
            .expect("Failed to read the benchmark");
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 1,
                ..Skipped::default()
            }
        );
        assert_eq!(variants(&results), 1);
        let parameters = keys
            .map(|key| format!(r#""k{key:02}":"1""#))
            .collect::<Vec<_>>()
            .join(",");
        assert_variant(
            &results,
            "Beta:Many",
            &format!("{{{parameters}}}"),
            &[larger(Throughput::SLUG_STR, [5.0; 7])],
        );
    }

    /// Every capture reads through each leaf as `bencher run` passes its stdout, with the blank
    /// lines at its start dropped and no last line break, exactly as from the capture's bytes.
    ///
    /// Kills: a text table that must be followed by the blank line the harness prints after it,
    /// and an Influx export that must end in a line break.
    #[test]
    fn swift_benchmark_rebuilt_stdout() {
        let settings = Settings::new(None, BmfVersion::V1);
        let mut claimed = 0;
        for entry in
            std::fs::read_dir("./tool_output/swift/benchmark").expect("Failed to read the fixtures")
        {
            let path = entry.expect("Failed to read a fixture").path();
            let input = std::fs::read_to_string(&path).expect("Failed to read a fixture");
            let rebuilt = input
                .lines()
                .skip_while(|line| line.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            for leaf in [Adapter::SwiftBenchmarkInflux, Adapter::SwiftBenchmarkText] {
                let read = leaf.convert(&input, settings);
                claimed += usize::from(read.is_some());
                assert_eq!(
                    leaf.convert(&rebuilt, settings),
                    read,
                    "{leaf} {}",
                    path.display()
                );
            }
        }
        assert_eq!(claimed, 14);
    }
}
