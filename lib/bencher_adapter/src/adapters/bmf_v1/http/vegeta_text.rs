use std::str::FromStr;

use bencher_json::project::report::Adapter;
use rust_decimal::{Decimal, prelude::ToPrimitive as _};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        http::vegeta::{Directions, Rates, Run},
        leaf::LeafResults,
    },
    results::adapter_results::AdapterResults,
};

/// Vegeta's text report (`lib/reporters.go:55-132` at 12.13.0), the same lines since 12.8.0;
/// an earlier report is not claimed.
///
/// Its rows are fixed, so every line up to `Error Set:` must be the one Vegeta prints there, and
/// a report cut short is skipped and counted. Input with several reports, as `-every` writes, is
/// refused.
pub struct AdapterHttpVegetaText;

impl Adaptable for AdapterHttpVegetaText {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let lines = input.lines().collect::<Vec<_>>();
        let mut reports = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains(REQUESTS));
        let (start, first) = reports.next()?;
        // `-every` clears a terminal before each report but the last (`report.go:136-141`).
        if reports.next().is_some() || !first.starts_with(REQUESTS) {
            return None;
        }
        let mut results = LeafResults::default();
        if let Some(run) = lines.get(start..).and_then(read_run) {
            run.insert(&mut results);
        } else {
            results.skip_benchmark();
        }
        results.finish_with_skips(Adapter::HttpVegetaText)
    }
}

/// The first row's label, padded to the latency label that 12.8.0 widened (`lib/reporters.go:60`).
const REQUESTS: &str = "Requests      [total, rate, throughput]         ";

/// The report from its first row, or `None` when any line is not the one Vegeta prints there.
fn read_run(report: &[&str]) -> Option<Run> {
    let mut lines = report.iter();
    let mut row = |label: &str| lines.next()?.strip_prefix(label);
    let [requests, rate, throughput] = cells(row(REQUESTS)?)?;
    let requests = integer::<u64>(requests)?;
    let rates = Rates {
        all: fixed(rate)?,
        successful: fixed(throughput)?,
    };
    let [total, attack, wait] = cells(row("Duration      [total, attack, wait]             ")?)?;
    for time in [total, attack, wait] {
        duration(time)?;
    }
    let latency = cells::<7>(row("Latencies     [min, mean, 50, 90, 95, 99, max]  ")?)?;
    let latency = ["min", "mean", "median", "p90", "p95", "p99", "max"]
        .into_iter()
        .zip(latency)
        .map(|(metric, time)| Some((metric, duration(time)?)))
        .collect::<Option<Vec<_>>>()?;
    let received = bytes(row("Bytes In      [total, mean]                     ")?)?;
    let sent = bytes(row("Bytes Out     [total, mean]                     ")?)?;
    let success_ratio =
        fixed(row("Success       [ratio]                           ")?.strip_suffix('%')?)?;
    let status_codes = status_codes(row("Status Codes  [code:count]                      ")?)?;
    // Vegeta counts every request under its status code (`lib/metrics.go:53-54`).
    status_codes
        .iter()
        .try_fold(0u64, |sum, (_, count)| sum.checked_add(*count))
        .filter(|sum| *sum == requests)?;
    row("Error Set:")?.is_empty().then_some(())?;
    Some(Run {
        latency,
        rates: (attack != ZERO).then_some(rates),
        requests: float(requests)?,
        success_ratio,
        size_per_request: Directions { received, sent },
        status_codes: status_codes
            .into_iter()
            .map(|(code, count)| Some((code, float(count)?)))
            .collect::<Option<_>>()?,
    })
}

/// How Go prints a duration of 0.
const ZERO: &str = "0s";

/// The `N` cells of a row, each after a comma and a space.
fn cells<const N: usize>(row: &str) -> Option<[&str; N]> {
    row.split(", ").collect::<Vec<_>>().try_into().ok()
}

/// `<total>, <mean>`: the mean bytes per request.
fn bytes(row: &str) -> Option<f64> {
    let [total, mean] = cells(row)?;
    integer::<u64>(total)?;
    fixed(mean)
}

