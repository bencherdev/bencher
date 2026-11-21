use bencher_json::{
    MeasureNameId, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Latency, Throughput},
            http::{
                DnsDialupTime, DnsLookupTime, FailedRequests, ReceivingTime, Requests, Responses,
                Responses1xx, Responses2xx, Responses3xx, Responses4xx, Responses5xx, SendingTime,
                SizePerRequest, WaitingTime,
            },
        },
        report::Adapter,
    },
};
use serde_json::{Map, Value};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::Finite,
        leaf::{LeafResults, Variant, parameter_set},
    },
    results::adapter_results::AdapterResults,
};

/// hey's text summary (`requester/print.go:96-125` at v0.1.5), as hey prints it from v0.1.1 on.
///
/// hey names nothing for a run, so a run is one benchmark that the server names by the run's
/// benchmark prefix, else `hey`. A summary starts at a `Summary:` line followed by its `  Total:`
/// line. A summary with any line hey does not print there is skipped and counted, as is a second
/// summary, which would name the same benchmark.
pub struct AdapterHttpHey;

impl Adaptable for AdapterHttpHey {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let lines = input.lines().collect::<Vec<_>>();
        let mut results = LeafResults::default();
        let mut read = false;
        let mut index = 0;
        while let Some(line) = lines.get(index) {
            index += 1;
            if *line != SUMMARY || !lines.get(index).is_some_and(|next| next.starts_with(TOTAL)) {
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
            if read {
                results.skip_benchmark();
            } else {
                run.insert(&mut results);
                read = true;
            }
        }
        results.finish_with_skips(Adapter::HttpHey)
    }
}

const SUMMARY: &str = "Summary:";
const TOTAL: &str = "  Total:\t";
/// The name of hey's one benchmark when the run sets no prefix.
const HEY: &str = "hey";
const MEAN: &str = "mean";
const VALUE: &str = "value";
const ERROR_DISTRIBUTION: &str = "Error distribution:";
/// The percentiles hey prints (`report.go:217-235`), each with its metric.
const PERCENTILES: [(&str, &str); 7] = [
    ("10", "p10"),
    ("25", "p25"),
    ("50", "median"),
    ("75", "p75"),
    ("90", "p90"),
    ("95", "p95"),
    ("99", "p99"),
];

/// The lines of the input, read in order.
struct Cursor<'a> {
    lines: &'a [&'a str],
    index: usize,
}

impl<'a> Cursor<'a> {
    fn next(&mut self) -> Option<&'a str> {
        let line = self.lines.get(self.index).copied()?;
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

