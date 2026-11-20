use bencher_json::project::report::Adapter;

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        leaf::LeafResults,
        swift::benchmark::{Group, Release, THROUGHPUT_LABEL, insert},
    },
    results::adapter_results::AdapterResults,
};

/// Swift package-benchmark's default text output at version 1: a table per benchmark under its
/// target (`BenchmarkTool+PrettyPrinting.swift:194-232`).
///
/// Each value is the table's own, rounded in its display unit, and per inner iteration where a
/// metric is marked `*` in a benchmark with a scaling factor, so it can differ from Influx's
/// value for the same run. A line the harness never prints inside a table skips that benchmark,
/// and a second report, as `baseline read` of 2 baselines prints, refuses the input.
pub struct AdapterSwiftBenchmarkText;

impl Adaptable for AdapterSwiftBenchmarkText {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let lines = input.lines().collect::<Vec<_>>();
        if lines.iter().filter(|line| is_machine(line)).count() > 1 {
            return None;
        }
        let mut results = LeafResults::default();
        let mut benchmarks = Vec::new();
        let (mut report, mut target) = (false, None);
        let mut name = None;
        let mut rest = lines.as_slice();
        while let [line, tail @ ..] = rest {
            if is_machine(line) {
                (report, target) = (true, None);
            } else if let [title, rule, body @ ..] = tail
                && let Some(widths) = header(line, title, rule)
            {
                let (groups, after) = table(body, &widths, &mut results);
                if let (Some(groups), Some(target), Some(name)) = (groups, target, name) {
                    benchmarks.push((target, name, groups));
                } else {
                    results.skip_benchmark();
                }
                (name, rest) = (None, after);
                continue;
            } else if let [block, closing, after @ ..] = tail
                && report
                && is_target(line, block, closing)
            {
                (target, name, rest) = (Some(*block), None, after);
                continue;
            }
            name = Some(*line).filter(|line| !line.is_empty());
            rest = tail;
        }
        let release = Release::of(benchmarks.iter().flat_map(|(_, _, groups)| groups));
        for (target, name, groups) in benchmarks {
            insert(&mut results, target, name, groups, release);
        }
        results.finish_with_skips(Adapter::SwiftBenchmarkText)
    }
}

/// The line that opens a report, after its header (`BenchmarkTool+PrettyPrinting.swift:72-74`).
fn is_machine(line: &str) -> bool {
    line.starts_with("Host '") && line.ends_with(" GB memory, running:")
}

/// A target's name between 2 rules of `=` as long as it
/// (`BenchmarkTool+PrettyPrinting.swift:213-226`).
fn is_target(rule: &str, name: &str, closing: &str) -> bool {
    !name.is_empty()
        && rule == closing
        && rule.chars().all(|c| c == '=')
        && rule.chars().count() == name.chars().count()
}

/// The widths of a table's columns, from its opening rule, its title row, and the rule under it,
/// when the title row is the default grouping's (`BenchmarkTool+PrettyPrinting.swift:103-145`).
fn header(opening: &str, title: &str, rule: &str) -> Option<Vec<usize>> {
    let cells = cells(title)?;
    if cells.iter().map(|cell| cell.trim()).ne(TITLE) {
        return None;
    }
    let widths = cells
        .iter()
        .map(|cell| cell.chars().count())
        .collect::<Vec<_>>();
    (is_rule(opening, OPENING, &widths) && is_rule(rule, HEADER, &widths)).then_some(widths)
}

const TITLE: [&str; 9] = [
    "Metric", "p0", "p25", "p50", "p75", "p90", "p99", "p100", "Samples",
];