/// `<code>:<count>`, each followed by two spaces.
fn status_codes(mut row: &str) -> Option<Vec<(u16, u64)>> {
    let mut status_codes = Vec::new();
    while !row.is_empty() {
        let (status_code, rest) = row.split_once("  ")?;
        let (code, count) = status_code.split_once(':')?;
        status_codes.push((integer(code)?, integer(count)?));
        row = rest;
    }
    Some(status_codes)
}

/// A number as Go's `%.2f` prints it.
fn fixed(cell: &str) -> Option<f64> {
    let (whole, fraction) = cell.split_once('.')?;
    integer::<u64>(whole)?;
    (fraction.len() == 2 && fraction.bytes().all(|byte| byte.is_ascii_digit())).then_some(())?;
    cell.parse().ok()
}

/// A whole number as Go prints it: digits, with no leading zero.
fn integer<T: FromStr>(cell: &str) -> Option<T> {
    let digits = cell.bytes().all(|byte| byte.is_ascii_digit());
    let leading_zero = cell.len() > 1 && cell.starts_with('0');
    (digits && !leading_zero).then_some(())?;
    cell.parse().ok()
}

/// A time in nanoseconds, as Go's `Duration.String` prints it after Vegeta rounds it to the
/// unit below its largest, so to 3 decimals below a minute (`lib/reporters.go:115-132`).
fn duration(cell: &str) -> Option<f64> {
    let nanos = if cell == ZERO {
        Decimal::ZERO
    } else if let Some(nanos) = cell.strip_suffix("ns") {
        decimal(nanos, 0, 1)?
    } else if let Some(micros) = cell.strip_suffix("µs") {
        decimal(micros, 3, 1_000)?
    } else if let Some(millis) = cell.strip_suffix("ms") {
        decimal(millis, 3, 1_000_000)?
    } else {
        clock(cell.strip_suffix('s')?)?
    };
    nanos.to_f64()
}

/// `<h>h<m>m0`, `<m>m<s>`, or `<s>`, before the final `s`: a time of 1 s or more, which Vegeta
/// rounds to the minute from an hour and to the second from a minute.
fn clock(cell: &str) -> Option<Decimal> {
    let Some((minutes, seconds)) = cell.split_once('m') else {
        return decimal(cell, 3, NANOS_PER_SECOND);
    };
    let minutes = if let Some((hours, minutes)) = minutes.split_once('h') {
        (seconds == "0").then_some(())?;
        u64::from(integer::<u32>(hours)?) * 60 + u64::from(integer::<u32>(minutes)?)
    } else {
        u64::from(integer::<u32>(minutes)?)
    };
    Decimal::from(minutes)
        .checked_mul(Decimal::from(60))?
        .checked_mul(Decimal::from(NANOS_PER_SECOND))?
        .checked_add(decimal(seconds, 0, NANOS_PER_SECOND)?)
}

const NANOS_PER_SECOND: u32 = 1_000_000_000;

/// A number with at most `places` decimals and no trailing zero, as Go prints a time in a unit,
/// times `factor`.
fn decimal(cell: &str, places: usize, factor: u32) -> Option<Decimal> {
    let (whole, fraction) = cell
        .split_once('.')
        .map_or((cell, None), |(whole, fraction)| (whole, Some(fraction)));
    integer::<u64>(whole)?;
    fraction
        .is_none_or(|fraction| {
            (1..=places).contains(&fraction.len())
                && fraction.bytes().all(|byte| byte.is_ascii_digit())
                && !fraction.ends_with('0')
        })
        .then_some(())?;
    cell.parse::<Decimal>()
        .ok()?
        .checked_mul(Decimal::from(factor))
}

/// A count as a metric's value.
fn float(count: u64) -> Option<f64> {
    Decimal::from(count).to_f64()
}

