use std::collections::BTreeSet;

use bencher_json::{BenchmarkName, ParameterSet, project::report::Adapter};
use rust_decimal::{Decimal, prelude::ToPrimitive as _};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        grpc::ghz::{Distribution, Run},
        leaf::LeafResults,
        percentile::percentile_name,
    },
    results::adapter_results::AdapterResults,
};

/// ghz's default text summaries of v0.80.0 on (`printer/template.go:4-22` at v0.121.0): a whole
/// summary with a new `Name:`, or the first with none, is a benchmark, and any other is skipped
/// and counted.
pub struct AdapterGrpcGhzText;

impl Adaptable for AdapterGrpcGhzText {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let lines = input.lines().collect::<Vec<_>>();
        let mut results = LeafResults::default();
        let mut read = BTreeSet::new();
        let mut index = 0;
        while let Some(line) = lines.get(index) {
            index += 1;
            if *line != SUMMARY
                || !lines
                    .get(index)
                    .is_some_and(|next| next.starts_with(NAME) || next.starts_with(COUNT))
            {
                continue;
            }
            let mut cursor = Cursor {
                lines: &lines,
                index,
            };
            let Some(run) = read_run(&mut cursor) else {
                results.skip_benchmark();
                continue;
            };
            index = cursor.index;
            if read.insert(run.benchmark.clone()) {
                run.insert(&mut results);
            } else {
                results.skip_benchmark();
            }
        }
        results.finish_with_skips(Adapter::GrpcGhzText)
    }
}

const SUMMARY: &str = "Summary:";
const NAME: &str = "  Name:\t\t";
const COUNT: &str = "  Count:\t";

/// The lines of the input, read in order.
struct Cursor<'a> {
    lines: &'a [&'a str],
    index: usize,
}

impl<'a> Cursor<'a> {
    fn next(&mut self) -> Option<&'a str> {
        let line = self.peek()?;
        self.index += 1;
        Some(line)
    }

    fn peek(&self) -> Option<&'a str> {
        self.lines.get(self.index).copied()
    }

    /// The next line, which must be `expected`.
    fn line(&mut self, expected: &str) -> Option<()> {
        (self.next()? == expected).then_some(())
    }

    /// The value after the next line's label, its colon, and its tab.
    fn value(&mut self, label: &str) -> Option<&'a str> {
        self.next()?.strip_prefix(label)?.strip_prefix(":\t")
    }

    /// Rows up to a blank line, each of which `row` must read.
    fn rows<T, F: Fn(&str) -> Option<T>>(&mut self, row: F) -> Option<Vec<T>> {
        let mut rows = Vec::new();
        loop {
            let line = self.next()?;
            if line.is_empty() {
                return Some(rows);
            }
            rows.push(row(line)?);
        }
    }
}

/// One summary after its `Summary:` line, or `None` when any line is not the one ghz prints
/// there or the name is not a benchmark name.
fn read_run(cursor: &mut Cursor<'_>) -> Option<Run> {
    let benchmark = if let Some(name) = cursor.peek().and_then(|line| line.strip_prefix(NAME)) {
        cursor.index += 1;
        Some(name.parse::<BenchmarkName>().ok()?)
    } else {
        None
    };
    let count = count_of(cursor.value("  Count")?)?;
    time(cursor.value("  Total")?)?;
    let slowest = time(cursor.value("  Slowest")?)?;
    let fastest = time(cursor.value("  Fastest")?)?;
    let average = time(cursor.value("  Average")?)?;
    let rps = fixed(cursor.value("  Requests/sec")?, 2)?.to_f64()?;
    cursor.line("")?;
    cursor.line("Response time histogram:")?;
    cursor.rows(histogram_row)?;
    cursor.line("Latency distribution:")?;
    let percentiles = cursor.rows(percentile_row)?;
    // ghz prints each block's title only when a row follows it, and a blank line after each. The
    // last blank line may be the end of the input: `bencher run` passes on a command's output
    // line by line, without its last line break.
    let statuses = if cursor.peek() == Some("Status code distribution:") {
        cursor.index += 1;
        cursor.rows(status_row).filter(|rows| !rows.is_empty())?
    } else {
        cursor.line("")?;
        Vec::new()
    };
    if cursor.peek() == Some("Error distribution:") {
        cursor.index += 1;
        error_rows(cursor)?;
    } else if cursor.peek().is_some() {
        cursor.line("")?;
    }
    Some(Run {
        benchmark,
        parameters: ParameterSet::default(),
        count,
        average,
        distribution: (!percentiles.is_empty()).then(|| Distribution {
            fastest,
            slowest,
            percentiles: percentiles
                .into_iter()
                .filter_map(Percentile::metric)
                .collect(),
        }),
        rps,
        statuses,
    })
}