/// A table's rows from the line after its header up to its closing rule, with the lines after
/// it, or `None` with the lines from the first line that is neither a row nor a rule where the
/// harness prints one. A row the leaf cannot read is skipped and counted.
fn table<'a>(
    mut lines: &'a [&'a str],
    widths: &[usize],
    results: &mut LeafResults,
) -> (Option<Vec<Group>>, &'a [&'a str]) {
    let mut groups = Vec::new();
    loop {
        let [row, tail @ ..] = lines else {
            return (None, lines);
        };
        let Some(cells) = cells(row).filter(|cells| {
            cells.len() == widths.len()
                && cells
                    .iter()
                    .zip(widths)
                    .all(|(cell, width)| cell.chars().count() == *width)
        }) else {
            return (None, lines);
        };
        if let Some(group) = group(&cells) {
            groups.push(group);
        } else {
            results.skip_measure();
        }
        let [rule, after @ ..] = tail else {
            return (None, tail);
        };
        if is_rule(rule, CLOSING, widths) {
            return (Some(groups), after);
        }
        if !is_rule(rule, BETWEEN, widths) {
            return (None, tail);
        }
        lines = after;
    }
}

/// The cells of a row between its outer bars.
fn cells(row: &str) -> Option<Vec<&str>> {
    let inner = row.strip_prefix('│')?.strip_suffix('│')?;
    Some(inner.split('│').collect())
}

/// The rule's ends, its fill, and the mark between columns, each as wide as its column.
type Rule = (char, char, char, char);
const OPENING: Rule = ('╒', '═', '╤', '╕');
const HEADER: Rule = ('╞', '═', '╪', '╡');
const BETWEEN: Rule = ('├', '─', '┼', '┤');
const CLOSING: Rule = ('╘', '═', '╧', '╛');

fn is_rule(line: &str, (left, fill, mark, right): Rule, widths: &[usize]) -> bool {
    line.strip_prefix(left)
        .and_then(|line| line.strip_suffix(right))
        .is_some_and(|inner| {
            let segments = inner.split(mark).collect::<Vec<_>>();
            segments.len() == widths.len()
                && segments.iter().zip(widths).all(|(segment, width)| {
                    segment.chars().count() == *width && segment.chars().all(|c| c == fill)
                })
        })
}

/// A row's metric: its description and its 7 percentiles, each the printed number times its
/// display unit. The sample count is not stored.
fn group(cells: &[&str]) -> Option<Group> {
    let [description, values @ .., samples] = cells else {
        return None;
    };
    let (label, factor) = metric(description.trim())?;
    number(samples.trim())?;
    let values = values
        .iter()
        .map(|value| number(value.trim()).map(|value| value * factor))
        .collect::<Option<Vec<_>>>()?
        .try_into()
        .ok()?;
    Some(Group { label, values })
}

/// The label and the factor to base units of a row's description: the metric's description,
/// then its display unit in parentheses unless it is a plain count, then ` *` where its values
/// are per inner iteration (`BenchmarkResult.swift:415-445`). The label is the description with
/// every space removed, as Influx writes it, so both leaves name a custom metric alike.
fn metric(description: &str) -> Option<(String, f64)> {
    let description = description.strip_suffix(" *").unwrap_or(description);
    if let Some((metric, unit)) = description
        .strip_suffix(')')
        .and_then(|description| description.rsplit_once(" ("))
    {
        let label = metric.replace(' ', "");
        if let Some(factor) = Kind::of(&label).factor(Some(unit)) {
            return Some((label, factor));
        }
    }
    let label = description.replace(' ', "");
    Kind::of(&label).factor(None).map(|factor| (label, factor))
}

/// What a metric's display unit, which the text table prints after its description, can be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Time,
    Throughput,
    Count,
}

impl Kind {
    /// The kind of the metric `label` names: the 4 time metrics, throughput, and every other
    /// metric, custom metrics included, which count (`BenchmarkMetric.swift:148-155`).
    fn of(label: &str) -> Self {
        if TIME_LABELS.contains(&label) {
            Self::Time
        } else if label == THROUGHPUT_LABEL {
            Self::Throughput
        } else {
            Self::Count
        }
    }

