use bencher_json::project::{
    measure::built_in::{
        BuiltInMeasure as _,
        default::{BytesThroughput, Latency, Throughput},
        http::{DnsDialupTime, DnsLookupTime, SizePerRequest, TimeToFirstByte},
    },
    report::Adapter,
};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::Finite,
        http::{
            k6::scale,
            oha::{Metrics, Run},
        },
        leaf::LeafResults,
        percentile::percentile_name,
    },
    results::adapter_results::AdapterResults,
};

/// oha's default text summary (`printer.rs:479-685` at 1.17.0, `:377-549` at 1.8.0).
///
/// A summary starts at a `Summary:` line followed by its `  Success rate:` line. A summary cut
/// short is skipped and counted, as is a second summary, which would name the same benchmark.
pub struct AdapterHttpOhaText;

impl Adaptable for AdapterHttpOhaText {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let lines = input.lines().collect::<Vec<_>>();
        let mut cursor = Cursor {
            lines: &lines,
            index: 0,
        };
        let mut results = LeafResults::default();
        let mut read = false;
        while let Some(line) = cursor.next() {
            if line != SUMMARY
                || !cursor
                    .peek()
                    .is_some_and(|next| next.starts_with(SUCCESS_RATE))
            {
                continue;
            }
            let start = cursor.index;
            let Some(run) = read_run(&mut cursor) else {
                cursor.index = start;
                results.skip_benchmark();
                continue;
            };
            if read {
                results.skip_benchmark();
            } else {
                run.insert(&mut results);
                read = true;
            }
        }
        results.finish_with_skips(Adapter::HttpOhaText)
    }
}

const SUMMARY: &str = "Summary:";
const SUCCESS_RATE: &str = "  Success rate:\t";

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

    /// The value after the next line's label and its tab.
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

    /// Rows for as long as `row` reads them, leaving the first line it does not, or `None` when
    /// that line still starts as a row does, as a row cut short or garbled does.
    fn rows_while<T, F: Fn(&str) -> Option<T>>(&mut self, row: F) -> Option<Vec<T>> {
        let mut rows = Vec::new();
        while let Some(value) = self.peek().and_then(&row) {
            rows.push(value);
            self.index += 1;
        }
        (!self.peek().is_some_and(|line| line.starts_with("  ["))).then_some(rows)
    }
}

/// One summary after its `Summary:` line, or `None` when any line is not the one oha prints
/// there, so a summary cut short never reads into what follows.
fn read_run(cursor: &mut Cursor<'_>) -> Option<Run> {
    let success_ratio = number(cursor.value("  Success rate")?.strip_suffix('%')?)?;
    let total = cursor.value("  Total")?;
    let units = Units::of(total);
    units.time(total)?;
    let slowest = units.time(cursor.value("  Slowest")?)?;
    let fastest = units.time(cursor.value("  Fastest")?)?;
    let average = units.time(cursor.value("  Average")?)?;
    let rate = number(cursor.value("  Requests/sec")?)?;
    cursor.line("")?;
    size(cursor.value("  Total data")?)?;
    let size_per_request = cursor.value("  Size/request")?;
    let size_per_request = if size_per_request == "NaN" {
        Finite::default()
    } else {
        size(size_per_request)?
    };
    let size_per_sec = size(cursor.value("  Size/sec")?)?;
    cursor.line("")?;
    let mut latency = distribution(cursor, units, "")?;
    latency.extend([
        (MEAN.to_owned(), average),
        ("min".to_owned(), fastest),
        ("max".to_owned(), slowest),
    ]);
    let breakdown = if cursor.peek() == Some("Response time histogram (2xx only):") {
        Some((
            distribution(cursor, units, " (2xx only)")?,
            distribution(cursor, units, " (4xx + 5xx only)")?,
        ))
    } else {
        None
    };
    cursor.line("")?;
    cursor.line("Details (average, fastest, slowest):")?;
    let mut measures = vec![(Latency::name_id(), latency)];
    let dns_dialup = triple(units, cursor.value("  DNS+dialup")?)?;
    let dns_lookup = triple(units, cursor.value("  DNS-lookup")?)?;
    // oha prints the first byte's line from 1.17.0 on.
    if cursor
        .peek()
        .is_some_and(|line| line.starts_with("  First byte:"))
    {
        measures.push((
            TimeToFirstByte::name_id(),
            triple(units, cursor.value("  First byte")?)?,
        ));
    }
    measures.extend([
        (DnsDialupTime::name_id(), dns_dialup),
        (DnsLookupTime::name_id(), dns_lookup),
        (Throughput::name_id(), vec![(MEAN.to_owned(), rate)]),
    ]);
    cursor.line("")?;
    cursor.line("Status code distribution:")?;
    let status_codes = cursor.rows_while(status_code)?;
    let errors = errors(cursor)?;
    Some(Run {
        measures,
        received: vec![
            (
                BytesThroughput::name_id(),
                vec![(MEAN.to_owned(), size_per_sec)],
            ),
            (
                SizePerRequest::name_id(),
                vec![(MEAN.to_owned(), size_per_request)],
            ),
        ],
        breakdown,
        success_ratio,
        status_codes,
        errors,
    })
}