#[cfg(test)]
mod test_http_vegeta_text {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Latency, Throughput},
                http::{
                    FailedRequests, Requests, Responses1xx, Responses2xx, Responses3xx,
                    Responses4xx, Responses5xx, SizePerRequest, SuccessRatio,
                },
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::{
            http::vegeta_text::AdapterHttpVegetaText,
            test_util::{assert_units, assert_value, assert_variant, convert_v1, value, variants},
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpVegetaText;

    fn named(suffix: &str) -> AdapterResults {
        let mut results = convert_v1(LEAF, &format!("http/vegeta/{suffix}"));
        results.name_nameless(None);
        results
    }

    fn fixture(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/http/vegeta/{suffix}"))
            .expect("Failed to read a fixture")
    }

    fn parse(input: &str) -> Option<AdapterResults> {
        AdapterHttpVegetaText::parse(input, Settings::new(None, BmfVersion::V1))
    }

    /// The input skipped and counted as one report, with nothing read.
    fn assert_skipped(input: &str) {
        let results = parse(input).unwrap_or_else(|| panic!("not claimed: {input:?}"));
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

    /// Kills a leaf that names the benchmark itself, one that records another leaf, and a skip
    /// counted for the error set or a row the leaf does not store.
    #[test]
    fn adapter_http_vegeta_text_is_one_nameless_benchmark() {
        for suffix in ["fast.txt", "drop.txt", "mixed.txt"] {
            let results = convert_v1(LEAF, &format!("http/vegeta/{suffix}"));
            assert_eq!(results.resolved_adapter, Some(LEAF), "{suffix}");
            assert!(results.inner.is_empty(), "{suffix}");
            assert_eq!(
                results.nameless.as_ref().map(|nameless| nameless.harness),
                Some("vegeta"),
                "{suffix}"
            );
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
        }
    }

    /// Kills a latency cell read into the wrong metric, the request rate read from the
    /// successful rate, the success ratio scaled again, the requests with no response written as
    /// `responses-0` or left out of `failed-requests`, a status code left out of its class, the
    /// sizes without their direction or swapped, and the successful rate without its parameter.
    #[test]
    fn adapter_http_vegeta_text_measures() {
        let results = named("mixed.txt");
        assert_eq!(variants(&results), 4);
        assert_variant(
            &results,
            "vegeta",
            "{}",
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("min", 60_834.0),
                        ("mean", 56_442_000.0),
                        ("median", 143_701.0),
                        ("p90", 366_847.0),
                        ("p95", 960_142_000.0),
                        ("p99", 1_001_000_000.0),
                        ("max", 1_001_000_000.0),
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 200.23)]),
                (Requests::SLUG_STR, vec![("value", 800.0)]),
                (SuccessRatio::SLUG_STR, vec![("value", 73.25)]),
                (FailedRequests::SLUG_STR, vec![("value", 47.0)]),
                ("responses-200", vec![("value", 586.0)]),
                ("responses-404", vec![("value", 79.0)]),
                ("responses-500", vec![("value", 88.0)]),
                (Responses2xx::SLUG_STR, vec![("value", 586.0)]),
                (Responses4xx::SLUG_STR, vec![("value", 79.0)]),
                (Responses5xx::SLUG_STR, vec![("value", 88.0)]),
            ],
        );
        for (direction, size) in [("received", 2.2), ("sent", 0.0)] {
            assert_variant(
                &results,
                "vegeta",
                &format!(r#"{{"@direction":"{direction}"}}"#),
                &[(SizePerRequest::SLUG_STR, vec![("mean", size)])],
            );
        }
        assert_variant(
            &results,
            "vegeta",
            r#"{"@responses":"successful"}"#,
            &[(Throughput::SLUG_STR, vec![("mean", 118.37)])],
        );
        assert_units(&results, &[]);
    }

    /// Kills any one of Go's time units read with the wrong factor or refused: microseconds,
    /// milliseconds, seconds, minutes, and hours, and nanoseconds refused.
    #[test]
    fn adapter_http_vegeta_text_units() {
        let fast = fixture("fast.txt");
        // Only the attack time, from the first request's start to the last's
        // (`lib/metrics.go:99`), can be under a microsecond, and no capture holds one.
        let nanoseconds = fast.replacen(", 2.999s, ", ", 300ns, ", 1);
        assert_ne!(nanoseconds, fast);
        let mut inputs = vec![("300ns", nanoseconds, 24_348.0, 742_304.0)];
        for (suffix, min, max) in [
            ("fast.txt", 24_348.0, 742_304.0),
            ("error.txt", 32_519.0, 1_358_000.0),
            ("mixed.txt", 60_834.0, 1_001_000_000.0),
            ("minute.txt", 61_000_000_000.0, 61_000_000_000.0),
            ("hour.txt", 3_660_000_000_000.0, 3_660_000_000_000.0),
        ] {
            inputs.push((suffix, fixture(suffix), min, max));
        }
        for (input, report, min, max) in inputs {
            let mut results = parse(&report).unwrap_or_else(|| panic!("not claimed: {input}"));
            results.name_nameless(None);
            for (metric, expected) in [("min", min), ("max", max)] {
                assert_eq!(
                    value(&results, "vegeta", "{}", Latency::SLUG_STR, metric),
                    Some(expected),
                    "{input} {metric}"
                );
            }
        }
    }

    /// Kills no 0 for the requests with no response, which Vegeta prints only once one fails.
    #[test]
    fn adapter_http_vegeta_text_no_failure_is_0() {
        assert_value(
            &named("fast.txt"),
            ("vegeta", "{}"),
            (FailedRequests::SLUG_STR, "value"),
            0.0,
        );
    }

    /// Kills the counts Vegeta prints in place of the rates of an attack whose requests all
    /// started at one instant, an attack time of `0s`, read as rates.
    #[test]
    fn adapter_http_vegeta_text_one_instant_has_no_rate() {
        let results = named("one_request.txt");
        assert_eq!(variants(&results), 3);
        assert_eq!(
            value(&results, "vegeta", "{}", Throughput::SLUG_STR, "mean"),
            None
        );
        assert_value(
            &results,
            ("vegeta", "{}"),
            (Latency::SLUG_STR, "max"),
            815_629.0,
        );
    }

    /// Kills a 1xx code counted in the 3xx class, a 3xx code counted in the 1xx class, and the
    /// two classes swapped.
    #[test]
    fn adapter_http_vegeta_text_informational_and_redirect_classes() {
        let results = named("codes.txt");
        for (measure, expected) in [
            ("responses-101", 10.0),
            ("responses-200", 10.0),
            ("responses-302", 10.0),
            ("responses-304", 10.0),
            (Responses1xx::SLUG_STR, 10.0),
            (Responses2xx::SLUG_STR, 10.0),
            (Responses3xx::SLUG_STR, 20.0),
        ] {
            assert_value(&results, ("vegeta", "{}"), (measure, "value"), expected);
        }
    }

    /// Kills a status code IANA does not assign dropped or given a built-in, and one past 599
    /// put in a class.
    #[test]
    fn adapter_http_vegeta_text_status_codes_iana_does_not_assign() {
        let results = named("unassigned.txt");
        for (measure, expected) in [
            ("responses-599", 10.0),
            ("responses-999", 10.0),
            (Responses2xx::SLUG_STR, 10.0),
            (Responses5xx::SLUG_STR, 10.0),
        ] {
            assert_value(&results, ("vegeta", "{}"), (measure, "value"), expected);
        }
        assert_units(
            &results,
            &[
                ("responses-599", "responses"),
                ("responses-999", "responses"),
            ],
        );
    }

    /// Kills a report cut short read in part, as zeros, or uncounted, wherever the cut falls:
    /// inside a label, a value, or a status code, at the end of a row, or inside `Error Set:`.
    /// A cut inside the first label or its padding leaves no line that is Vegeta's, so nothing is
    /// claimed. No capture is cut short, so the test cuts real captures at every character.
    #[test]
    fn adapter_http_vegeta_text_cut_report_is_skipped_and_counted() {
        for suffix in ["mixed.txt", "minute.txt", "hour.txt"] {
            let report = fixture(suffix);
            let end = report
                .find("Error Set:")
                .expect("Failed to find the error set")
                + 9;
            for (cut, _) in report.char_indices().take_while(|(cut, _)| *cut <= end) {
                let input = report.get(..cut).expect("Failed to cut the report");
                if input.contains("Requests      [total, rate, throughput]         ") {
                    assert_skipped(input);
                } else {
                    assert_eq!(parse(input), None, "{input:?}");
                }
            }
        }
    }

    /// Kills a row reader that passes over a line Vegeta never prints in its place: a label
    /// padded otherwise, a cell too many or too few, a rate or a size with other than 2
    /// decimals, a count with a fraction or a leading zero, a time with a trailing zero, no
    /// unit, more decimals than Vegeta rounds it to, seconds beside hours, or hours without
    /// minutes, a status code with one space after it, status counts that do not sum to the
    /// requests, and a line in place of `Error Set:` or after it. No capture holds such a line,
    /// as two writers to one output can leave it, so the test puts each in place of a row of a
    /// real capture.
    #[test]
    fn adapter_http_vegeta_text_refuses_a_foreign_row() {
        let mixed = fixture("mixed.txt");
        let lines = mixed.lines().collect::<Vec<_>>();
        let label = |index: usize| lines[index].get(..48).expect("Failed to find the label");
        let values = [
            (0, " 800, 200.23, 118.37"),
            (0, "800, 200.23"),
            (0, "800, 200.2, 118.37"),
            (0, "0800, 200.23, 118.37"),
            (1, "4.950s, 3.995s, 954.999ms"),
            (1, "4.9501s, 3.995s, 954.999ms"),
            (1, "1m4.95s, 3.995s, 954.999ms"),
            (1, "1h1m5s, 3.995s, 954.999ms"),
            (1, "1h5s, 3.995s, 954.999ms"),
            (1, "4.95s, 3.995s, 954.9991ms"),
            (1, "4.95s, 3.995s, 954.9991µs"),
            (1, "4.95s, 3.995s, 954.5ns"),
            (1, "4.95s, 3.995s, 954.999"),
            (1, "4.95s, 3.995s, 0954.999ms"),
            (2, "60.834µs, 56.442ms"),
            (3, "1758, 2.2"),
            (3, "1758.00, 2.20"),
            (5, "73.25"),
            (6, "0:47 200:586  404:79  500:88  "),
            (6, "0:47  200:586  404:79  500:89  "),
        ]
        .map(|(index, values)| (index, format!("{}{values}", label(index))));
        let whole = [(7, "Error Set"), (7, "Error Set: 404 Not Found")]
            .map(|(index, row)| (index, row.to_owned()));
        for (index, row) in values.into_iter().chain(whole) {
            let mut edited = lines.clone();
            edited[index] = &row;
            assert_skipped(&edited.join("\n"));
        }
    }

    /// Kills a leaf that requires the report at the start or the end of the input, as a script
    /// that prints a line before and after `vegeta report` leaves it. No capture holds such a
    /// line, so the test adds them to a real capture.
    #[test]
    fn adapter_http_vegeta_text_reads_between_other_lines() {
        let input = format!("attack done\n{}done\n", fixture("fast.txt"));
        let mut results = parse(&input).expect("Failed to parse the report");
        results.name_nameless(None);
        assert_eq!(results.skipped, Skipped::default());
        assert_value(
            &results,
            ("vegeta", "{}"),
            ("responses-200", "value"),
            3_000.0,
        );
    }

    /// Kills a leaf that reads the first or the last of the reports `-every` writes, to a file
    /// or to a terminal, which it clears before each report but the last, and one that reads a
    /// report after those codes, as `-every` leaves the output when it stops after its first
    /// report. No capture stops there, so the test cuts a real capture after its first report.
    #[test]
    fn adapter_http_vegeta_text_every_is_refused() {
        let every = fixture("every.txt");
        let first = every
            .find("Error Set:\n")
            .expect("Failed to find the error set")
            + "Error Set:\n".len();
        for input in [
            fixture("every_output.txt").as_str(),
            every.as_str(),
            every.get(..first).expect("Failed to cut the output"),
        ] {
            assert_eq!(parse(input), None, "{input:?}");
        }
    }
}