    /// The time after the next line's label.
    fn time(&mut self, label: &str) -> Option<Finite> {
        seconds(self.value(label)?)
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

    /// Rows for as long as `row` reads them, leaving the first line it does not.
    fn rows_while<T, F: Fn(&str) -> Option<T>>(&mut self, row: F) -> Vec<T> {
        let mut rows = Vec::new();
        while let Some(value) = self.peek().and_then(&row) {
            rows.push(value);
            self.index += 1;
        }
        rows
    }
}

/// One summary after its `Summary:` line, or `None` when any line is not the one hey prints
/// there, so a summary cut short never reads into what follows.
fn read_run(cursor: &mut Cursor<'_>) -> Option<Run> {
    cursor.time("  Total")?;
    let slowest = cursor.time("  Slowest")?;
    let fastest = cursor.time("  Fastest")?;
    let average = cursor.time("  Average")?;
    let rate = number(cursor.value("  Requests/sec")?)?;
    cursor.line("  ")?;
    // hey prints the sizes only when a response had a body (`print.go:103-105`).
    let size_per_request = if cursor
        .peek()
        .is_some_and(|line| line.starts_with("  Total data:"))
    {
        bytes(cursor.value("  Total data")?)?;
        Some(bytes(cursor.value("  Size/request")?)?)
    } else {
        None
    };
    cursor.line("")?;
    cursor.line("Response time histogram:")?;
    cursor.rows(histogram_row)?;
    cursor.line("")?;
    cursor.line("Latency distribution:")?;
    let rows = cursor.rows(percentile_row)?;
    let sign = rows.first().map(|(sign, _)| *sign);
    if rows.iter().any(|(row_sign, _)| Some(*row_sign) != sign) {
        return None;
    }
    cursor.line("Details (average, fastest, slowest):")?;
    let dns_dialup = triple(cursor.value("  DNS+dialup")?)?;
    let dns_lookup = triple(cursor.value("  DNS-lookup")?)?;
    let sending = triple(cursor.value("  req write")?)?;
    let waiting = triple(cursor.value("  resp wait")?)?;
    let receiving = triple(cursor.value("  resp read")?)?;
    cursor.line("")?;
    cursor.line("Status code distribution:")?;
    let status_codes = cursor.rows_while(status_code);
    let errors = errors(cursor)?;

    // When every request failed, hey prints a `NaN` average and leaves every fastest and slowest
    // at 0 (`report.go:119`, `:167-169`), which is no time any request took.
    let sampled = average.get().is_some();
    let mut latency = stats([average, fastest, slowest], sampled);
    latency.extend(rows.into_iter().filter_map(|(_, percentile)| percentile));
    // Before v0.1.5 hey prints `%` for `%%`, and up to v0.1.4 it prints the latency's fastest and
    // slowest as DNS+dialup's (`print.go:114` at v0.1.4), so only a summary that prints `%%` is
    // sure to give DNS+dialup's own.
    let dialup_spread = sampled && sign == Some(PercentSign::Doubled);
    Some(Run {
        measures: vec![
            (Latency::name_id(), latency),
            (Throughput::name_id(), metrics([(MEAN, rate)])),
            (DnsDialupTime::name_id(), stats(dns_dialup, dialup_spread)),
            (DnsLookupTime::name_id(), stats(dns_lookup, sampled)),
            (SendingTime::name_id(), stats(sending, sampled)),
            (WaitingTime::name_id(), stats(waiting, sampled)),
            (ReceivingTime::name_id(), stats(receiving, sampled)),
        ],
        size_per_request,
        status_codes,
        errors,
    })
}

/// What one hey summary reports, each metric finite.
struct Run {
    measures: Vec<(MeasureNameId, Metrics)>,
    size_per_request: Option<f64>,
    status_codes: Vec<(u16, f64)>,
    /// How many requests failed with each error.
    errors: Vec<f64>,
}

/// Each metric of a measure, by its name.
type Metrics = Vec<(&'static str, f64)>;

impl Run {
    fn insert(self, results: &mut LeafResults) {
        let Self {
            measures,
            size_per_request,
            status_codes,
            errors,
        } = self;
        let mut run = results.nameless_variant(HEY, ParameterSet::default());
        for (measure, metrics) in measures {
            for (metric, value) in metrics {
                run.insert(measure.clone(), metric, value);
            }
        }
        let failed = errors.iter().sum::<f64>();
        let responses = status_codes.iter().map(|(_, count)| count).sum::<f64>();
        run.insert(Requests::name_id(), VALUE, responses + failed);
        run.insert(FailedRequests::name_id(), VALUE, failed);
        insert_status_codes(&mut run, &status_codes);
        if let Some(size) = size_per_request {
            let Some(parameters) = parameter_set(Map::from_iter([(
                "@direction".to_owned(),
                Value::from("received"),
            )])) else {
                results.skip_variant();
                return;
            };
            results
                .nameless_variant(HEY, parameters)
                .insert(SizePerRequest::name_id(), MEAN, size);
        }
    }
}

/// The finite metrics among `metrics`.
fn metrics<const N: usize>(metrics: [(&'static str, Finite); N]) -> Metrics {
    metrics
        .into_iter()
        .filter_map(|(metric, value)| Some((metric, value.get()?)))
        .collect()
}

/// A mean, fastest, and slowest as `mean`, then `min` and `max` only when `spread` holds.
fn stats([mean, min, max]: [Finite; 3], spread: bool) -> Metrics {
    if spread {
        metrics([(MEAN, mean), ("min", min), ("max", max)])
    } else {
        metrics([(MEAN, mean)])
    }
}

/// Each status code's count as `responses-<code>`, built in for a code IANA assigns, else named
/// at run time, and each class's sum as `responses-1xx` to `responses-5xx`, for the classes that
/// hold a code.
fn insert_status_codes(variant: &mut Variant<'_>, status_codes: &[(u16, f64)]) {
    let mut classes: [Option<f64>; 5] = [None; 5];
    for &(code, count) in status_codes {
        if let Some(measure) = Responses::name_id(code)
            .or_else(|| variant.named(&Responses::slug(code), Responses::UNITS_STR))
        {
            variant.insert(measure, VALUE, count);
        }
        if let Some(class) = class(code).and_then(|class| classes.get_mut(class)) {
            *class.get_or_insert(0.0) += count;
        }
    }
    let measures = [
        Responses1xx::name_id(),
        Responses2xx::name_id(),
        Responses3xx::name_id(),
        Responses4xx::name_id(),
        Responses5xx::name_id(),
    ];
    for (measure, count) in measures.into_iter().zip(classes) {
        if let Some(count) = count {
            variant.insert(measure, VALUE, count);
        }
    }
}

/// The index of a status code's class among 1xx to 5xx, if it has one.
fn class(code: u16) -> Option<usize> {
    match code {
        100..=199 => Some(0),
        200..=299 => Some(1),
        300..=399 => Some(2),
        400..=499 => Some(3),
        500..=599 => Some(4),
        _ => None,
    }
}

/// The error rows after the status rows, which hey heads with a blank line and its title only
/// when a request failed (`print.go:123-124`), or `None` when a line that hey does not print
/// there follows. The input may end where hey prints only blank lines.
fn errors(cursor: &mut Cursor<'_>) -> Option<Vec<f64>> {
    match cursor.next() {
        None => return Some(Vec::new()),
        Some("") => {},
        Some(_) => return None,
    }
    match cursor.next() {
        None | Some("") => Some(Vec::new()),
        Some(ERROR_DISTRIBUTION) => {
            let errors = cursor.rows_while(error);
            (!errors.is_empty() && matches!(cursor.next(), None | Some(""))).then_some(errors)
        },
        Some(_) => None,
    }
}

/// How a summary prints the percent sign of its percentile rows: v0.1.5 prints the template's
/// `%%` as written (`report.go:134`), and earlier builds print it as `%`, since they use the
/// summary as the format string.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PercentSign {
    Doubled,
    Single,
}

/// `  <percent>%% in <time> secs`, with the sign the row prints and its metric, or no metric for
/// the `0%% in 0.0000 secs` hey prints for a percentile it never reached (`report.go:228-233`).
fn percentile_row(line: &str) -> Option<(PercentSign, Option<(&'static str, f64)>)> {
    let (percent, rest) = line.strip_prefix("  ")?.split_once('%')?;
    let (sign, time) = if let Some(time) = rest.strip_prefix("% in ") {
        (PercentSign::Doubled, time)
    } else {
        (PercentSign::Single, rest.strip_prefix(" in ")?)
    };
    if percent == "0" {
        return (time == "0.0000 secs").then_some((sign, None));
    }
    let (_, metric) = PERCENTILES
        .iter()
        .find(|(printed, _)| *printed == percent)?;
    Some((sign, seconds(time)?.get().map(|time| (*metric, time))))
}

/// `  <label> [<count>]\t|■■■`, the label a time in seconds with 3 decimals.
fn histogram_row(line: &str) -> Option<()> {
    let (label, rest) = line.strip_prefix("  ")?.split_once(" [")?;
    let (whole, fraction) = label.split_once('.')?;
    let (count, bar) = rest.split_once("]\t|")?;
    (digits(whole)
        && fraction.len() == 3
        && digits(fraction)
        && digits(count)
        && bar.chars().all(|c| c == '■'))
    .then_some(())
}

/// `<time> secs, <time> secs, <time> secs`: the mean, min, and max.
fn triple(cells: &str) -> Option<[Finite; 3]> {
    let mut times = cells.split(", ").map(seconds);
    let triple = [times.next()??, times.next()??, times.next()??];
    times.next().is_none().then_some(triple)
}

/// `  [<code>]\t<count> responses`.
fn status_code(line: &str) -> Option<(u16, f64)> {
    let (code, count) = line.strip_prefix("  [")?.split_once("]\t")?;
    let count = count.strip_suffix(" responses")?;
    (digits(code) && digits(count)).then_some((code.parse().ok()?, count.parse().ok()?))
}

/// `  [<count>]\t<error>`, whose count is all that is read.
fn error(line: &str) -> Option<f64> {
    let (count, _) = line.strip_prefix("  [")?.split_once("]\t")?;
    digits(count).then(|| count.parse().ok()).flatten()
}

/// `<size> bytes`.
fn bytes(cell: &str) -> Option<f64> {
    let size = cell.strip_suffix(" bytes")?;
    digits(size).then(|| size.parse().ok()).flatten()
}

/// A time in nanoseconds from the seconds hey prints with `%4.4f` (`print.go:68-70`), its whole
/// and fractional digits scaled apart, so that the time is exact.
fn seconds(cell: &str) -> Option<Finite> {
    Some(match fixed(cell.strip_suffix(" secs")?)? {
        Fixed::Digits(whole, fraction) => {
            let (whole, fraction) = (whole.parse::<f64>().ok()?, fraction.parse::<f64>().ok()?);
            Finite::from(Some(whole * 1e9 + fraction * 1e5))
        },
        Fixed::NotANumber => Finite::default(),
    })
}

/// A number hey prints with `%4.4f`.
fn number(cell: &str) -> Option<Finite> {
    Some(match fixed(cell)? {
        Fixed::Digits(..) => Finite::from(Some(cell.parse().ok()?)),
        Fixed::NotANumber => Finite::default(),
    })
}

/// A number Go prints with `%4.4f`, or `None` for any other cell.
fn fixed(cell: &str) -> Option<Fixed<'_>> {
    if cell == " NaN" {
        return Some(Fixed::NotANumber);
    }
    let (whole, fraction) = cell.split_once('.')?;
    (digits(whole) && fraction.len() == 4 && digits(fraction))
        .then_some(Fixed::Digits(whole, fraction))
}

/// A number Go prints with `%4.4f`.
enum Fixed<'a> {
    /// Its whole and fractional digits.
    Digits(&'a str, &'a str),
    /// The `NaN` hey prints for no sample, which the width pads to ` NaN`.
    NotANumber,
}

/// Whether `cell` is one or more ASCII digits.
fn digits(cell: &str) -> bool {
    !cell.is_empty() && cell.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod test_http_hey {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Latency, Throughput},
                http::{
                    DnsDialupTime, DnsLookupTime, FailedRequests, ReceivingTime, Requests,
                    Responses1xx, Responses2xx, Responses3xx, Responses4xx, Responses5xx,
                    SendingTime, SizePerRequest, WaitingTime,
                },
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::{
            http::hey::AdapterHttpHey,
            test_util::{
                Expected, assert_ignores_average, assert_units, assert_value, assert_variant,
                convert_v1, value, variants,
            },
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpHey;
    const RECEIVED: &str = r#"{"@direction":"received"}"#;

    fn named(suffix: &str) -> AdapterResults {
        let mut results = convert_v1(LEAF, &format!("http/hey/{suffix}"));
        results.name_nameless(None);
        results
    }

    fn parsed(input: &str) -> AdapterResults {
        let mut results = AdapterHttpHey::parse(input, Settings::new(None, BmfVersion::V1))
            .expect("Failed to parse the summary");
        results.name_nameless(None);
        results
    }

    fn fixture(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/http/hey/{suffix}"))
            .expect("Failed to read a fixture")
    }

    /// `latency` with its mean, min, and max, then each percentile given, from `p10` on.
    fn latency(mean: f64, min: f64, max: f64, percentiles: &[f64]) -> Expected {
        let mut metrics = vec![("mean", mean), ("min", min), ("max", max)];
        metrics.extend(
            ["p10", "p25", "median", "p75", "p90", "p95", "p99"]
                .into_iter()
                .zip(percentiles.iter().copied()),
        );
        (Latency::SLUG_STR, metrics)
    }

    /// A details measure with its mean, min, and max.
    fn detail(measure: &'static str, mean: f64, min: f64, max: f64) -> Expected {
        (measure, vec![("mean", mean), ("min", min), ("max", max)])
    }

    fn count(measure: &'static str, count: f64) -> Expected {
        (measure, vec![("value", count)])
    }

    /// Kills a leaf that names the benchmark itself, one that records another leaf, and a skip
    /// counted for a histogram, a percentile hey never reached, or a `NaN`.
    #[test]
    fn adapter_http_hey_is_one_nameless_benchmark() {
        for suffix in ["fast.txt", "drop.txt", "few.txt", "mixed_timeout_0_1_4.txt"] {
            let results = convert_v1(LEAF, &format!("http/hey/{suffix}"));
            assert_eq!(results.resolved_adapter, Some(LEAF), "{suffix}");
            assert!(results.inner.is_empty(), "{suffix}");
            assert_eq!(
                results.nameless.as_ref().map(|nameless| nameless.harness),
                Some("hey"),
                "{suffix}"
            );
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
        }
    }

    /// Kills an error line left out of the failed requests or the requests, a status left out of
    /// the requests or of its class, a percentile misnamed, and the size per request without its
    /// direction or on the run's own variant.
    #[test]
    fn adapter_http_hey_measures() {
        let results = named("mixed_timeout.txt");
        assert_eq!(variants(&results), 2);
        assert_variant(
            &results,
            "hey",
            "{}",
            &[
                latency(
                    100_000.0,
                    0.0,
                    900_000.0,
                    &[
                        0.0, 0.0, 100_000.0, 100_000.0, 200_000.0, 300_000.0, 700_000.0,
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 124.756_6)]),
                detail(DnsDialupTime::SLUG_STR, 0.0, 0.0, 500_000.0),
                detail(DnsLookupTime::SLUG_STR, 0.0, 0.0, 200_000.0),
                detail(SendingTime::SLUG_STR, 0.0, 0.0, 600_000.0),
                detail(WaitingTime::SLUG_STR, 0.0, 0.0, 400_000.0),
                detail(ReceivingTime::SLUG_STR, 0.0, 0.0, 600_000.0),
                count(Requests::SLUG_STR, 2_000.0),
                count(FailedRequests::SLUG_STR, 96.0),
                count("responses-200", 1_481.0),
                count("responses-404", 216.0),
                count("responses-500", 207.0),
                count(Responses2xx::SLUG_STR, 1_481.0),
                count(Responses4xx::SLUG_STR, 216.0),
                count(Responses5xx::SLUG_STR, 207.0),
            ],
        );
        assert_variant(
            &results,
            "hey",
            RECEIVED,
            &[(SizePerRequest::SLUG_STR, vec![("mean", 2.0)])],
        );
    }

    /// Kills `Slowest` and `Fastest` swapped, a details line's mean, fastest, and slowest read in
    /// another order, `resp wait` and `resp read` swapped, a time scaled by another factor, and a
    /// size per request made up when hey prints no size.
    #[test]
    fn adapter_http_hey_latency_and_details() {
        let results = named("slow.txt");
        assert_eq!(variants(&results), 1);
        assert_variant(
            &results,
            "hey",
            "{}",
            &[
                latency(
                    50_800_000.0,
                    50_200_000.0,
                    52_500_000.0,
                    &[
                        50_500_000.0,
                        50_600_000.0,
                        50_700_000.0,
                        50_900_000.0,
                        51_200_000.0,
                        51_500_000.0,
                        52_500_000.0,
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 196.834_9)]),
                detail(DnsDialupTime::SLUG_STR, 0.0, 0.0, 800_000.0),
                detail(DnsLookupTime::SLUG_STR, 0.0, 0.0, 200_000.0),
                detail(SendingTime::SLUG_STR, 0.0, 0.0, 600_000.0),
                detail(
                    WaitingTime::SLUG_STR,
                    50_700_000.0,
                    50_200_000.0,
                    52_400_000.0,
                ),
                detail(ReceivingTime::SLUG_STR, 100_000.0, 0.0, 1_600_000.0),
                count(Requests::SLUG_STR, 500.0),
                count(FailedRequests::SLUG_STR, 0.0),
                count("responses-200", 500.0),
                count(Responses2xx::SLUG_STR, 500.0),
            ],
        );
    }

    /// Kills a `0%% in 0.0000 secs` row, which hey prints for a percentile it never reached, read
    /// as `min` or any other metric, or refused, the DNS lines swapped, and `req write` read from
    /// another line.
    #[test]
    fn adapter_http_hey_percentiles_not_reached() {
        let results = named("few.txt");
        assert_eq!(results.skipped, Skipped::default());
        assert_variant(
            &results,
            "hey",
            "{}",
            &[
                latency(
                    500_000.0,
                    100_000.0,
                    2_200_000.0,
                    &[100_000.0, 100_000.0, 200_000.0, 2_200_000.0],
                ),
                (Throughput::SLUG_STR, vec![("mean", 1_807.864_5)]),
                detail(DnsDialupTime::SLUG_STR, 200_000.0, 0.0, 1_100_000.0),
                detail(DnsLookupTime::SLUG_STR, 100_000.0, 0.0, 400_000.0),
                detail(SendingTime::SLUG_STR, 100_000.0, 0.0, 200_000.0),
                detail(WaitingTime::SLUG_STR, 100_000.0, 100_000.0, 400_000.0),
                detail(ReceivingTime::SLUG_STR, 0.0, 0.0, 100_000.0),
                count(Requests::SLUG_STR, 5.0),
                count(FailedRequests::SLUG_STR, 0.0),
                count("responses-200", 5.0),
                count(Responses2xx::SLUG_STR, 5.0),
            ],
        );
        let results = named("one.txt");
        assert_eq!(results.skipped, Skipped::default());
        assert_variant(
            &results,
            "hey",
            "{}",
            &[
                latency(600_000.0, 600_000.0, 600_000.0, &[]),
                (Throughput::SLUG_STR, vec![("mean", 1_718.744_6)]),
                detail(DnsDialupTime::SLUG_STR, 300_000.0, 300_000.0, 300_000.0),
                detail(DnsLookupTime::SLUG_STR, 100_000.0, 100_000.0, 100_000.0),
                detail(SendingTime::SLUG_STR, 0.0, 0.0, 0.0),
                detail(WaitingTime::SLUG_STR, 100_000.0, 100_000.0, 100_000.0),
                detail(ReceivingTime::SLUG_STR, 0.0, 0.0, 0.0),
                count(Requests::SLUG_STR, 1.0),
                count(FailedRequests::SLUG_STR, 0.0),
                count("responses-200", 1.0),
                count(Responses2xx::SLUG_STR, 1.0),
            ],
        );
    }

    /// Kills the 0 that hey leaves as every fastest and slowest of a run with no response written
    /// as a time, a `NaN` written or counted as skipped, a status code or class filled with 0, and
    /// a run of errors alone refused.
    #[test]
    fn adapter_http_hey_all_errors() {
        for (suffix, rate, requests) in [
            ("drop.txt", 27_659.838_1, 2_000.0),
            ("refused.txt", 62_578.242_4, 200.0),
            ("drop_0_1_4.txt", 28_145.208_4, 2_000.0),
        ] {
            let results = named(suffix);
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
            assert_eq!(variants(&results), 1, "{suffix}");
            assert_variant(
                &results,
                "hey",
                "{}",
                &[
                    (Throughput::SLUG_STR, vec![("mean", rate)]),
                    count(Requests::SLUG_STR, requests),
                    count(FailedRequests::SLUG_STR, requests),
                ],
            );
        }
    }

    /// Kills a `%` row refused, DNS+dialup's fastest and slowest read from a build that prints
    /// `%`, where up to v0.1.4 they are the latency's, and the other details' fastest and slowest
    /// dropped with them.
    #[test]
    fn adapter_http_hey_before_v0_1_5() {
        let results = named("mixed_timeout_0_1_4.txt");
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(variants(&results), 2);
        assert_variant(
            &results,
            "hey",
            "{}",
            &[
                latency(
                    100_000.0,
                    0.0,
                    2_800_000.0,
                    &[
                        0.0, 0.0, 100_000.0, 100_000.0, 200_000.0, 300_000.0, 900_000.0,
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 110.904_4)]),
                (DnsDialupTime::SLUG_STR, vec![("mean", 0.0)]),
                detail(DnsLookupTime::SLUG_STR, 0.0, 0.0, 600_000.0),
                detail(SendingTime::SLUG_STR, 0.0, 0.0, 1_100_000.0),
                detail(WaitingTime::SLUG_STR, 100_000.0, 0.0, 2_800_000.0),
                detail(ReceivingTime::SLUG_STR, 0.0, 0.0, 600_000.0),
                count(Requests::SLUG_STR, 2_000.0),
                count(FailedRequests::SLUG_STR, 125.0),
                count("responses-200", 1_432.0),
                count("responses-404", 221.0),
                count("responses-500", 222.0),
                count(Responses2xx::SLUG_STR, 1_432.0),
                count(Responses4xx::SLUG_STR, 221.0),
                count(Responses5xx::SLUG_STR, 222.0),
            ],
        );
        let results = named("few_0_1_4.txt");
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(
            value(&results, "hey", "{}", Latency::SLUG_STR, "p75"),
            Some(500_000.0)
        );
        assert_eq!(
            value(&results, "hey", "{}", DnsDialupTime::SLUG_STR, "max"),
            None
        );
    }