/// The error rows after the status rows, which oha heads with a blank line and its title only
/// when at least 1 row follows (`printer.rs:676-682` at 1.17.0, `:540-546` at 1.8.0), or `None`
/// when no row or a cut row follows the title, or the title is cut short. A blank line that ends
/// the input reads as no error: a cut there prints what a run with no error prints.
fn errors(cursor: &mut Cursor<'_>) -> Option<Vec<(String, f64)>> {
    if cursor.peek() != Some("") {
        return Some(Vec::new());
    }
    let Some(title) = cursor.lines.get(cursor.index + 1).copied() else {
        return Some(Vec::new());
    };
    if title == ERROR_DISTRIBUTION {
        cursor.index += 2;
        return cursor.rows_while(error).filter(|rows| !rows.is_empty());
    }
    (title.is_empty() || !ERROR_DISTRIBUTION.starts_with(title)).then(Vec::new)
}

const ERROR_DISTRIBUTION: &str = "Error distribution:";

const MEAN: &str = "mean";

/// How a summary prints its times. oha before 1.9.0 prints every time in `secs` and each
/// histogram label with no unit (`printer.rs:397-412`, `:499-512`, `:573`, `:619` at 1.8.0);
/// later versions print one unit that fits after every time and label.
#[derive(Clone, Copy)]
enum Units {
    Secs,
    Scaled,
}

impl Units {
    /// The units of the summary whose `Total:` time is `total`.
    fn of(total: &str) -> Self {
        if total.ends_with(" secs") {
            Self::Secs
        } else {
            Self::Scaled
        }
    }

    /// A time in nanoseconds.
    fn time(self, cell: &str) -> Option<Finite> {
        match self {
            Self::Secs => Some(
                number(cell.strip_suffix(" secs")?)?.map(|value| scale(value, NANOS_PER_SECOND)),
            ),
            Self::Scaled => time(cell),
        }
    }

    /// A histogram bar's label, in seconds or in nanoseconds.
    fn label(self, cell: &str) -> Option<Finite> {
        match self {
            Self::Secs => number(cell),
            Self::Scaled => time(cell),
        }
    }
}

/// A response time histogram, which is skipped, then its distribution's percentiles, each block
/// headed with `qualifier` after its title.
fn distribution(cursor: &mut Cursor<'_>, units: Units, qualifier: &str) -> Option<Metrics> {
    cursor.line(&format!("Response time histogram{qualifier}:"))?;
    cursor.rows(|line| histogram_row(units, line))?;
    cursor.line(&format!("Response time distribution{qualifier}:"))?;
    cursor.rows(|line| percentile_row(units, line))
}

