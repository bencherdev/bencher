use std::borrow::Cow;

use bencher_json::project::report::Adapter;

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        http::locust::{Entry, nanos},
        leaf::LeafResults,
        percentile::percentile_name,
    },
    results::adapter_results::AdapterResults,
};

/// Locust's request statistics, `<prefix>_stats.csv`, as Locust 1.1 and later write it.
///
/// Locust writes it with Python's `csv.writer` (`StatsCSV.requests_csv`,
/// `locust/stats.py:1002-1015` and `:1041-1067` at 2.47.0): a header of fixed columns and then
/// Locust's percentile levels, a row per request, and the whole test's `Aggregated` row last. The
/// leaf refuses any record Locust does not write there, so a cut or joined file never reads.
pub struct AdapterHttpLocustCsv;

impl Adaptable for AdapterHttpLocustCsv {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let (header, body) = input.split_once('\n')?;
        let header = header
            .strip_suffix("\r\r")
            .or_else(|| header.strip_suffix('\r'))
            .unwrap_or(header);
        let levels = levels(header)?;
        let mut rows = records(body)?;
        let total = rows.pop()?;
        let mut entries = rows
            .iter()
            .map(|row| entry(row, &levels).filter(|entry| entry.request.is_some()))
            .collect::<Option<Vec<_>>>()?;
        entries.push(entry(&total, &levels).filter(|entry| entry.request.is_none())?);
        let mut results = LeafResults::default();
        for entry in entries {
            entry.insert(&mut results);
        }
        results.finish_with_skips(Adapter::HttpLocustCsv)
    }
}

/// The columns before Locust's percentile levels.
const COLUMNS: [&str; 11] = [
    "Type",
    "Name",
    "Request Count",
    "Failure Count",
    "Median Response Time",
    "Average Response Time",
    "Min Response Time",
    "Max Response Time",
    "Average Content Size",
    "Requests/s",
    "Failures/s",
];
/// The `Type` and `Name` of the whole test's row.
const TOTAL: (&str, &str) = ("", "Aggregated");
/// What Locust writes for each percentile of a row with no requests (`stats.py:1000`).
const NOT_AVAILABLE: &str = "N/A";

/// The metric of each percentile column Locust's header names, `None` for a column the leaf
/// skips, or `None` when the line is not Locust's header. The `50%` column is the histogram's
/// median, which `Median Response Time` holds kept within the exact `min` and `max`, and the
/// `100%` column is the histogram's top, which `max` holds exactly.
fn levels(header: &str) -> Option<Vec<Option<String>>> {
    let mut columns = header.split(',');
    if !COLUMNS.iter().all(|column| columns.next() == Some(*column)) {
        return None;
    }
    columns
        .map(|column| {
            let metric = percentile_name(column.strip_suffix('%')?)?;
            Some((!matches!(metric.as_str(), "min" | "median" | "max")).then_some(metric))
        })
        .collect()
}