    /// Kills a 1xx code counted in the 3xx class, a 3xx code counted in the 1xx class, a status
    /// code IANA does not assign dropped or given a built-in, one past 599 put in a class, and a
    /// class left out when its codes are not ones IANA assigns.
    #[test]
    fn adapter_http_hey_status_codes() {
        let results = named("codes_1xx_3xx.txt");
        for (measure, expected) in [
            ("responses-101", 10.0),
            ("responses-200", 10.0),
            ("responses-302", 10.0),
            ("responses-304", 10.0),
            (Responses1xx::SLUG_STR, 10.0),
            (Responses2xx::SLUG_STR, 10.0),
            (Responses3xx::SLUG_STR, 20.0),
            (Requests::SLUG_STR, 40.0),
        ] {
            assert_value(&results, ("hey", "{}"), (measure, "value"), expected);
        }
        let results = named("redirect.txt");
        assert_eq!(variants(&results), 1);
        assert_value(
            &results,
            ("hey", "{}"),
            (Responses3xx::SLUG_STR, "value"),
            200.0,
        );
        let results = named("codes_599_999.txt");
        for (measure, expected) in [
            ("responses-599", 10.0),
            ("responses-999", 10.0),
            (Responses2xx::SLUG_STR, 20.0),
            (Responses5xx::SLUG_STR, 10.0),
        ] {
            assert_value(&results, ("hey", "{}"), (measure, "value"), expected);
        }
        assert_units(
            &results,
            &[
                ("responses-599", "responses"),
                ("responses-999", "responses"),
            ],
        );
    }