/// `  <label> [<count>] |■■■`, the bar's label right aligned and its count padded.
fn histogram_row(units: Units, line: &str) -> Option<()> {
    let (label, rest) = line.trim_start_matches(' ').split_once(" [")?;
    units.label(label)?;
    let (count, bar) = rest.split_once(']')?;
    count_of(count)?;
    bar.trim_start_matches(' ')
        .strip_prefix('|')?
        .chars()
        .all(|c| c == '■')
        .then_some(())
}

/// `  <percent>% in <time>`.
fn percentile_row(units: Units, line: &str) -> Option<(String, Finite)> {
    let (percent, value) = line.strip_prefix("  ")?.split_once("% in ")?;
    Some((percentile_name(percent)?, units.time(value)?))
}

/// `<time>, <time>, <time>`: the mean, min, and max.
fn triple(units: Units, cells: &str) -> Option<Metrics> {
    let mut times = cells.split(", ");
    let metrics = [MEAN, "min", "max"]
        .into_iter()
        .map(|metric| Some((metric.to_owned(), units.time(times.next()?)?)))
        .collect::<Option<Metrics>>()?;
    times.next().is_none().then_some(metrics)
}

/// `  [<code>] <count> responses`.
fn status_code(line: &str) -> Option<(u16, f64)> {
    let (code, count) = line.strip_prefix("  [")?.split_once("] ")?;
    Some((
        code.parse().ok()?,
        count_of(count.strip_suffix(" responses")?)?,
    ))
}

/// `  [<count>] <message>`.
fn error(line: &str) -> Option<(String, f64)> {
    let (count, message) = line.strip_prefix("  [")?.split_once("] ")?;
    Some((message.to_owned(), count_of(count)?))
}

/// A count, digits only.
fn count_of(cell: &str) -> Option<f64> {
    if cell.is_empty() || !cell.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    cell.parse().ok()
}

/// A number as oha prints it, `NaN`, `inf`, and `-inf` included.
fn number(cell: &str) -> Option<Finite> {
    Some(Finite::from(Some(cell.parse::<f64>().ok()?)))
}

/// A time in nanoseconds, from the units oha's summary prints from 1.9.0 on
/// (`timescale.rs:21-34` at 1.17.0), `10 sec` and `10 min` included, which 1.9.0 to 1.15.x print
/// for a slowest response over 10 s and over 10 minutes (`TimeScale::from_f64` in `printer.rs:445`
/// at 1.12.0).
fn time(cell: &str) -> Option<Finite> {
    let (value, unit) = cell.split_once(' ')?;
    let factors: &[u32] = match unit {
        "ns" => &[],
        "us" => &[1_000],
        "ms" => &[1_000_000],
        "sec" => &[NANOS_PER_SECOND],
        "10 sec" => &[10, NANOS_PER_SECOND],
        "min" => &[60, NANOS_PER_SECOND],
        "10 min" => &[600, NANOS_PER_SECOND],
        "hr" => &[3_600, NANOS_PER_SECOND],
        _ => return None,
    };
    Some(number(value)?.map(|value| {
        factors
            .iter()
            .fold(value, |value, factor| scale(value, *factor))
    }))
}

const NANOS_PER_SECOND: u32 = 1_000_000_000;

/// A size in bytes, from the binary units `byte-unit` prints, `B` with no fraction.
fn size(cell: &str) -> Option<Finite> {
    let (value, unit) = cell.split_once(' ')?;
    let power = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"]
        .iter()
        .position(|prefix| *prefix == unit)?;
    Some(
        number(value)?
            .map(|value| (0..power).fold(value, |value, _| scale(value, BYTES_PER_KIBIBYTE))),
    )
}

const BYTES_PER_KIBIBYTE: u32 = 1_024;