    /// The factor from the text table's display `unit` to the base unit, where `None` is a
    /// description with no unit, or `None` for a unit the table never prints for this kind
    /// (`BenchmarkResult.swift:415-445`).
    fn factor(self, unit: Option<&str>) -> Option<f64> {
        let lookup = |units: &[(&str, f64)], unit: &str| {
            units
                .iter()
                .find(|(known, _)| *known == unit)
                .map(|(_, factor)| *factor)
        };
        match (self, unit) {
            (Self::Time, Some(unit)) => lookup(&TIME_UNITS, unit),
            (Self::Throughput, Some("#")) | (Self::Count, None) => Some(1.0),
            (Self::Throughput | Self::Count, Some(unit)) => lookup(&COUNT_UNITS, unit),
            (Self::Time | Self::Throughput, None) => None,
        }
    }
}

const TIME_LABELS: [&str; 4] = [
    "Time(wallclock)",
    "Time(totalCPU)",
    "Time(userCPU)",
    "Time(systemCPU)",
];
const TIME_UNITS: [(&str, f64); 6] = [
    ("ns", 1.0),
    ("μs", 1e3),
    ("ms", 1e6),
    ("s", 1e9),
    ("ks", 1e12),
    ("Ms", 1e15),
];
const COUNT_UNITS: [(&str, f64); 5] =
    [("K", 1e3), ("M", 1e6), ("G", 1e9), ("T", 1e12), ("P", 1e15)];

/// A cell's number: an integer, or `%.2e` from 10,000,000 up
/// (`BenchmarkTool+PrettyPrinting.swift:41-47`).
fn number(cell: &str) -> Option<f64> {
    let digits = |digits: &str, count: Option<usize>| {
        !digits.is_empty()
            && digits.bytes().all(|byte| byte.is_ascii_digit())
            && count.is_none_or(|count| digits.len() == count)
    };
    let printed = if let Some((mantissa, exponent)) = cell.split_once('e') {
        let exponent = exponent
            .strip_prefix('+')
            .or_else(|| exponent.strip_prefix('-'));
        mantissa
            .split_once('.')
            .is_some_and(|(whole, fraction)| digits(whole, Some(1)) && digits(fraction, Some(2)))
            && exponent.is_some_and(|exponent| digits(exponent, None) && exponent.len() >= 2)
    } else {
        digits(cell, None)
    };
    printed.then(|| cell.parse().ok()).flatten()
}