    /// Kills a time scaled in floating point, which reads 0.0041 s as 4100000.0000000005 ns. No
    /// capture this leaf reads holds such a time, so the test puts one in place of a real
    /// capture's slowest.
    #[test]
    fn adapter_http_hey_scales_in_decimal() {
        let input =
            fixture("fast.txt").replace("  Slowest:\t0.0027 secs", "  Slowest:\t0.0041 secs");
        assert_eq!(
            value(&parsed(&input), "hey", "{}", Latency::SLUG_STR, "max"),
            Some(4_100_000.0)
        );
    }

    /// Kills a second summary written over the first, merged into it, or dropped uncounted.
    #[test]
    fn adapter_http_hey_second_summary_is_skipped_and_counted() {
        let results = named("concat.txt");
        assert_eq!(results.skipped.benchmarks, 1);
        assert_value(&results, ("hey", "{}"), ("responses-200", "value"), 500.0);
        assert_eq!(value(&results, "hey", "{}", "responses-500", "value"), None);
    }

    /// Kills a summary cut short that hides the summary after it, as a reader does that goes on
    /// past the line where the cut summary failed, and a cut summary read in part or uncounted.
    /// No capture holds a cut summary, so the test joins the first 30 lines of one real capture,
    /// which end inside its percentiles, to the second summary of another.
    #[test]
    fn adapter_http_hey_cut_summary_reads_nothing_of_the_next() {
        let concat = fixture("concat.txt");
        let second = concat
            .lines()
            .enumerate()
            .filter(|(_, line)| *line == "Summary:")
            .nth(1)
            .map(|(index, _)| index)
            .expect("Failed to find the second summary");
        let cut = fixture("mixed_timeout.txt")
            .lines()
            .take(30)
            .chain(concat.lines().skip(second))
            .collect::<Vec<_>>()
            .join("\n");
        let results = parsed(&cut);
        assert_eq!(results.skipped.benchmarks, 1);
        assert_value(
            &results,
            ("hey", "{}"),
            (Latency::SLUG_STR, "max"),
            1_200_000.0,
        );
        assert_value(&results, ("hey", "{}"), ("responses-500", "value"), 500.0);
        assert_eq!(value(&results, "hey", "{}", "responses-200", "value"), None);
    }