/// A row's entry, or `None` when it is not a row Locust writes under the header.
fn entry<'a>(row: &'a [Cow<'a, str>], levels: &[Option<String>]) -> Option<Entry<'a>> {
    let [
        kind,
        name,
        requests,
        failures,
        median,
        mean,
        min,
        max,
        size,
        rate,
        failure_rate,
        percentiles @ ..,
    ] = row
    else {
        return None;
    };
    if percentiles.len() != levels.len() {
        return None;
    }
    let requests = count(requests)?;
    let failures = count(failures)?;
    let times = [median, mean, min, max]
        .map(|time| number(time).and_then(nanos))
        .into_iter()
        .collect::<Option<Vec<_>>>()?;
    let size = number(size)?.parse::<f64>().ok()?;
    let rates = (
        number(rate)?.parse::<f64>().ok()?,
        number(failure_rate)?.parse::<f64>().ok()?,
    );
    // Locust writes `N/A` for every percentile of a row with no requests, and its median, mean,
    // and size then fall back to 0 (`stats.py:428-433`, `:494-499`), which no request measured.
    let sampled = requests > 0.0;
    let mut latency = Vec::new();
    if sampled {
        latency.extend(
            ["median", "mean", "min", "max"]
                .map(Cow::Borrowed)
                .into_iter()
                .zip(times),
        );
    }
    for (level, cell) in levels.iter().zip(percentiles) {
        if !sampled {
            if cell != NOT_AVAILABLE {
                return None;
            }
            continue;
        }
        let value = digits(cell).then(|| nanos(cell)).flatten()?;
        if let Some(metric) = level {
            latency.push((Cow::Owned(metric.clone()), value));
        }
    }
    let request = (kind.as_ref(), name.as_ref());
    Some(Entry {
        request: (request != TOTAL).then_some(request),
        requests,
        failures,
        latency,
        size_per_request: sampled.then_some(size),
        rates: Some(rates),
    })
}

/// A count, which Locust writes as a Python `int`.
fn count(cell: &str) -> Option<f64> {
    digits(cell).then(|| cell.parse().ok()).flatten()
}

/// A number as Python writes an `int` or a finite, non-negative `float`, such as `3`, `0.5`, or
/// `1e-05`.
fn number(cell: &str) -> Option<&str> {
    let (mantissa, exponent) = cell.split_once('e').unwrap_or((cell, "0"));
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, "0"));
    let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
    (digits(whole) && digits(fraction) && digits(exponent)).then_some(cell)
}

/// Whether `cell` is one or more ASCII digits.
fn digits(cell: &str) -> bool {
    !cell.is_empty() && cell.bytes().all(|byte| byte.is_ascii_digit())
}

/// Each record of `input`, its fields as Python's `csv.writer` quotes them, ending in its `\r\n`
/// (`\r\r\n` on Windows), or in `\n`, a last `\r`, or the end of the input as `bencher run`
/// rebuilds the lines it reads, or `None` when a record is not one `csv.writer` writes.
fn records(input: &str) -> Option<Vec<Vec<Cow<'_, str>>>> {
    let mut records = Vec::new();
    let mut rest = input;
    while !rest.is_empty() {
        let mut fields = Vec::new();
        loop {
            let (field, after) = if let Some(quoted) = rest.strip_prefix('"') {
                quoted_field(quoted)?
            } else {
                let end = rest.find([',', '"', '\r', '\n']).unwrap_or(rest.len());
                let (field, after) = rest.split_at_checked(end)?;
                (Cow::Borrowed(field), after)
            };
            fields.push(field);
            if let Some(after) = after.strip_prefix(',') {
                rest = after;
                continue;
            }
            rest = RECORD_ENDS
                .into_iter()
                .find_map(|end| after.strip_prefix(end))
                .or_else(|| matches!(after, "" | "\r").then_some(""))?;
            break;
        }
        records.push(fields);
    }
    Some(records)
}

/// What ends a record before another. Locust opens the file in text mode without `newline=""`
/// (`stats.py:1115-1116` at 2.47.0), so on Windows each `\r\n` is written as `\r\r\n`.
const RECORD_ENDS: [&str; 3] = ["\r\r\n", "\r\n", "\n"];

/// A field after its opening quote, each doubled quote one quote, and the input after its
/// closing quote.
fn quoted_field(input: &str) -> Option<(Cow<'_, str>, &str)> {
    let mut field = String::new();
    let mut rest = input;
    loop {
        let (text, after) = rest.split_once('"')?;
        field.push_str(text);
        if let Some(after) = after.strip_prefix('"') {
            field.push('"');
            rest = after;
        } else {
            return Some((Cow::Owned(field), after));
        }
    }
}