#[cfg(test)]
mod test_http_oha_text {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{BytesThroughput, Latency, Throughput},
                http::{
                    DnsDialupTime, DnsLookupTime, FailedRequests, Responses1xx, Responses2xx,
                    Responses3xx, Responses4xx, Responses5xx, SizePerRequest, SuccessRatio,
                    TimeToFirstByte, Timeouts,
                },
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::{
            http::oha_text::AdapterHttpOhaText,
            test_util::{
                Expected, assert_units, assert_value, assert_variant, convert_v1, value, variants,
            },
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpOhaText;

    fn named(suffix: &str) -> AdapterResults {
        let mut results = convert_v1(LEAF, &format!("http/oha/{suffix}"));
        results.name_nameless(None);
        results
    }

    fn fixture(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/http/oha/{suffix}"))
            .expect("Failed to read a fixture")
    }

    /// The nine percentiles oha prints, in order from `p10` to `p99_99`.
    fn percentiles(values: [f64; 9]) -> Expected {
        (
            Latency::SLUG_STR,
            [
                "p10", "p25", "median", "p75", "p90", "p95", "p99", "p99_9", "p99_99",
            ]
            .into_iter()
            .zip(values)
            .collect(),
        )
    }

    /// Kills a leaf that names the benchmark itself, one that records another leaf, and a skip
    /// counted for a histogram or a value that is not a number.
    #[test]
    fn adapter_http_oha_text_is_one_nameless_benchmark() {
        for suffix in ["fast.txt", "drop.txt", "mixed_timeout_breakdown.txt"] {
            let results = convert_v1(LEAF, &format!("http/oha/{suffix}"));
            assert_eq!(results.resolved_adapter, Some(LEAF), "{suffix}");
            assert!(results.inner.is_empty(), "{suffix}");
            assert_eq!(
                results.nameless.as_ref().map(|nameless| nameless.harness),
                Some("oha"),
                "{suffix}"
            );
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
        }
    }

    /// Kills a time left in microseconds, `Slowest` and `Fastest` swapped, a distribution row
    /// misnamed, the success rate scaled again, a timeout left out of the failed requests, a
    /// status code left out of its class, the byte rate or the size without their direction,
    /// and the two halves of the success breakdown swapped or read from the whole run.
    #[test]
    fn adapter_http_oha_text_measures() {
        let results = named("mixed_timeout_breakdown.txt");
        assert_eq!(variants(&results), 4);
        let (latency, mut metrics) = percentiles([
            23_817.0, 32_461.0, 46_723.0, 81_767.0, 192_492.0, 322_213.0, 444_597.0, 517_161.0,
            517_161.0,
        ]);
        metrics.extend([("mean", 80_876.4), ("min", 11_369.0), ("max", 517_161.0)]);
        assert_variant(
            &results,
            "oha",
            "{}",
            &[
                (latency, metrics),
                (
                    TimeToFirstByte::SLUG_STR,
                    vec![("mean", 82_631.9), ("min", 11_318.0), ("max", 517_101.0)],
                ),
                (
                    DnsDialupTime::SLUG_STR,
                    vec![("mean", 77_373.3), ("min", 13_631.0), ("max", 292_158.0)],
                ),
                (
                    DnsLookupTime::SLUG_STR,
                    vec![("mean", 14_961.6), ("min", 1_491.0), ("max", 64_322.0)],
                ),
                (Throughput::SLUG_STR, vec![("mean", 298.886_4)]),
                (SuccessRatio::SLUG_STR, vec![("value", 88.67)]),
                (Timeouts::SLUG_STR, vec![("value", 38.0)]),
                (FailedRequests::SLUG_STR, vec![("value", 68.0)]),
                ("responses-200", vec![("value", 420.0)]),
                ("responses-404", vec![("value", 60.0)]),
                ("responses-500", vec![("value", 52.0)]),
                (Responses2xx::SLUG_STR, vec![("value", 420.0)]),
                (Responses4xx::SLUG_STR, vec![("value", 60.0)]),
                (Responses5xx::SLUG_STR, vec![("value", 52.0)]),
            ],
        );
        assert_variant(
            &results,
            "oha",
            r#"{"@direction":"received"}"#,
            &[
                (BytesThroughput::SLUG_STR, vec![("mean", 627.0)]),
                (SizePerRequest::SLUG_STR, vec![("mean", 2.0)]),
            ],
        );
        assert_variant(
            &results,
            "oha",
            r#"{"@responses":"successful"}"#,
            &[percentiles([
                24_790.0, 32_720.0, 47_995.0, 86_396.0, 192_953.0, 322_213.0, 461_624.0, 517_161.0,
                517_161.0,
            ])],
        );
        assert_variant(
            &results,
            "oha",
            r#"{"@responses":"not_successful"}"#,
            &[percentiles([
                22_626.0, 32_119.0, 41_215.0, 61_757.0, 166_190.0, 334_245.0, 395_079.0, 418_896.0,
                418_896.0,
            ])],
        );
    }

    /// Kills any one of oha's time units read with the wrong factor or left out, `10 min`
    /// included, a binary byte unit read as a decimal one, and a unit scaled in floating point,
    /// which gives `611287.0400000001` for `596.96 KiB`.
    #[test]
    fn adapter_http_oha_text_units() {
        for (suffix, slowest) in [
            ("fast_nanoseconds.txt", 9_179_374.0),
            ("mixed_timeout_breakdown.txt", 517_161.0),
            ("fast.txt", 2_732_400.0),
            ("mixed_seconds.txt", 3_001_500_000.0),
            ("hang_minutes.txt", 10_008_000_000.0),
            ("hang_hours.txt", 10_080_000_000.0),
            ("hang_ten_seconds_1_12_0.txt", 10_012_000_000.0),
            ("hang_ten_minutes_1_12_0.txt", 605_040_000_000.0),
            ("mixed_breakdown_1_8_0.txt", 1_700_000.0),
        ] {
            assert_eq!(
                value(&named(suffix), "oha", "{}", Latency::SLUG_STR, "max"),
                Some(slowest),
                "{suffix}"
            );
        }
        assert_eq!(
            value(
                &named("fast.txt"),
                "oha",
                r#"{"@direction":"received"}"#,
                BytesThroughput::SLUG_STR,
                "mean"
            ),
            Some(611_287.04)
        );
    }

    /// Kills `NaN`, `inf`, or `-inf` written or counted as skipped, a `NaN` size per request that
    /// refuses the run, a status code or class filled with 0, and a variant built with no
    /// metric.
    #[test]
    fn adapter_http_oha_text_all_errors() {
        let results = named("drop.txt");
        assert_eq!(variants(&results), 2);
        assert_variant(
            &results,
            "oha",
            "{}",
            &[
                (Throughput::SLUG_STR, vec![("mean", 74_250.523_6)]),
                (SuccessRatio::SLUG_STR, vec![("value", 0.0)]),
                (Timeouts::SLUG_STR, vec![("value", 0.0)]),
                (FailedRequests::SLUG_STR, vec![("value", 2_000.0)]),
            ],
        );
        assert_variant(
            &results,
            "oha",
            r#"{"@direction":"received"}"#,
            &[(BytesThroughput::SLUG_STR, vec![("mean", 0.0)])],
        );
    }

    /// Kills the first byte's line required, which oha prints only from 1.17.0.
    #[test]
    fn adapter_http_oha_text_before_the_first_byte() {
        let results = named("mixed_breakdown_1_12_0.txt");
        assert_eq!(variants(&results), 4);
        assert_eq!(
            value(&results, "oha", "{}", TimeToFirstByte::SLUG_STR, "mean"),
            None
        );
        assert_value(
            &results,
            ("oha", "{}"),
            (DnsLookupTime::SLUG_STR, "max"),
            344_000.0,
        );
    }

    /// Kills a summary from before 1.9.0 skipped, its `secs` read with another factor, its
    /// histogram labels with no unit refused, a `NaN` one included, and a time printed as
    /// `0.0000 secs` left out rather than stored as printed.
    #[test]
    fn adapter_http_oha_text_reads_seconds_before_1_9() {
        let results = named("mixed_breakdown_1_8_0.txt");
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(variants(&results), 4);
        let (latency, mut metrics) = percentiles([
            0.0,
            0.0,
            0.0,
            100_000.0,
            100_000.0,
            200_000.0,
            400_000.0,
            1_700_000.0,
            1_700_000.0,
        ]);
        metrics.extend([("mean", 100_000.0), ("min", 0.0), ("max", 1_700_000.0)]);
        assert_variant(
            &results,
            "oha",
            "{}",
            &[
                (latency, metrics),
                (
                    DnsDialupTime::SLUG_STR,
                    vec![("mean", 100_000.0), ("min", 0.0), ("max", 1_600_000.0)],
                ),
                (
                    DnsLookupTime::SLUG_STR,
                    vec![("mean", 100_000.0), ("min", 0.0), ("max", 1_400_000.0)],
                ),
                (Throughput::SLUG_STR, vec![("mean", 298.686_6)]),
                (SuccessRatio::SLUG_STR, vec![("value", 88.67)]),
                (Timeouts::SLUG_STR, vec![("value", 34.0)]),
                (FailedRequests::SLUG_STR, vec![("value", 68.0)]),
                ("responses-200", vec![("value", 415.0)]),
                ("responses-404", vec![("value", 59.0)]),
                ("responses-500", vec![("value", 58.0)]),
                (Responses2xx::SLUG_STR, vec![("value", 415.0)]),
                (Responses4xx::SLUG_STR, vec![("value", 59.0)]),
                (Responses5xx::SLUG_STR, vec![("value", 58.0)]),
            ],
        );
        assert_variant(
            &results,
            "oha",
            r#"{"@direction":"received"}"#,
            &[
                (BytesThroughput::SLUG_STR, vec![("mean", 619.0)]),
                (SizePerRequest::SLUG_STR, vec![("mean", 2.0)]),
            ],
        );
        assert_variant(
            &results,
            "oha",
            r#"{"@responses":"successful"}"#,
            &[percentiles([
                0.0,
                0.0,
                0.0,
                100_000.0,
                100_000.0,
                200_000.0,
                400_000.0,
                1_700_000.0,
                1_700_000.0,
            ])],
        );
        assert_variant(
            &results,
            "oha",
            r#"{"@responses":"not_successful"}"#,
            &[percentiles([
                0.0, 0.0, 0.0, 100_000.0, 100_000.0, 300_000.0, 300_000.0, 300_000.0, 300_000.0,
            ])],
        );
        let results = named("drop_1_8_0.txt");
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(variants(&results), 2);
        assert_variant(
            &results,
            "oha",
            "{}",
            &[
                (Throughput::SLUG_STR, vec![("mean", 54_700.365_5)]),
                (SuccessRatio::SLUG_STR, vec![("value", 0.0)]),
                (Timeouts::SLUG_STR, vec![("value", 0.0)]),
                (FailedRequests::SLUG_STR, vec![("value", 2_000.0)]),
            ],
        );
        assert_variant(
            &results,
            "oha",
            r#"{"@direction":"received"}"#,
            &[(BytesThroughput::SLUG_STR, vec![("mean", 0.0)])],
        );
    }

    /// Kills a 1xx code counted in the 3xx class, a 3xx code counted in the 1xx class, and the
    /// two classes swapped.
    #[test]
    fn adapter_http_oha_text_informational_and_redirect_classes() {
        let results = named("codes_1xx_3xx.txt");
        for (measure, expected) in [
            ("responses-101", 10.0),
            ("responses-200", 10.0),
            ("responses-302", 10.0),
            ("responses-304", 10.0),
            (Responses1xx::SLUG_STR, 10.0),
            (Responses2xx::SLUG_STR, 10.0),
            (Responses3xx::SLUG_STR, 20.0),
        ] {
            assert_value(&results, ("oha", "{}"), (measure, "value"), expected);
        }
    }

    /// Kills a status code IANA does not assign dropped or given a built-in, one past 599 put in
    /// a class, and a class that leaves out a code IANA does not assign.
    #[test]
    fn adapter_http_oha_text_status_codes_iana_does_not_assign() {
        let results = named("codes_599_999.txt");
        for (measure, expected) in [
            ("responses-599", 10.0),
            ("responses-999", 10.0),
            (Responses2xx::SLUG_STR, 20.0),
            (Responses5xx::SLUG_STR, 10.0),
        ] {
            assert_value(&results, ("oha", "{}"), (measure, "value"), expected);
        }
        assert_units(
            &results,
            &[
                ("responses-599", "responses"),
                ("responses-999", "responses"),
            ],
        );
    }

    /// Kills a status or error row cut short or garbled that ends its block in silence, so the
    /// error block is not read, or is read only up to that row, and the failed requests and the
    /// timeouts read 0 or less than oha counted. No capture holds such a row, so the test cuts
    /// one in a real capture, as an output cut short or two writers to one output leave it.
    #[test]
    fn adapter_http_oha_text_cut_row_is_skipped_and_counted() {
        let lines = fixture("mixed_timeout_breakdown.txt")
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let row = |start: &str| {
            lines
                .iter()
                .position(|line| line.starts_with(start))
                .expect("Failed to find the row")
        };
        let (status, error) = (row("  [404] "), row("  [30] "));
        for input in [
            [&lines[..status], &["  [404] 6".to_owned()]].concat(),
            [
                &lines[..status],
                &["  [404] 60 respon".to_owned()],
                &lines[status + 1..],
            ]
            .concat(),
            [&lines[..error], &["  [3".to_owned()]].concat(),
        ] {
            let results =
                AdapterHttpOhaText::parse(&input.join("\n"), Settings::new(None, BmfVersion::V1))
                    .expect("Failed to parse the summary");
            assert!(results.nameless.is_none(), "{input:?}");
            assert_eq!(
                results.skipped,
                Skipped {
                    benchmarks: 1,
                    ..Skipped::default()
                },
                "{input:?}"
            );
        }
    }

    /// Kills an error title with no row after it and a title cut short, each read as a run with
    /// no error, so the failed requests and the timeouts read 0. No capture is cut there, so the
    /// test cuts a real capture where an output cut short ends.
    #[test]
    fn adapter_http_oha_text_cut_error_title_is_skipped_and_counted() {
        let summary = fixture("mixed_timeout_breakdown.txt");
        let title = error_title(&summary);
        for end in [
            title + "Error dist".len(),
            title + "Error distribution:\n".len(),
        ] {
            let input = summary.get(..end).expect("Failed to cut the summary");
            let results = AdapterHttpOhaText::parse(input, Settings::new(None, BmfVersion::V1))
                .expect("Failed to parse the summary");
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

    /// Kills a run with no error refused when the user's own lines after the summary start with
    /// a blank line, then the end of the input, a blank line, or another line, as `; echo`,
    /// `; echo; echo`, and `; echo; echo done` print them. A summary cut right before its error
    /// block prints the same lines, so it reads as a run with no error too; the test cuts a real
    /// capture there.
    #[test]
    fn adapter_http_oha_text_user_lines_after_a_run_with_no_error() {
        let summary = fixture("mixed_timeout_breakdown.txt");
        let cut = summary
            .get(..error_title(&summary))
            .expect("Failed to cut the summary")
            .to_owned();
        let inputs = ["\n", "\n\n", "\ndone\n"]
            .map(|after| format!("{}{after}", fixture("fast.txt")))
            .into_iter()
            .chain([cut]);
        for input in inputs {
            let results = AdapterHttpOhaText::parse(&input, Settings::new(None, BmfVersion::V1))
                .expect("Failed to parse the summary");
            assert!(results.nameless.is_some(), "{input}");
            assert_eq!(results.skipped, Skipped::default(), "{input}");
        }
    }

    /// The start of the error title's line in a summary.
    fn error_title(summary: &str) -> usize {
        summary
            .find("\nError distribution:\n")
            .expect("Failed to find the error title")
            + 1
    }

    /// Kills a second summary written over the first, merged into it, or dropped uncounted.
    #[test]
    fn adapter_http_oha_text_second_summary_is_skipped_and_counted() {
        let results = named("concat.txt");
        assert_eq!(results.skipped.benchmarks, 1);
        assert_value(&results, ("oha", "{}"), ("responses-200", "value"), 500.0);
        assert_eq!(value(&results, "oha", "{}", "responses-500", "value"), None);
    }

    /// Kills a summary cut short that hides the summary after it, as a reader does that goes on
    /// past the line where the cut summary failed, and a cut summary read in part or uncounted.
    /// No capture holds a cut summary, so the test joins the first 30 lines of one real capture
    /// to the second summary of another.
    #[test]
    fn adapter_http_oha_text_cut_summary_reads_nothing_of_the_next() {
        let cut = fixture("mixed_timeout_breakdown.txt")
            .lines()
            .take(30)
            .chain(fixture("concat.txt").lines().skip(44))
            .collect::<Vec<_>>()
            .join("\n");
        let mut results = AdapterHttpOhaText::parse(&cut, Settings::new(None, BmfVersion::V1))
            .expect("Failed to parse the cut summary");
        results.name_nameless(None);
        assert_eq!(results.skipped.benchmarks, 1);
        assert_value(
            &results,
            ("oha", "{}"),
            (Latency::SLUG_STR, "max"),
            543_902.0,
        );
        assert_eq!(value(&results, "oha", "{}", "responses-200", "value"), None);
    }

    /// Kills a row reader that passes over a line oha never prints in a block: a histogram row
    /// cut short, as two writers to one output can leave it, one with a bar of another
    /// character (ghz's), one with no unit in a summary that prints units, a time in `secs` in
    /// that summary, and a label or a time with a unit in a summary from before 1.9.0, which
    /// prints every time in `secs` and no unit on a label. No capture holds such a line, so the
    /// test puts each in place of a row of a real capture.
    #[test]
    fn adapter_http_oha_text_refuses_a_foreign_row() {
        for (suffix, index, row) in [
            ("fast.txt", 14, "  0.283 ms [49"),
            ("fast.txt", 14, "  0.283 ms [49] |∎∎∎"),
            ("fast.txt", 14, "  0.283 [49] |"),
            ("fast.txt", 26, "  10.00% in 0.0000 secs"),
            ("mixed_breakdown_1_8_0.txt", 14, "  0.283 ms [49] |"),
            ("mixed_breakdown_1_8_0.txt", 26, "  10.00% in 23.8170 us"),
        ] {
            let mut lines = fixture(suffix)
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            lines[index] = row.to_owned();
            let results =
                AdapterHttpOhaText::parse(&lines.join("\n"), Settings::new(None, BmfVersion::V1))
                    .expect("Failed to parse the summary");
            assert!(results.nameless.is_none(), "{suffix} {row}");
            assert_eq!(results.skipped.benchmarks, 1, "{suffix} {row}");
        }
    }

    /// Kills a variant built for a half of the success breakdown with no response, which oha
    /// prints as `NaN` when every response is 2xx.
    #[test]
    fn adapter_http_oha_text_breakdown_with_no_other_responses() {
        let results = named("fast_breakdown.txt");
        assert_eq!(variants(&results), 3);
        assert_eq!(results.skipped, Skipped::default());
        assert!(
            value(
                &results,
                "oha",
                r#"{"@responses":"successful"}"#,
                Latency::SLUG_STR,
                "median"
            )
            .is_some()
        );
    }
}