    /// Kills a row reader that passes over a line hey never prints in its block: a histogram row
    /// cut short, with a label of 2 decimals, with ghz's bar, or in oha's form; a percentile hey
    /// never prints, one in oha's form, one with 3 decimals, one whose percent sign differs from
    /// the other rows', and a percentile not reached that prints a time; a time or total data
    /// with no unit; a details line with 2 or 4 times; a status or error row cut short; and an
    /// error title cut short or with no row. No capture holds such a line, so the test puts each in place of a
    /// line of a real capture, as an output cut short or two writers to one output leave it.
    #[test]
    fn adapter_http_hey_refuses_a_foreign_row() {
        for (suffix, start, row) in [
            ("fast.txt", "  0.000 [1673]", "  0.000 [16"),
            ("fast.txt", "  0.000 [1673]", "  0.00 [1673]\t|■■"),
            ("fast.txt", "  0.000 [1673]", "  0.000 [1673]\t|∎∎∎"),
            ("fast.txt", "  0.000 [1673]", "  0.283 ms [1673] |■■"),
            ("fast.txt", "  50%% in", "  33%% in 0.0001 secs"),
            ("fast.txt", "  50%% in", "  50.00% in 46.72 us"),
            ("fast.txt", "  50%% in", "  50%% in 0.000 secs"),
            ("fast.txt", "  50%% in", "  50% in 0.0001 secs"),
            ("few.txt", "  0%% in", "  0%% in 0.0001 secs"),
            ("fast.txt", "  Average:", "  Average:\t0.0002"),
            ("fast.txt", "  Total data:", "  Total data:\t6000"),
            (
                "fast.txt",
                "  DNS-lookup:",
                "  DNS-lookup:\t0.0000 secs, 0.0000 secs",
            ),
            (
                "fast.txt",
                "  DNS-lookup:",
                "  DNS-lookup:\t0.0000 secs, 0.0000 secs, 0.0003 secs, 0.0001 secs",
            ),
            ("fast.txt", "  [200]", "  [200]\t20"),
            ("mixed_timeout.txt", "  [95]", "  [9"),
            ("mixed_timeout.txt", ERROR_TITLE, "Error dist"),
        ] {
            let mut lines = fixture(suffix)
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let index = lines
                .iter()
                .position(|line| line.starts_with(start))
                .expect("Failed to find the line");
            if let Some(line) = lines.get_mut(index) {
                row.clone_into(line);
            }
            assert_refused(&lines.join("\n"), &format!("{suffix} {row}"));
        }
        let summary = fixture("mixed_timeout.txt");
        let title = summary
            .find(ERROR_TITLE)
            .expect("Failed to find the error title");
        let cut = summary
            .get(..title + ERROR_TITLE.len() + 1)
            .expect("Failed to cut the summary");
        assert_refused(cut, cut);
    }