/// `  <mark> [<count>] |∎∎∎`, the mark in milliseconds, padded after the mark and the count from
/// v0.101.0 on (`printer/printer.go:154-195` at v0.121.0), with a tab before the bar before that
/// (`:337-354` at v0.80.0).
fn histogram_row(line: &str) -> Option<()> {
    let (mark, rest) = line.strip_prefix("  ")?.split_once(" [")?;
    fixed(mark.trim_end_matches(' '), 3)?;
    let (count, bar) = rest.split_once(']')?;
    count_of(count)?;
    bar.strip_prefix('\t')
        .or_else(|| bar.strip_prefix(' ').map(|bar| bar.trim_start_matches(' ')))?
        .strip_prefix('|')?
        .chars()
        .all(|c| c == '∎')
        .then_some(())
}

/// `  <percent> % in <time> `.
fn percentile_row(line: &str) -> Option<Percentile> {
    let (percent, value) = line
        .strip_prefix("  ")?
        .strip_suffix(' ')?
        .split_once(" % in ")?;
    count_of(percent)?;
    let value = time(value)?;
    if percent == "0" {
        return Some(Percentile::Unset);
    }
    Some(Percentile::Latency(percentile_name(percent)?, value))
}

/// A row of the latency distribution.
enum Percentile {
    /// A percentile's metric name and its latency.
    Latency(String, f64),
    /// A slot ghz had no latency for, which it prints at 0 % (`runner/reporter.go:305-311`).
    Unset,
}

impl Percentile {
    fn metric(self) -> Option<(String, f64)> {
        match self {
            Self::Latency(metric, value) => Some((metric, value)),
            Self::Unset => None,
        }
    }
}

/// `  [<status>]   <count> responses   `, aligned by Go's tabwriter with 3 spaces of padding
/// (`printer/printer.go:207-218`).
fn status_row(line: &str) -> Option<(String, f64)> {
    let (status, rest) = line.strip_prefix("  [")?.split_once(']')?;
    let (count, rest) = rest
        .strip_prefix(PADDING)?
        .trim_start_matches(' ')
        .split_once(" responses")?;
    rest.strip_prefix(PADDING)?
        .bytes()
        .all(|byte| byte == b' ')
        .then_some((status.to_owned(), count_of(count)?))
}

/// The error rows up to a blank line or the end of the input, where a line that opens no row
/// continues the message before it and the last line of every row must end in the padding ghz's
/// tabwriter prints after each message (`  [%+v]\t%+s\t\n`, `printer/printer.go:226` at v0.121.0,
/// `:385` at v0.80.0), so a row cut short is refused.
fn error_rows(cursor: &mut Cursor<'_>) -> Option<()> {
    let first = cursor.next()?;
    error_row(first)?;
    let mut last = first;
    while let Some(line) = cursor.next().filter(|line| !line.is_empty()) {
        if error_row(line).is_some() {
            last.ends_with(PADDING).then_some(())?;
        }
        last = line;
    }
    last.ends_with(PADDING).then_some(())
}

/// `  [<count>]   <error>`.
fn error_row(line: &str) -> Option<()> {
    let (count, rest) = line.strip_prefix("  [")?.split_once(']')?;
    count_of(count)?;
    rest.starts_with(PADDING).then_some(())
}