#[cfg(test)]
mod test_swift_benchmark_text {
    use std::collections::BTreeSet;

    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Allocations, CpuTime, Latency, Throughput, UserTime},
                gungraun::SysCount,
                swift::{LogicalIo, PhysicalIo, UnfreedAllocations},
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

    const ALPHA: &str = "swift/benchmark/text_alpha.txt";

    fn read(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/{suffix}"))
            .expect("Failed to read a fixture")
    }

    fn convert(input: &str) -> Option<AdapterResults> {
        Adapter::SwiftBenchmarkText.convert(input, Settings::new(None, BmfVersion::V1))
    }

    /// Kills a display unit left unapplied or applied to the wrong kind of metric, a `*` or a
    /// unit left in the label, a custom metric named with its spaces, throughput named as a
    /// metric that prefers smaller values, tags left in the name, and a leaf that records no
    /// leaf.
    #[test]
    fn adapter_swift_benchmark_text_display_units() {
        let results = convert_v1(Adapter::SwiftBenchmarkText, ALPHA);
        assert_eq!(results.resolved_adapter, Some(Adapter::SwiftBenchmarkText));
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(variants(&results), 11);
        let scaled = |values: [f64; 7], factor: f64| values.map(|value| value * factor);
        assert_variant(
            &results,
            "Alpha:Insert",
            r#"{"count":"4096","kind":"set"}"#,
            &[
                smaller(Allocations::SLUG_STR, [13.0; 7]),
                larger(
                    Throughput::SLUG_STR,
                    scaled([12.0, 11.0, 11.0, 11.0, 11.0, 9.0, 3.0], 1e3),
                ),
                smaller(
                    Latency::SLUG_STR,
                    scaled([86.0, 88.0, 89.0, 92.0, 95.0, 108.0, 294.0], 1e3),
                ),
            ],
        );
        assert_variant(
            &results,
            "Alpha:Fixed ms",
            "{}",
            &[
                larger(
                    Throughput::SLUG_STR,
                    [656.0, 644.0, 643.0, 639.0, 638.0, 633.0, 633.0],
                ),
                smaller(CpuTime::SLUG_STR, [0.0; 7]),
                smaller(Latency::SLUG_STR, [2e6; 7]),
            ],
        );
        assert_variant(
            &results,
            "Alpha:Custom metrics",
            "{}",
            &[
                larger(
                    "CacheHits",
                    [1_000.0, 978.0, 951.0, 922.0, 911.0, 900.0, 900.0],
                ),
                smaller("QueueDepth", [1.0, 3.0, 6.0, 10.0, 11.0, 12.0, 12.0]),
                smaller(
                    Latency::SLUG_STR,
                    [590.0, 591.0, 601.0, 611.0, 861.0, 1_091.0, 2_655.0],
                ),
            ],
        );
        assert_eq!(
            value(
                &results,
                "Alpha:Insert",
                r#"{"count":"1","kind":"array"}"#,
                Throughput::SLUG_STR,
                "max"
            ),
            Some(14e6)
        );
    }

    /// Kills a `%.2e` cell refused or misread, and a second target's tables read under the
    /// first target's name.
    #[test]
    fn adapter_swift_benchmark_text_targets() {
        let results = convert_v1(
            Adapter::SwiftBenchmarkText,
            "swift/benchmark/text_progress.txt",
        );
        assert_eq!(results.skipped, Skipped::default());
        for benchmark in ["Alpha:Sum array", "Beta:Sum array"] {
            assert_eq!(
                value(&results, benchmark, "{}", CpuTime::SLUG_STR, "max"),
                Some(1e7),
                "{benchmark}"
            );
        }
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
        assert_eq!(
            value(
                &results,
                "Beta:All metrics",
                "{}",
                UserTime::SLUG_STR,
                "max"
            ),
            Some(1e7)
        );
        assert_eq!(
            value(
                &results,
                "Alpha:All metrics",
                "{}",
                Latency::SLUG_STR,
                "max"
            ),
            None
        );
    }

    /// The text and Influx of one run's filter name the same benchmarks, variants, measures, and
    /// metrics, on Swift 6.3 with every metric Linux measures and on Swift 6.2 with jemalloc.
    ///
    /// Kills: a text label that differs from Influx's, such as a custom metric keeping its
    /// spaces, a description keeping its unit or its `*`, or a metric whose description the
    /// text leaf cannot read.
    #[test]
    fn adapter_swift_benchmark_text_names_as_influx() {
        for (text, influx) in [
            (ALPHA, "swift/benchmark/influx_alpha.csv"),
            (
                "swift/benchmark/text_swift62_jemalloc.txt",
                "swift/benchmark/influx_swift62_jemalloc.csv",
            ),
        ] {
            let text_results = convert_v1(Adapter::SwiftBenchmarkText, text);
            let influx_results = convert_v1(Adapter::SwiftBenchmarkInflux, influx);
            assert_eq!(text_results.skipped, Skipped::default(), "{text}");
            assert_eq!(names(&text_results), names(&influx_results), "{text}");
        }
    }

    fn names(results: &AdapterResults) -> BTreeSet<String> {
        results
            .inner
            .iter()
            .flat_map(|(benchmark, variants)| {
                variants.iter().flat_map(move |(parameters, measures)| {
                    measures.inner.iter().flat_map(move |(measure, metrics)| {
                        metrics.inner.keys().map(move |metric| {
                            format!("{benchmark} {parameters} {measure} {metric}")
                        })
                    })
                })
            })
            .collect()
    }

    /// A second report after the first refuses the input, a table with a line the harness never
    /// prints inside it skips that benchmark, counted, while the next table still reads, and a
    /// table with no report header before it has no target and is skipped.
    ///
    /// The capture is edited because the harness prints each report and table whole: a rule
    /// between 2 rows is replaced by a blank line, as an interleaved log holds, a value cell is
    /// widened by one space, as a merged line shifts it, and the report's `Host` line is changed.
    ///
    /// Kills: a second report read into the first, a row read where a rule belongs, a cell of
    /// another width read, a cut table read into the next one, and a target taken from the
    /// `Running Benchmarks` banner.
    #[test]
    fn adapter_swift_benchmark_text_strict_tables() {
        let alpha = read(ALPHA);
        assert_eq!(convert(&format!("{alpha}{alpha}")), None);
        let ruled = "│ Malloc (total) *               │         1 │";
        let (head, tail) = alpha.split_at(alpha.find(ruled).expect("Failed to find a row"));
        let (row, tail) = tail.split_at(tail.find('\n').expect("Failed to find a line") + 1);
        let (_, tail) = tail.split_at(tail.find('\n').expect("Failed to find a rule") + 1);
        let unruled = format!("{head}{row}\n{tail}");
        let widened = alpha.replacen(
            "│ CacheHits                      │      1000 │",
            "│ CacheHits                      │       1000 │",
            1,
        );
        for (edited, benchmark) in [
            (unruled, ("Alpha:Insert", r#"{"count":"1","kind":"array"}"#)),
            (widened, ("Alpha:Custom metrics", "{}")),
        ] {
            assert!(edited != alpha, "{benchmark:?}");
            let results = convert(&edited).unwrap_or_else(|| panic!("{benchmark:?}"));
            assert_eq!(
                results.skipped,
                Skipped {
                    benchmarks: 1,
                    ..Skipped::default()
                },
                "{benchmark:?}"
            );
            assert_eq!(variants(&results), 10, "{benchmark:?}");
            let (name, parameters) = benchmark;
            assert_eq!(
                value(&results, name, parameters, Latency::SLUG_STR, "max"),
                None,
                "{benchmark:?}"
            );
            assert_eq!(
                value(&results, "Alpha:Fixed ms", "{}", Latency::SLUG_STR, "max"),
                Some(2e6),
                "{benchmark:?}"
            );
        }
        let headless = alpha.replacen("Host 'localhost'", "Host: localhost", 1);
        let results = convert(&headless).expect("Failed to read the tables");
        assert_eq!((variants(&results), results.skipped.benchmarks), (0, 11));
    }

    /// `Malloc / free Δ *` from 1.34.1, beside no label that 1.35.0 first writes, is skipped and
    /// counted while the benchmark's other metrics read, and from 1.38.1, in a run that holds
    /// `Free (total) *`, it reads as unfreed allocations, also in a benchmark with no other
    /// metric.
    ///
    /// The 1.38.1 capture is edited because its every benchmark with the label also holds a label
    /// that 1.35.0 first writes: `Sum array`'s table, cut to its header and its
    /// `Malloc / free Δ *` row, is copied as a benchmark `Leak only` ahead of every other, as a
    /// hand-picked metric list prints it.
    ///
    /// Kills: the check removed, a check that any other label passes, and a check of each
    /// benchmark on its own or of the benchmarks before it.
    #[test]
    fn adapter_swift_benchmark_text_malloc_free_delta() {
        let old = convert_v1(
            Adapter::SwiftBenchmarkText,
            "swift/benchmark/text_1_34_1.txt",
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
        let alpha = read(ALPHA);
        let mut sum = alpha
            .lines()
            .skip_while(|line| *line != "Sum array")
            .skip(1);
        let header = sum.by_ref().take(3).collect::<Vec<_>>();
        let rows = sum.collect::<Vec<_>>();
        let row = |start: &str| {
            *rows
                .iter()
                .find(|line| line.starts_with(start))
                .expect("Failed to find a row")
        };
        let table = [
            vec!["Leak only"],
            header,
            vec![row("│ Malloc / free Δ *"), row("╘")],
        ]
        .concat()
        .join("\n");
        let first = "\nCustom metrics\n";
        assert_eq!(alpha.matches(first).count(), 1);
        let new = convert(&alpha.replacen(first, &format!("\n{table}\n{first}"), 1))
            .expect("Failed to read the run");
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