    const ERROR_TITLE: &str = "Error distribution:";

    fn assert_refused(input: &str, case: &str) {
        let results = AdapterHttpHey::parse(input, Settings::new(None, BmfVersion::V1))
            .expect("Failed to parse the summary");
        assert!(results.nameless.is_none(), "{case}");
        assert_eq!(
            results.skipped,
            Skipped {
                benchmarks: 1,
                ..Skipped::default()
            },
            "{case}"
        );
    }

    /// Kills a run refused when the user's own lines follow its summary, as `; echo done` prints
    /// them, or when its trailing blank lines are trimmed in full or in part, and its errors lost
    /// when they are. No capture holds either, so the test adds a line to or trims a real capture.
    #[test]
    fn adapter_http_hey_lines_after_the_summary() {
        for (suffix, failed) in [("fast.txt", 0.0), ("mixed_timeout.txt", 96.0)] {
            let summary = fixture(suffix);
            let trimmed = summary.trim_end();
            for input in [
                format!("{summary}done\n"),
                trimmed.to_owned(),
                format!("{trimmed}\n\n"),
            ] {
                let results = parsed(&input);
                assert_eq!(results.skipped, Skipped::default(), "{input}");
                assert_value(
                    &results,
                    ("hey", "{}"),
                    (FailedRequests::SLUG_STR, "value"),
                    failed,
                );
            }
        }
    }