const PADDING: &str = "   ";

/// A count, digits only.
fn count_of(cell: &str) -> Option<f64> {
    if cell.is_empty() || !cell.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    cell.parse().ok()
}

/// A time in nanoseconds: whole nanoseconds under 10 µs, else milliseconds or seconds with 2
/// decimals (`formatNanoUnit`, `printer/printer.go:123-135`).
fn time(cell: &str) -> Option<f64> {
    let (value, unit) = cell.split_once(' ')?;
    let factor: u64 = match unit {
        "ns" => return count_of(value),
        "ms" => 1_000_000,
        "s" => 1_000_000_000,
        _ => return None,
    };
    fixed(value, 2)?
        .checked_mul(Decimal::from(factor))?
        .to_f64()
}

/// A number printed with exactly `decimals` decimals.
fn fixed(cell: &str, decimals: usize) -> Option<Decimal> {
    let (whole, fraction) = cell.split_once('.')?;
    (!whole.is_empty()
        && fraction.len() == decimals
        && whole
            .bytes()
            .chain(fraction.bytes())
            .all(|byte| byte.is_ascii_digit()))
    .then(|| cell.parse().ok())
    .flatten()
}

#[cfg(test)]
mod test_grpc_ghz_text {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Latency, Throughput},
                grpc::{
                    StatusAborted, StatusAlreadyExists, StatusCancelled, StatusDataLoss,
                    StatusDeadlineExceeded, StatusFailedPrecondition, StatusInternal,
                    StatusInvalidArgument, StatusNotFound, StatusOk, StatusOutOfRange,
                    StatusPermissionDenied, StatusResourceExhausted, StatusUnauthenticated,
                    StatusUnavailable, StatusUnimplemented, StatusUnknown,
                },
                http::{FailedRequests, Requests},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::{
            grpc::ghz_text::AdapterGrpcGhzText,
            test_util::{
                Expected, assert_units, assert_value, assert_variant, convert_v1, value, variants,
            },
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::GrpcGhzText;

    fn named(suffix: &str) -> AdapterResults {
        let mut results = convert_v1(LEAF, &format!("grpc/ghz/{suffix}"));
        results.name_nameless(None);
        results
    }

    fn fixture(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/grpc/ghz/{suffix}"))
            .expect("Failed to read a fixture")
    }

    fn parse(input: &str) -> Option<AdapterResults> {
        AdapterGrpcGhzText::parse(input, Settings::new(None, BmfVersion::V1))
    }

    /// `latency` with ghz's mean, min, max, and the seven percentiles it prints.
    fn latency([mean, min, max]: [f64; 3], percentiles: [f64; 7]) -> Expected {
        let mut metrics = vec![("mean", mean), ("min", min), ("max", max)];
        metrics.extend(
            ["p10", "p25", "median", "p75", "p90", "p95", "p99"]
                .into_iter()
                .zip(percentiles),
        );
        (Latency::SLUG_STR, metrics)
    }

    fn count(measure: &'static str, value: f64) -> Expected {
        (measure, vec![("value", value)])
    }

    /// Kills a leaf that names a summary with no `Name:` itself, which a prefix would then go in
    /// front of, a time left in milliseconds, `Slowest` and `Fastest` swapped, a percentile row
    /// misnamed, `failed-requests` other than 0 for a run with no error, a leaf that records
    /// another leaf, and a skip counted for the histogram, of either layout.
    #[test]
    fn adapter_grpc_ghz_text_ok() {
        for (suffix, expected, rate) in [
            (
                "ok_summary.txt",
                latency(
                    [430_000.0, 60_000.0, 1_930_000.0],
                    [
                        160_000.0,
                        230_000.0,
                        370_000.0,
                        570_000.0,
                        800_000.0,
                        950_000.0,
                        1_170_000.0,
                    ],
                ),
                12_736.85,
            ),
            (
                "ok_summary_0_80_0.txt",
                latency(
                    [210_000.0, 40_000.0, 3_320_000.0],
                    [
                        100_000.0, 130_000.0, 160_000.0, 220_000.0, 300_000.0, 400_000.0, 790_000.0,
                    ],
                ),
                42_311.8,
            ),
        ] {
            let results = convert_v1(LEAF, &format!("grpc/ghz/{suffix}"));
            assert_eq!(results.resolved_adapter, Some(LEAF), "{suffix}");
            assert!(results.inner.is_empty(), "{suffix}");
            assert_eq!(
                results.nameless.as_ref().map(|nameless| nameless.harness),
                Some("ghz"),
                "{suffix}"
            );
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
            let results = named(suffix);
            assert_eq!(variants(&results), 1, "{suffix}");
            assert_variant(
                &results,
                "ghz",
                "{}",
                &[
                    expected,
                    (Throughput::SLUG_STR, vec![("mean", rate)]),
                    count(Requests::SLUG_STR, 2_000.0),
                    count(FailedRequests::SLUG_STR, 0.0),
                    count(StatusOk::SLUG_STR, 2_000.0),
                ],
            );
            assert_units(&results, &[]);
        }
    }

    /// Kills `Name:` ignored, read with its tabs, or read as a line ghz never prints there.
    #[test]
    fn adapter_grpc_ghz_text_name() {
        let results = convert_v1(LEAF, "grpc/ghz/name_summary.txt");
        assert!(results.nameless.is_none());
        assert_value(
            &results,
            ("greeter ok", "{}"),
            (Latency::SLUG_STR, "median"),
            280_000.0,
        );
    }

    /// Kills `Average:` left out, an OK status counted as failed, a status other than OK left out
    /// of the failed requests or mapped to another's measure, a status or error row of another
    /// width refused, and a time in seconds read with another factor.
    #[test]
    fn adapter_grpc_ghz_text_mixed_statuses() {
        assert_variant(
            &named("mixed_timeout_summary.txt"),
            "ghz",
            "{}",
            &[
                latency(
                    [48_170_000.0, 40_000.0, 1_560_000.0],
                    [
                        80_000.0, 100_000.0, 130_000.0, 210_000.0, 310_000.0, 370_000.0, 560_000.0,
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 198.66)]),
                count(Requests::SLUG_STR, 2_000.0),
                count(FailedRequests::SLUG_STR, 609.0),
                count(StatusOk::SLUG_STR, 1_391.0),
                count(StatusNotFound::SLUG_STR, 192.0),
                count(StatusInternal::SLUG_STR, 200.0),
                count(StatusResourceExhausted::SLUG_STR, 121.0),
                count(StatusDeadlineExceeded::SLUG_STR, 96.0),
            ],
        );
        let results = named("mixed_count_errors_summary.txt");
        assert_value(
            &results,
            ("ghz", "{}"),
            (Latency::SLUG_STR, "max"),
            1_000_000_000.0,
        );
        assert_value(
            &results,
            ("ghz", "{}"),
            (Latency::SLUG_STR, "p99"),
            1_000_000_000.0,
        );
        for (suffix, failed, calls) in [
            ("mixed_timeout_summary_0_80_0.txt", 591.0, 2_000.0),
            ("server_restart_summary.txt", 765_150.0, 770_453.0),
        ] {
            let results = named(suffix);
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
            assert_value(
                &results,
                ("ghz", "{}"),
                (FailedRequests::SLUG_STR, "value"),
                failed,
            );
            assert_value(
                &results,
                ("ghz", "{}"),
                (Requests::SLUG_STR, "value"),
                calls,
            );
        }
    }

    /// Kills ghz's placeholder `Fastest` and `Slowest` of `0 ns` written, or counted as skipped,
    /// for a run with no OK call, its empty distribution refused, and its mean, rate, and counts
    /// left out with them.
    #[test]
    fn adapter_grpc_ghz_text_no_ok_call() {
        for (suffix, [mean, rate, calls]) in [
            ("unavailable_summary.txt", [270_000.0, 15_736.19, 2_000.0]),
            ("refused_summary.txt", [50_000.0, 15_152.56, 200.0]),
        ] {
            let results = named(suffix);
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
            assert_variant(
                &results,
                "ghz",
                "{}",
                &[
                    (Latency::SLUG_STR, vec![("mean", mean)]),
                    (Throughput::SLUG_STR, vec![("mean", rate)]),
                    count(Requests::SLUG_STR, calls),
                    count(FailedRequests::SLUG_STR, calls),
                    count(StatusUnavailable::SLUG_STR, calls),
                ],
            );
        }
    }

    /// Kills a time scaled in floating point, which gives `64989999.99999999` for `64.99 ms`.
    #[test]
    fn adapter_grpc_ghz_text_scales_in_decimal() {
        let results = named("deadline_count_errors_summary.txt");
        for (metric, expected) in [
            ("min", 64_990_000.0),
            ("p10", 65_020_000.0),
            ("median", 65_230_000.0),
        ] {
            assert_eq!(
                value(&results, "ghz", "{}", Latency::SLUG_STR, metric),
                Some(expected),
                "{metric}"
            );
        }
    }

    /// Kills ghz's placeholder `Average` of `0 ns` written for a run that counted no call, a
    /// summary with no status block refused, and its rate and counts left out.
    #[test]
    fn adapter_grpc_ghz_text_no_call_counted() {
        let results = named("skip_all_summary.txt");
        assert_eq!(results.skipped, Skipped::default());
        assert_variant(
            &results,
            "ghz",
            "{}",
            &[
                (Throughput::SLUG_STR, vec![("mean", 0.0)]),
                count(Requests::SLUG_STR, 0.0),
                count(FailedRequests::SLUG_STR, 0.0),
            ],
        );
    }

    /// Kills any status Go prints mapped to another code's measure, Go's `Canceled` not mapped to
    /// the spec's `cancelled`, and a code gRPC does not register dropped or given a built-in.
    #[test]
    fn adapter_grpc_ghz_text_every_status_code() {
        let results = named("codes_summary.txt");
        for measure in [
            StatusOk::SLUG_STR,
            StatusCancelled::SLUG_STR,
            StatusUnknown::SLUG_STR,
            StatusInvalidArgument::SLUG_STR,
            StatusDeadlineExceeded::SLUG_STR,
            StatusNotFound::SLUG_STR,
            StatusAlreadyExists::SLUG_STR,
            StatusPermissionDenied::SLUG_STR,
            StatusResourceExhausted::SLUG_STR,
            StatusFailedPrecondition::SLUG_STR,
            StatusAborted::SLUG_STR,
            StatusOutOfRange::SLUG_STR,
            StatusUnimplemented::SLUG_STR,
            StatusInternal::SLUG_STR,
            StatusUnavailable::SLUG_STR,
            StatusDataLoss::SLUG_STR,
            StatusUnauthenticated::SLUG_STR,
            "grpc-status-17",
        ] {
            assert_value(&results, ("ghz", "{}"), (measure, "value"), 10.0);
        }
        assert_value(
            &results,
            ("ghz", "{}"),
            (FailedRequests::SLUG_STR, "value"),
            170.0,
        );
        assert_units(&results, &[("grpc-status-17", "responses")]);
    }

    /// Kills a summary refused, read otherwise, or counted as skipped when `bencher run` passes it
    /// on: line by line, from the first line that is not blank, without the last line break
    /// (`services/cli/src/bencher/sub/run/runner/command.rs`), so ghz's last blank line, after
    /// the status rows or after the error rows, is gone. The test builds that input from each
    /// real capture, as `bencher run` builds it from ghz's output.
    #[test]
    fn adapter_grpc_ghz_text_reads_bencher_run_stdout() {
        for suffix in [
            "ok_summary.txt",
            "ok_summary_0_80_0.txt",
            "name_summary.txt",
            "mixed_timeout_summary.txt",
            "mixed_timeout_summary_0_80_0.txt",
            "mixed_count_errors_summary.txt",
            "deadline_count_errors_summary.txt",
            "unavailable_summary.txt",
            "refused_summary.txt",
            "skip_all_summary.txt",
            "codes_summary.txt",
            "server_restart_summary.txt",
        ] {
            let output = fixture(suffix);
            let stdout = bencher_run_stdout(&output);
            assert_eq!(format!("\n{stdout}\n"), output, "{suffix}");
            let results = parse(&output).expect("Failed to parse the summary");
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
            assert_eq!(parse(&stdout), Some(results), "{suffix}");
        }
    }

    /// Kills an error row whose message holds a line break refused at its next line, which skips
    /// the whole summary, of either version, in either form of the output.
    #[test]
    fn adapter_grpc_ghz_text_multiline_error() {
        for (suffix, mean, rate) in [
            ("multiline_summary.txt", 220_000.0, 5_056.62),
            ("multiline_summary_0_80_0.txt", 100_000.0, 13_477.85),
        ] {
            let output = fixture(suffix);
            for input in [output.clone(), bencher_run_stdout(&output)] {
                let mut results = parse(&input).expect("Failed to parse the summary");
                results.name_nameless(None);
                assert_eq!(results.skipped, Skipped::default(), "{suffix}");
                assert_variant(
                    &results,
                    "ghz",
                    "{}",
                    &[
                        (Latency::SLUG_STR, vec![("mean", mean)]),
                        (Throughput::SLUG_STR, vec![("mean", rate)]),
                        count(Requests::SLUG_STR, 20.0),
                        count(FailedRequests::SLUG_STR, 20.0),
                        count(StatusInternal::SLUG_STR, 20.0),
                    ],
                );
            }
        }
    }

    /// Kills a second summary with another `Name:` skipped, and a summary whose name an earlier
    /// one had read over it, in either form of the output.
    #[test]
    fn adapter_grpc_ghz_text_reads_each_named_summary() {
        let output = fixture("names_summary.txt");
        for input in [output.clone(), bencher_run_stdout(&output)] {
            let results = parse(&input).expect("Failed to parse the summaries");
            assert!(results.nameless.is_none());
            assert_eq!(
                results.skipped,
                Skipped {
                    benchmarks: 1,
                    ..Skipped::default()
                }
            );
            assert_eq!(variants(&results), 2);
            assert_value(&results, ("a", "{}"), (Requests::SLUG_STR, "value"), 500.0);
            assert_value(
                &results,
                ("a", "{}"),
                (Latency::SLUG_STR, "mean"),
                680_000.0,
            );
            assert_value(
                &results,
                ("b", "{}"),
                (FailedRequests::SLUG_STR, "value"),
                157.0,
            );
            assert_value(&results, ("b", "{}"), (StatusOk::SLUG_STR, "value"), 343.0);
        }
    }

    /// Kills a nameless summary after the first read over it, or not counted as skipped, of
    /// either version, in either form of the output.
    #[test]
    fn adapter_grpc_ghz_text_skips_a_second_nameless_summary() {
        for (suffix, mean) in [
            ("concat_summary.txt", 340_000.0),
            ("nameless_pair_summary_0_80_0.txt", 200_000.0),
        ] {
            let output = fixture(suffix);
            for input in [output.clone(), bencher_run_stdout(&output)] {
                let mut results = parse(&input).expect("Failed to parse the summaries");
                assert_eq!(
                    results.skipped,
                    Skipped {
                        benchmarks: 1,
                        ..Skipped::default()
                    },
                    "{suffix}"
                );
                results.name_nameless(None);
                assert_eq!(variants(&results), 1, "{suffix}");
                assert_value(&results, ("ghz", "{}"), (Latency::SLUG_STR, "mean"), mean);
                assert_value(
                    &results,
                    ("ghz", "{}"),
                    (FailedRequests::SLUG_STR, "value"),
                    0.0,
                );
                assert_value(
                    &results,
                    ("ghz", "{}"),
                    (StatusOk::SLUG_STR, "value"),
                    500.0,
                );
            }
        }
    }

    /// Kills a summary required at the start or the end of the input, and a lone `Summary:` line
    /// taken for one: the user's own lines before and after a summary, as `bencher run` captures
    /// a script's whole output, leave it read as it is alone. No capture holds such lines, so the
    /// test adds them to a real capture.
    #[test]
    fn adapter_grpc_ghz_text_user_lines_around_a_summary() {
        let summary = fixture("mixed_timeout_summary.txt");
        let alone = parse(&summary);
        assert!(alone.is_some());
        assert_eq!(
            parse(&format!("starting\nSummary:\n{summary}done\n\n")),
            alone
        );
    }

    /// Kills a summary cut short that is read in part, or claimed with no skip counted: cut
    /// inside a status row, before the blank line that ends the status block, inside the error
    /// block's title, inside its first row, and inside the percentiles. No capture is cut, so the
    /// test cuts a real capture where an output cut short ends.
    #[test]
    fn adapter_grpc_ghz_text_cut_summary_is_skipped_and_counted() {
        let summary = fixture("mixed_timeout_summary.txt");
        let at = |text: &str| summary.find(text).expect("Failed to find the cut");
        for end in [
            at("[Internal]") + "[Internal]   ".len(),
            at("\nError distribution:"),
            at("Error distribution:") + "Error dist".len(),
            at("  [192]   rpc") + "  [192]   rpc".len(),
            at("  75 % in"),
        ] {
            let input = summary.get(..end).expect("Failed to cut the summary");
            let results = parse(input).expect("Failed to parse the cut summary");
            assert!(results.nameless.is_none(), "{input}");
            assert_eq!(
                results.skipped,
                Skipped {
                    benchmarks: 1,
                    ..Skipped::default()
                },
                "{input}"
            );
        }
    }

    /// Kills a row whose last line has no padding read as a message that continues, which takes
    /// the output after a summary cut inside an error row as the row's message, so the next
    /// summary is lost and the cut one read, in either form of the output. No capture is cut, so
    /// the test cuts a real capture and puts another real capture after it, byte for byte.
    #[test]
    fn adapter_grpc_ghz_text_cut_error_row_before_more_output() {
        let cut = |suffix: &str, text: &str| {
            let summary = fixture(suffix);
            let end = summary.find(text).expect("Failed to find the cut") + text.len();
            summary
                .get(..end)
                .expect("Failed to cut the summary")
                .to_owned()
        };
        let mixed = cut("mixed_timeout_summary.txt", "  [192]   rpc");
        let multiline = cut("multiline_summary.txt", "gorout");
        let names = fixture("names_summary.txt");
        let pair = fixture("nameless_pair_summary_0_80_0.txt");
        let other = |path: &str| {
            std::fs::read_to_string(format!("./tool_output/http/{path}"))
                .expect("Failed to read a fixture")
        };
        let hey = other("hey/fast.txt");
        let foreign = format!("{}{hey}", other("oha/fast.txt"));
        let a_and_b = [("a", 500.0, 500.0), ("b", 500.0, 343.0)];
        let nameless = [("ghz", 500.0, 500.0)];
        for (output, skipped, expected) in [
            (format!("{mixed}{names}"), 2, &a_and_b[..]),
            (format!("{mixed}{pair}"), 2, &nameless[..]),
            (format!("{mixed}{foreign}"), 1, &[][..]),
            (format!("{mixed}{hey}"), 1, &[][..]),
            (multiline.clone(), 1, &[][..]),
            (format!("{multiline}{names}"), 2, &a_and_b[..]),
        ] {
            for input in [output.clone(), bencher_run_stdout(&output)] {
                let mut results = parse(&input).expect("Failed to parse the output");
                assert_eq!(
                    results.skipped,
                    Skipped {
                        benchmarks: skipped,
                        ..Skipped::default()
                    },
                    "{input}"
                );
                results.name_nameless(None);
                assert_eq!(variants(&results), expected.len(), "{input}");
                for &(benchmark, requests, ok) in expected {
                    assert_value(
                        &results,
                        (benchmark, "{}"),
                        (Requests::SLUG_STR, "value"),
                        requests,
                    );
                    assert_value(
                        &results,
                        (benchmark, "{}"),
                        (StatusOk::SLUG_STR, "value"),
                        ok,
                    );
                }
            }
        }
    }

    /// Kills a row reader that passes over a line ghz never prints in a block, or skips it: a
    /// histogram row with oha's bar or oha's mark, a percentile row as hey prints it, a time in
    /// `secs`, a status row as oha prints it, a status row cut short, an error row as oha prints
    /// it, and an error block that opens with the second line of a message. No capture holds
    /// such a line, so the test puts each in place of a row of a real capture.
    #[test]
    fn adapter_grpc_ghz_text_refuses_a_foreign_row() {
        for (suffix, row, foreign) in [
            ("ok_summary.txt", "  0.247 [544] |", "  0.247 [544] |■■■■"),
            ("ok_summary.txt", "  0.247 [544] |", "  0.247 ms [544] |"),
            (
                "ok_summary.txt",
                "  10 % in 0.16 ms \n",
                "  10% in 0.0002 secs\n",
            ),
            (
                "ok_summary.txt",
                "  25 % in 0.23 ms \n",
                "  25 % in 0.23 secs \n",
            ),
            (
                "ok_summary.txt",
                "  [OK]   2000 responses   \n",
                "  [200] 2000 responses\n",
            ),
            (
                "ok_summary.txt",
                "  [OK]   2000 responses   \n",
                "  [OK]   2000 respon\n",
            ),
            ("mixed_timeout_summary.txt", "  [192]   rpc", "  [192] rpc"),
            (
                "multiline_summary.txt",
                "  [20]   rpc error: code = Internal desc = panic: boom\n",
                "",
            ),
        ] {
            let summary = fixture(suffix);
            assert!(parse(&summary).is_some_and(|results| results.nameless.is_some()));
            let input = summary.replacen(row, foreign, 1);
            assert_ne!(input, summary, "{row}");
            let results = parse(&input).expect("Failed to parse the summary");
            assert!(results.nameless.is_none(), "{foreign}");
            assert_eq!(results.skipped.benchmarks, 1, "{foreign}");
        }
    }

    /// Kills an error row on one line read with no padding after its text, which ghz's tabwriter
    /// always prints there, when another row follows it. No capture holds such a row, so the test
    /// trims one in a real capture.
    #[test]
    fn adapter_grpc_ghz_text_refuses_an_unpadded_error_row() {
        let summary = fixture("mixed_timeout_summary.txt");
        let row = summary
            .lines()
            .find(|line| line.starts_with("  [192]   "))
            .expect("Failed to find the row");
        let input = summary.replacen(row, row.trim_end(), 1);
        assert_ne!(input, summary);
        let results = parse(&input).expect("Failed to parse the summary");
        assert!(results.nameless.is_none());
        assert_eq!(results.skipped.benchmarks, 1);
    }

    /// Kills a percentile slot ghz prints at 0 % for a latency of 0 (`runner/reporter.go:305-311`)
    /// read as `min` over `Fastest`, or counted as skipped. No capture holds one, so the test puts
    /// it in place of a row of a real capture.
    #[test]
    fn adapter_grpc_ghz_text_unset_percentile_slot() {
        let summary = fixture("ok_summary.txt").replace("  10 % in 0.16 ms ", "  0 % in 0 ns ");
        let mut results = parse(&summary).expect("Failed to parse the summary");
        results.name_nameless(None);
        assert_eq!(results.skipped, Skipped::default());
        assert_value(
            &results,
            ("ghz", "{}"),
            (Latency::SLUG_STR, "min"),
            60_000.0,
        );
        assert_eq!(value(&results, "ghz", "{}", Latency::SLUG_STR, "p10"), None);
    }

    /// `output` as `bencher run` passes it on: line by line, from the first line that is not
    /// blank, without the last line break.
    fn bencher_run_stdout(output: &str) -> String {
        output
            .lines()
            .skip_while(|line| line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