#[cfg(test)]
mod test_http_locust_csv {
    use std::collections::BTreeSet;

    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Latency, Throughput},
                http::{FailedRequests, Requests, SizePerRequest},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::{
            http::locust_csv::AdapterHttpLocustCsv,
            test_util::{Expected, assert_variant, convert_v1, value, variants},
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpLocustCsv;
    const GET: &str = r#"{"@method":"GET","@type":"http"}"#;
    const GET_FAILURES: &str = r#"{"@method":"GET","@responses":"failures","@type":"http"}"#;
    const GET_RECEIVED: &str = r#"{"@direction":"received","@method":"GET","@type":"http"}"#;
    /// Every fixture this leaf reads.
    const FIXTURES: [&str; 12] = [
        "basic_stats.csv",
        "error_stats.csv",
        "hang_stats.csv",
        "large_stats.csv",
        "mixed_stats.csv",
        "multi_stats.csv",
        "percentiles_stats.csv",
        "quoted_stats.csv",
        "scenarios_stats.csv",
        "survey_stats.csv",
        "v1_1_1_multi_stats.csv",
        "zero_stats.csv",
    ];

    fn fixture(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/http/locust/{suffix}"))
            .expect("Failed to read a fixture")
    }

    fn parse(input: &str) -> Option<AdapterResults> {
        AdapterHttpLocustCsv::parse(input, Settings::new(None, BmfVersion::V1))
    }

    fn named(suffix: &str) -> AdapterResults {
        let mut results = convert_v1(LEAF, &format!("http/locust/{suffix}"));
        results.name_nameless(None);
        results
    }

    /// Milliseconds as Locust prints them, in nanoseconds.
    fn ms(millis: f64) -> f64 {
        millis * 1e6
    }

    /// `latency` with its median, mean, min, and max, in milliseconds, then each metric given.
    fn latency(
        median: f64,
        mean: f64,
        min: f64,
        max: f64,
        more: &[(&'static str, f64)],
    ) -> Expected {
        let mut metrics = vec![
            ("median", ms(median)),
            ("mean", ms(mean)),
            ("min", ms(min)),
            ("max", ms(max)),
        ];
        metrics.extend(more.iter().map(|(metric, millis)| (*metric, ms(*millis))));
        (Latency::SLUG_STR, metrics)
    }

    /// The default levels from `p66` to `p99_99`, in milliseconds.
    fn levels(millis: [f64; 9]) -> Vec<(&'static str, f64)> {
        [
            "p66", "p75", "p80", "p90", "p95", "p98", "p99", "p99_9", "p99_99",
        ]
        .into_iter()
        .zip(millis)
        .collect()
    }

    fn count(measure: &'static str, count: f64) -> Expected {
        (measure, vec![("value", count)])
    }

    fn mean(measure: &'static str, mean: f64) -> Expected {
        (measure, vec![("mean", mean)])
    }

    /// The benchmarks and the canonical parameters of each variant.
    fn shape(results: &AdapterResults) -> BTreeSet<(String, String)> {
        results
            .inner
            .iter()
            .flat_map(|(benchmark, variants)| {
                variants
                    .keys()
                    .map(move |parameters| (benchmark.to_string(), parameters.canonical()))
            })
            .collect()
    }

    /// Kills the `Aggregated` row read as a request or left out, a request named with its
    /// `Type`, a GET and a POST to one path merged, the failure rate or the size on the request's
    /// own variant, and a leaf that records another leaf.
    #[test]
    fn adapter_http_locust_csv_names_each_request_and_the_whole_test() {
        let results = convert_v1(LEAF, "http/locust/multi_stats.csv");
        assert_eq!(results.resolved_adapter, Some(LEAF));
        assert_eq!(
            results.nameless.as_ref().map(|nameless| nameless.harness),
            Some("locust")
        );
        assert_eq!(results.skipped, Skipped::default());
        let results = named("multi_stats.csv");
        let post = r#"{"@method":"POST","@type":"http"}"#;
        let post_failures = r#"{"@method":"POST","@responses":"failures","@type":"http"}"#;
        let post_received = r#"{"@direction":"received","@method":"POST","@type":"http"}"#;
        let mut expected = BTreeSet::new();
        for benchmark in ["/fast", "/fast?id=[id]", "/mixed", "/slow"] {
            for parameters in [GET, GET_FAILURES, GET_RECEIVED] {
                expected.insert((benchmark.to_owned(), parameters.to_owned()));
            }
        }
        for parameters in [post, post_failures, post_received] {
            expected.insert(("/post".to_owned(), parameters.to_owned()));
        }
        for parameters in [
            "{}",
            r#"{"@responses":"failures"}"#,
            r#"{"@direction":"received"}"#,
        ] {
            expected.insert(("locust".to_owned(), parameters.to_owned()));
        }
        assert_eq!(shape(&results), expected);
    }

    /// A request may be named `Aggregated`, and only its empty `Type` marks the whole test's
    /// row; the input is a real capture with a request renamed so.
    ///
    /// Kills the whole test's row known by its `Name` alone.
    #[test]
    fn adapter_http_locust_csv_reads_a_request_named_aggregated() {
        let input = fixture("basic_stats.csv").replace("GET,/slow,", "GET,Aggregated,");
        let mut results = parse(&input).unwrap();
        results.name_nameless(None);
        assert_eq!(
            value(&results, "Aggregated", GET, Requests::SLUG_STR, "value"),
            Some(1_682.0)
        );
        assert_eq!(
            value(&results, "locust", "{}", Requests::SLUG_STR, "value"),
            Some(6_014.0)
        );
    }

    /// Kills a percentile column read one column off, the `50%` or `100%` column written, the
    /// median read from `50%`, `Request Count` and `Failure Count` swapped, a time left in
    /// milliseconds, and `Requests/s` and `Failures/s` swapped.
    #[test]
    fn adapter_http_locust_csv_measures() {
        let results = named("multi_stats.csv");
        assert_variant(
            &results,
            "/mixed",
            GET,
            &[
                latency(
                    0.223_643_000_026_640_95,
                    48.819_109_179_998_634,
                    0.223_643_000_026_640_95,
                    1_001.998_523_999_986_9,
                    &levels([1.0, 1.0, 1.0, 1.0, 3.0, 1_000.0, 1_000.0, 1_000.0, 1_000.0]),
                ),
                mean(Throughput::SLUG_STR, 94.453_639_908_013_4),
                count(Requests::SLUG_STR, 850.0),
                count(FailedRequests::SLUG_STR, 260.0),
            ],
        );
        assert_variant(
            &results,
            "/mixed",
            GET_FAILURES,
            &[mean(Throughput::SLUG_STR, 28.891_701_618_921_743)],
        );
        assert_variant(
            &results,
            "/mixed",
            GET_RECEIVED,
            &[mean(SizePerRequest::SLUG_STR, 2.082_352_941_176_470_7)],
        );
        assert_variant(
            &results,
            "locust",
            "{}",
            &[
                latency(
                    1.0,
                    18.470_767_787_557_648,
                    0.222_339_999_993_437_23,
                    1_001.998_523_999_986_9,
                    &levels([1.0, 1.0, 2.0, 51.0, 52.0, 52.0, 54.0, 1_000.0, 1_000.0]),
                ),
                mean(Throughput::SLUG_STR, 525.162_237_888_554_5),
                count(Requests::SLUG_STR, 4_726.0),
                count(FailedRequests::SLUG_STR, 260.0),
            ],
        );
        assert_variant(
            &results,
            "locust",
            r#"{"@responses":"failures"}"#,
            &[mean(Throughput::SLUG_STR, 28.891_701_618_921_743)],
        );
        assert_variant(
            &results,
            "locust",
            r#"{"@direction":"received"}"#,
            &[mean(SizePerRequest::SLUG_STR, 2_980.717_731_696_995_5)],
        );
    }

    /// Kills a millisecond time scaled through a float, which gives 584498.6914440909 ns for the
    /// 0.584498691444091 ms Locust printed.
    #[test]
    fn adapter_http_locust_csv_scales_the_printed_decimal() {
        let results = named("multi_stats.csv");
        let mean = value(&results, "/fast", GET, Latency::SLUG_STR, "mean").unwrap();
        assert_eq!(mean.to_bits(), 584_498.691_444_091f64.to_bits(), "{mean}");
    }

    /// Python writes a float under 1e-4 with a negative exponent, as `Failures/s` is after one
    /// failure in a 3 hour run; the input is a real capture with that rate edited in.
    ///
    /// Kills an exponent read without its `-`.
    #[test]
    fn adapter_http_locust_csv_reads_a_negative_exponent() {
        let input = fixture("basic_stats.csv").replace(",0.0,1,", ",9.259259259259259e-05,1,");
        let mut results = parse(&input).unwrap();
        results.name_nameless(None);
        for (benchmark, parameters) in [
            ("/fast", GET_FAILURES),
            ("locust", r#"{"@responses":"failures"}"#),
        ] {
            assert_eq!(
                value(
                    &results,
                    benchmark,
                    parameters,
                    Throughput::SLUG_STR,
                    "mean"
                ),
                Some(9.259_259_259_259_259e-5),
                "{benchmark}"
            );
        }
    }

    /// Kills `@method` written for a custom client's type, and its `@type` written as `http`.
    #[test]
    fn adapter_http_locust_csv_keeps_a_custom_type() {
        let results = named("scenarios_stats.csv");
        let benchmarks = shape(&results)
            .into_iter()
            .filter(|(benchmark, parameters)| {
                benchmark == "select one" || (benchmark == "pay" && !parameters.contains("@d"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            benchmarks,
            vec![
                (
                    "pay".to_owned(),
                    r#"{"@method":"POST","@responses":"failures","@type":"http"}"#.to_owned()
                ),
                (
                    "pay".to_owned(),
                    r#"{"@method":"POST","@type":"http"}"#.to_owned()
                ),
                (
                    "select one".to_owned(),
                    r#"{"@direction":"received","@type":"SQL"}"#.to_owned()
                ),
                (
                    "select one".to_owned(),
                    r#"{"@responses":"failures","@type":"SQL"}"#.to_owned()
                ),
                ("select one".to_owned(), r#"{"@type":"SQL"}"#.to_owned()),
            ]
        );
    }

    /// Locust keeps a `Type` as the user wrote it, and its HTTP clients send `get` as `GET`; the
    /// input is a real capture with a `GET` written as `get`.
    ///
    /// Kills a verb known in upper case only, and `@method` written as printed.
    #[test]
    fn adapter_http_locust_csv_reads_a_verb_in_any_case() {
        let input = fixture("basic_stats.csv");
        let lower = input.replace("GET,/fast,", "get,/fast,");
        assert_ne!(lower, input);
        assert_eq!(parse(&lower), parse(&input));
    }

    /// Kills the default levels read in place of the header's, and the header's `50%` or `100%`
    /// column written.
    #[test]
    fn adapter_http_locust_csv_reads_the_levels_the_header_names() {
        let results = named("percentiles_stats.csv");
        assert_variant(
            &results,
            "/slow",
            GET,
            &[
                latency(
                    52.0,
                    52.097_927_936_158_25,
                    50.540_947_995_614_26,
                    55.385_016_021_318_734,
                    &[("p25", 51.0), ("p99_95", 55.0)],
                ),
                mean(Throughput::SLUG_STR, 76.099_621_772_348_04),
                count(Requests::SLUG_STR, 304.0),
                count(FailedRequests::SLUG_STR, 0.0),
            ],
        );
    }

    /// A `0%` level prints the histogram's bottom bucket, 0 here, beside the exact
    /// `Min Response Time`.
    ///
    /// Kills the `0%` column written as `min`.
    #[test]
    fn adapter_http_locust_csv_keeps_the_exact_min_beside_a_0_level() {
        let results = named("zero_stats.csv");
        assert_variant(
            &results,
            "/fast",
            GET,
            &[
                latency(
                    1.0,
                    0.952_523_976_317_522_6,
                    0.384_123_064_577_579_5,
                    16.073_046_950_623_393,
                    &[("p90", 1.0)],
                ),
                mean(Throughput::SLUG_STR, 3_192.267_995_886_399),
                count(Requests::SLUG_STR, 12_773.0),
                count(FailedRequests::SLUG_STR, 0.0),
            ],
        );
    }

    /// Kills Locust's placeholder times and size for a row with no requests written as
    /// measurements, its `N/A` percentiles counted or refused, and its counts and rates left out.
    #[test]
    fn adapter_http_locust_csv_writes_no_time_for_no_requests() {
        let results = named("hang_stats.csv");
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(variants(&results), 2);
        assert_variant(
            &results,
            "locust",
            "{}",
            &[
                mean(Throughput::SLUG_STR, 0.0),
                count(Requests::SLUG_STR, 0.0),
                count(FailedRequests::SLUG_STR, 0.0),
            ],
        );
        assert_variant(
            &results,
            "locust",
            r#"{"@responses":"failures"}"#,
            &[mean(Throughput::SLUG_STR, 0.0)],
        );
    }

    /// Kills a quoted field split at its comma, its doubled quote kept, and `HEAD` read as a
    /// custom type.
    #[test]
    fn adapter_http_locust_csv_reads_a_quoted_name() {
        let results = named("quoted_stats.csv");
        assert_eq!(
            shape(&results)
                .into_iter()
                .filter(|(_, parameters)| !parameters.contains("@d") && !parameters.contains("@r"))
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                (r#"/fast?a=1,b="2""#.to_owned(), GET.to_owned()),
                (
                    "/fast".to_owned(),
                    r#"{"@method":"HEAD","@type":"http"}"#.to_owned()
                ),
                ("locust".to_owned(), "{}".to_owned()),
            ])
        );
    }

    /// Locust 1.1 prints a `99.999%` column, and Locust 1.0 writes its own CSV, every field
    /// quoted, every time cut to an integer, and the whole test's `Type` as `None`, which the
    /// leaf refuses.
    ///
    /// Kills a gate on the current default levels, and a `99.999%` column left out.
    #[test]
    fn adapter_http_locust_csv_reads_locust_1_1_on() {
        let results = named("v1_1_1_multi_stats.csv");
        assert_eq!(
            value(&results, "/slow", GET, Latency::SLUG_STR, "p99_999"),
            Some(ms(55.0))
        );
        assert_eq!(
            value(&results, "/slow", GET, Latency::SLUG_STR, "p99_99"),
            Some(ms(55.0))
        );
        assert_eq!(parse(&fixture("v1_0_3_multi_stats.csv")), None);
    }

    /// `bencher run` rebuilds the command's output line by line, which drops each `\r` and the
    /// last line break, while `--file` passes the bytes as Locust wrote them.
    ///
    /// Kills a reader that needs `\r\n` or a final line break.
    #[test]
    fn adapter_http_locust_csv_reads_rebuilt_stdout_the_same() {
        for suffix in FIXTURES {
            let input = fixture(suffix);
            assert!(input.ends_with("\r\n"), "{suffix}");
            assert_eq!(parse(&rebuilt(&input)), parse(&input), "{suffix}");
            assert!(parse(&input).is_some(), "{suffix}");
        }
    }

    /// Locust opens the file in text mode without `newline=""`, so on Windows each `\r\n` that
    /// `csv.writer` ends a record with is written as `\r\r\n`, which `bencher run` rebuilds as
    /// `\r\n` between records and a `\r` at the end. Each input is a real capture with its line
    /// ends edited so.
    ///
    /// Kills a reader that refuses `\r\r\n`, a header that keeps its second `\r`, and a reader
    /// that refuses the rebuilt `\r` at the end.
    #[test]
    fn adapter_http_locust_csv_reads_windows_line_ends() {
        for suffix in FIXTURES {
            let input = fixture(suffix);
            let windows = input.replace("\r\n", "\r\r\n");
            assert_eq!(parse(&windows), parse(&input), "{suffix}");
            assert_eq!(parse(&rebuilt(&windows)), parse(&input), "{suffix}");
        }
    }

    /// `output` as `bencher run` rebuilds it from the command's lines.
    fn rebuilt(output: &str) -> String {
        output
            .lines()
            .skip_while(|line| line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Each edit is of a real capture into a shape Locust never writes: a file cut inside a
    /// row or before its `Aggregated` row, a second file joined after it, a row after the
    /// `Aggregated` row, a second `Aggregated` row, a percentile that is not an integer, a
    /// percentile for a row with no requests, a missing cell, a quote inside a cell, a bare
    /// carriage return inside a cell or between rows, and three before a line feed.
    ///
    /// Kills each of the reader's refusals removed, so a cut or joined file reads partly.
    #[test]
    fn adapter_http_locust_csv_refuses_what_locust_does_not_write() {
        let input = fixture("basic_stats.csv");
        let mut lines = input.split_inclusive("\r\n").collect::<Vec<_>>();
        let total = lines.pop().unwrap();
        let slow = lines.pop().unwrap();
        let fast = lines.pop().unwrap();
        let header = lines.pop().unwrap();
        let cut = total.split(',').take(15).collect::<Vec<_>>().join(",");
        for (edit, edited) in [
            ("cut inside a row", format!("{header}{fast}{slow}{cut}")),
            ("cut before Aggregated", format!("{header}{fast}{slow}")),
            ("two files joined", format!("{input}{input}")),
            (
                "a row after Aggregated",
                format!("{header}{fast}{total}{slow}"),
            ),
            (
                "a second Aggregated row",
                format!("{header}{fast}{slow}{total}{total}"),
            ),
            (
                "a percentile that is not an integer",
                format!(
                    "{header}{}{slow}{total}",
                    fast.replace(",170,170", ",inf,170")
                ),
            ),
            (
                "a percentile for no requests",
                fixture("hang_stats.csv").replacen("N/A", "0", 1),
            ),
            (
                "a missing cell",
                format!("{header}{}{slow}{total}", fast.replace(",170,170", ",170")),
            ),
            (
                "a quote inside a cell",
                format!("{header}{}{slow}{total}", fast.replace("/fast", "/fa\"st")),
            ),
            (
                "a bare carriage return",
                format!("{header}{}{slow}{total}", fast.replace("/fast", "/fa\rst")),
            ),
            (
                "a bare carriage return between rows",
                format!("{header}{}{slow}{total}", fast.replace("\r\n", "\r")),
            ),
            (
                "three carriage returns",
                format!("{header}{}{slow}{total}", fast.replace("\r\n", "\r\r\r\n")),
            ),
        ] {
            assert_eq!(parse(&edited), None, "{edit}");
        }
        assert!(parse(&format!("{header}{fast}{slow}{total}")).is_some());
    }

    /// Locust keys each row on its `Type` and `Name`, so a second row of both, edited into a
    /// real capture, is not Locust's own.
    ///
    /// Kills the repeat written over the first, or dropped uncounted.
    #[test]
    fn adapter_http_locust_csv_skips_a_repeated_request() {
        let input = fixture("basic_stats.csv");
        let mut lines = input.split_inclusive("\r\n").collect::<Vec<_>>();
        let total = lines.pop().unwrap();
        let slow = lines.pop().unwrap();
        let fast = lines.pop().unwrap();
        let header = lines.pop().unwrap();
        let repeat = fast.replace(",4332,", ",1,");
        let mut results = parse(&format!("{header}{fast}{repeat}{slow}{total}")).unwrap();
        results.name_nameless(None);
        assert_eq!(results.skipped.variants, 1);
        assert_eq!(
            value(&results, "/fast", GET, Requests::SLUG_STR, "value"),
            Some(4_332.0)
        );
    }
}