    /// Kills a capture read one way from a file and another as `bencher run` passes it, with no
    /// blank line at the start and no last line break, as a reader that starts past the first
    /// line does.
    #[test]
    fn adapter_http_hey_reads_bencher_run_output() {
        let mut paths = std::fs::read_dir("./tool_output/http/hey")
            .expect("Failed to read the captures")
            .map(|entry| entry.expect("Failed to read a capture").path())
            .collect::<Vec<_>>();
        paths.sort();
        let settings = Settings::new(None, BmfVersion::V1);
        for path in paths {
            let raw = std::fs::read_to_string(&path).expect("Failed to read a capture");
            let rebuilt = raw
                .lines()
                .skip_while(|line| line.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(
                AdapterHttpHey::parse(&rebuilt, settings),
                AdapterHttpHey::parse(&raw, settings),
                "{}",
                path.display()
            );
        }
    }

    /// Kills a summary in v0.1.0's layout, which no other line of this leaf reads, dropped
    /// uncounted rather than skipped and counted.
    #[test]
    fn adapter_http_hey_v0_1_0_is_skipped_and_counted() {
        for suffix in ["fast_0_1_0.txt", "mixed_timeout_0_1_0.txt"] {
            assert_refused(&fixture(suffix), suffix);
        }
    }

    /// Kills a statistic picked by the run's average setting.
    #[test]
    fn adapter_http_hey_ignores_average() {
        assert_ignores_average(LEAF, "http/hey/mixed_timeout.txt");
    }
}
