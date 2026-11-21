use std::collections::{BTreeMap, btree_map};

use bencher_json::{
    BenchmarkName, MeasureNameId, ParameterSet, Slug,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Latency, Throughput},
            http::{
                ConnectErrors, ConnectingTime, DnsLookupTime, DroppedIterations, FailedIterations,
                FailedRequests, IterationTime, Iterations, RequestTime, Requests, Responses,
                Responses1xx, Responses2xx, Responses3xx, Responses4xx, Responses5xx, Timeouts,
                TlsHandshakingTime,
            },
        },
        report::Adapter,
    },
};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::Map;

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::Finite,
        leaf::{LeafResults, Variant, parameter_set},
    },
    results::adapter_results::AdapterResults,
};

/// Artillery at version 1: the `aggregate` of its `--output` report.
///
/// The report may hold `intermediate` too. The test as a whole has no name, so the server names
/// it by the run's benchmark prefix, else `artillery`, and each endpoint the `metrics-by-endpoint`
/// plugin records is a benchmark of its own.
pub struct AdapterHttpArtillery;

impl Adaptable for AdapterHttpArtillery {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let Report { aggregate } = serde_json::from_str(input).ok()?;
        let mut results = LeafResults::default();
        aggregate.insert(&mut results);
        results.finish(Adapter::HttpArtillery)
    }
}

#[derive(Deserialize)]
struct Report {
    aggregate: Aggregate,
}

/// A packed period of metrics (`lib/core/ssms.ts` at 2.0.34), which Artillery writes in its
/// report with `histograms` as a copy of `summaries` (`lib/cmds/run.ts:320-325`).
#[derive(Deserialize)]
struct Aggregate {
    counters: BTreeMap<String, Finite>,
    #[serde(default)]
    rates: BTreeMap<String, Finite>,
    summaries: BTreeMap<String, Summary>,
}

/// `summarizeHistogram` (`lib/core/ssms.ts:738-752`): each statistic rounded to 0.1, with `p50`
/// a copy of `median` and `count` the number of samples, neither read.
#[derive(Deserialize)]
struct Summary {
    min: Finite,
    max: Finite,
    mean: Finite,
    median: Finite,
    p75: Finite,
    p90: Finite,
    p95: Finite,
    p99: Finite,
    p999: Finite,
}

const ARTILLERY: &str = "artillery";
const VALUE: &str = "value";
const RESPONSES: &str = "@responses";
const ENDPOINT: &str = "plugins.metrics-by-endpoint.";
const NANOS_PER_MILLI: u32 = 1_000_000;
const ERRORS_UNITS: &str = "errors";

impl Aggregate {
    fn insert(self, results: &mut LeafResults) {
        let Self {
            counters,
            rates,
            summaries,
        } = self;
        let mut test = Counts::default();
        let mut endpoints = BTreeMap::<String, Counts>::new();
        let mut flows = Flows::default();
        let mut custom = Vec::new();
        for (name, count) in counters {
            match Counter::of(&name) {
                Counter::Requests => flows.requests = Some(count),
                Counter::Completed => flows.completed = Some(count),
                Counter::Skipped => flows.skipped = Some(count),
                Counter::Failed => flows.failed = Some(count),
                Counter::Code(code) => test.codes.push((code, count)),
                Counter::Error(code) => test.errors.push((code.to_owned(), count)),
                Counter::EndpointCode(endpoint, code) => endpoints
                    .entry(endpoint.to_owned())
                    .or_default()
                    .codes
                    .push((code, count)),
                Counter::EndpointError(endpoint, code) => endpoints
                    .entry(endpoint.to_owned())
                    .or_default()
                    .errors
                    .push((code.to_owned(), count)),
                Counter::Cut => {},
                Counter::Custom => custom.push((name, count)),
            }
        }
        let mut latencies = BTreeMap::new();
        for (name, summary) in summaries {
            if let Some(endpoint) = name.strip_prefix(ENDPOINT_RESPONSE_TIME) {
                latencies.insert(endpoint.to_owned(), summary);
                endpoints.entry(endpoint.to_owned()).or_default();
                continue;
            }
            let (measure, parameters) = match name.as_str() {
                "http.response_time" => (Latency::name_id(), Map::new()),
                "http.dns" => (DnsLookupTime::name_id(), Map::new()),
                "http.tcp" => (ConnectingTime::name_id(), Map::new()),
                "http.tls" => (TlsHandshakingTime::name_id(), Map::new()),
                "http.total" => (RequestTime::name_id(), Map::new()),
                "vusers.session_length" => (IterationTime::name_id(), Map::new()),
                _ => {
                    let Some(class) = name
                        .strip_prefix("http.response_time.")
                        .filter(|class| CLASSES.contains(class))
                    else {
                        results.skip_measure();
                        continue;
                    };
                    (
                        Latency::name_id(),
                        Map::from_iter([(RESPONSES.to_owned(), class.into())]),
                    )
                },
            };
            if let Some(parameters) = parameter_set(parameters) {
                summary.insert(
                    &mut results.nameless_variant(ARTILLERY, parameters),
                    &measure,
                );
            }
        }
        let mut whole = results.nameless_variant(ARTILLERY, ParameterSet::default());
        for (name, rate) in rates {
            if name == "http.request_rate" {
                rate.insert(&mut whole, &Throughput::name_id(), "window_mean", 1.0);
            } else {
                whole.skip_measure();
            }
        }
        // The endpoint plugin counts an endpoint's errors only from 1.3.0, which Artillery 2.0.8
        // first ships (`artillery-plugin-metrics-by-endpoint` 1.3.0, `index.js:88-102`).
        let endpoint_errors = if test.errors.is_empty()
            || endpoints.values().any(|counts| !counts.errors.is_empty())
        {
            ErrorCounts::Counted
        } else {
            ErrorCounts::Uncounted
        };
        flows.insert(&mut whole);
        test.insert(&mut whole, ErrorCounts::Counted);
        for (name, count) in custom {
            insert_custom(&mut whole, &name, count);
        }
        for (endpoint, counts) in endpoints {
            let Ok(name) = endpoint.parse::<BenchmarkName>() else {
                results.skip_benchmark();
                continue;
            };
            let mut variant = results.variant(name, ParameterSet::default());
            if let Some(summary) = latencies.remove(&endpoint) {
                summary.insert(&mut variant, &Latency::name_id());
            }
            counts.insert(&mut variant, endpoint_errors);
        }
    }
}

const ENDPOINT_RESPONSE_TIME: &str = "plugins.metrics-by-endpoint.response_time.";
/// The classes Artillery splits `http.response_time` by (`lib/core/engine_http.ts:1084-1109`).
const CLASSES: [&str; 4] = ["2xx", "3xx", "4xx", "5xx"];

/// The test's counts, or one endpoint's.
#[derive(Default)]
struct Counts {
    codes: Vec<(u16, Finite)>,
    errors: Vec<(String, Finite)>,
}

/// The test's request and flow counts, each 0 when Artillery has not written it, as it writes a
/// counter only once it counts (`lib/core/runner.ts:296-297`, `:486-492`).
#[derive(Default)]
struct Flows {
    requests: Option<Finite>,
    completed: Option<Finite>,
    skipped: Option<Finite>,
    failed: Option<Finite>,
}

impl Flows {
    fn insert(self, variant: &mut Variant<'_>) {
        let Self {
            requests,
            completed,
            skipped,
            failed,
        } = self;
        for (measure, count) in [
            (Requests::name_id(), requests),
            (Iterations::name_id(), completed),
            (DroppedIterations::name_id(), skipped),
            (FailedIterations::name_id(), failed),
        ] {
            count
                .unwrap_or_else(|| Some(0.0).into())
                .insert(variant, &measure, VALUE, 1.0);
        }
    }
}

/// How the leaf reads a counter, by its name.
enum Counter<'a> {
    Requests,
    Completed,
    Skipped,
    Failed,
    Code(u16),
    Error(&'a str),
    EndpointCode(&'a str, u16),
    EndpointError(&'a str, &'a str),
    /// A count that another holds (`http.responses`), a byte total, or one the phases set.
    Cut,
    Custom,
}

impl<'a> Counter<'a> {
    fn of(name: &'a str) -> Self {
        match name {
            "http.requests" => return Self::Requests,
            "vusers.completed" => return Self::Completed,
            "vusers.skipped" => return Self::Skipped,
            "vusers.failed" => return Self::Failed,
            "http.responses" | "http.downloaded_bytes" | "vusers.created" => return Self::Cut,
            _ => {},
        }
        if name.starts_with("vusers.created_by_name.") {
            return Self::Cut;
        }
        if let Some(code) = name
            .strip_prefix("http.codes.")
            .and_then(|code| code.parse().ok())
        {
            return Self::Code(code);
        }
        if let Some(code) = name.strip_prefix("errors.") {
            return Self::Error(code);
        }
        let Some(counter) = name.strip_prefix(ENDPOINT) else {
            return Self::Custom;
        };
        // An endpoint's name may hold dots, but its status code or error code comes last.
        if let Some((endpoint, code)) = counter.rsplit_once(".codes.")
            && let Ok(code) = code.parse()
        {
            return Self::EndpointCode(endpoint, code);
        }
        counter
            .rsplit_once(".errors.")
            .map_or(Self::Custom, |(endpoint, code)| {
                Self::EndpointError(endpoint, code)
            })
    }
}

impl Summary {
    /// Each statistic in milliseconds as `measure`'s metric in nanoseconds, or none when a sample
    /// was not a number, which leaves `min`, `max`, or `mean` null and counts the sample as 0
    /// (`@artilleryio/sketches-js` 2.1.1, `dist/ddsketch/DDSketch.js:51-75`), as `http.tls` does
    /// over plain HTTP.
    fn insert(self, variant: &mut Variant<'_>, measure: &MeasureNameId) {
        let Self {
            min,
            max,
            mean,
            median,
            p75,
            p90,
            p95,
            p99,
            p999,
        } = self;
        if [min, max, mean].iter().any(|value| value.get().is_none()) {
            return;
        }
        for (metric, value) in [
            ("min", min),
            ("max", max),
            ("mean", mean),
            ("median", median),
            ("p75", p75),
            ("p90", p90),
            ("p95", p95),
            ("p99", p99),
            ("p99_9", p999),
        ] {
            value
                .map(|value| scale(value, NANOS_PER_MILLI))
                .insert(variant, measure, metric, 1.0);
        }
    }
}

impl Counts {
    /// The status codes and their classes, and the errors if counted: summed as
    /// `failed-requests`, a timeout and a refused connection also as their own counts, and each
    /// code as a measure named at run time.
    fn insert(self, variant: &mut Variant<'_>, error_counts: ErrorCounts) {
        let Self { codes, errors } = self;
        insert_status_codes(variant, &codes);
        if let ErrorCounts::Uncounted = error_counts {
            return;
        }
        let mut failed = 0.0;
        let mut timeouts = 0.0;
        let mut refused = 0.0;
        let mut measures = BTreeMap::new();
        for (code, count) in errors {
            let Some(count) = count.get() else {
                variant.skip_metric();
                continue;
            };
            failed += count;
            match code.as_str() {
                "ETIMEDOUT" => timeouts += count,
                "ECONNREFUSED" => refused += count,
                _ => {},
            }
            if let Some(slug) = Slug::new(format!("errors-{code}")) {
                if let btree_map::Entry::Vacant(entry) = measures.entry(slug) {
                    entry.insert(count);
                } else {
                    variant.skip_measure();
                }
            }
        }
        variant.insert(FailedRequests::name_id(), VALUE, failed);
        variant.insert(Timeouts::name_id(), VALUE, timeouts);
        variant.insert(ConnectErrors::name_id(), VALUE, refused);
        for (slug, count) in measures {
            if let Some(measure) = variant.named(slug.as_ref(), ERRORS_UNITS) {
                variant.insert(measure, VALUE, count);
            }
        }
    }
}

/// Whether the report counts a benchmark's errors, so that a count absent is 0.
#[derive(Clone, Copy)]
enum ErrorCounts {
    Counted,
    Uncounted,
}

/// Each status code's count as `responses-<code>`, built in for a code IANA assigns, else named
/// at run time, and each class's sum as `responses-1xx` to `responses-5xx`, for the classes that
/// hold a code.
fn insert_status_codes(variant: &mut Variant<'_>, status_codes: &[(u16, Finite)]) {
    let mut classes: [Option<f64>; 5] = [None; 5];
    for &(code, count) in status_codes {
        let Some(count) = count.get() else {
            variant.skip_metric();
            continue;
        };
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

/// A user's own counter, a measure named by the counter, unless its name makes the slug of a
/// measure the leaf writes, which it would overwrite.
fn insert_custom(variant: &mut Variant<'_>, name: &str, count: Finite) {
    let Ok(measure) = name.parse::<MeasureNameId>() else {
        variant.skip_measure();
        return;
    };
    if writes(name) {
        variant.skip_measure();
        return;
    }
    count.insert(variant, &measure, VALUE, 1.0);
}

/// Whether a name makes the slug of a measure the leaf writes: a built-in, a status code's, or
/// an error code's.
fn writes(name: &str) -> bool {
    Slug::new(name).is_some_and(|slug| {
        let slug = slug.as_ref();
        WRITTEN.contains(&slug)
            || slug.starts_with("errors-")
            || slug
                .strip_prefix("responses-")
                .is_some_and(|code| !code.is_empty() && code.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// Every built-in measure the leaf writes but `responses-<code>`.
const WRITTEN: [&str; 19] = [
    Latency::SLUG_STR,
    DnsLookupTime::SLUG_STR,
    ConnectingTime::SLUG_STR,
    TlsHandshakingTime::SLUG_STR,
    RequestTime::SLUG_STR,
    Throughput::SLUG_STR,
    Requests::SLUG_STR,
    FailedRequests::SLUG_STR,
    Timeouts::SLUG_STR,
    ConnectErrors::SLUG_STR,
    Responses1xx::SLUG_STR,
    Responses2xx::SLUG_STR,
    Responses3xx::SLUG_STR,
    Responses4xx::SLUG_STR,
    Responses5xx::SLUG_STR,
    Iterations::SLUG_STR,
    DroppedIterations::SLUG_STR,
    FailedIterations::SLUG_STR,
    IterationTime::SLUG_STR,
];

/// `value` times `times`, in decimal arithmetic on the shortest text that reads back as `value`,
/// so `16.4` ms is `16400000` ns, else in floating point past the decimal range.
fn scale(value: f64, times: u32) -> f64 {
    value
        .to_string()
        .parse::<Decimal>()
        .ok()
        .and_then(|decimal| decimal.checked_mul(Decimal::from(times)))
        .and_then(|decimal| decimal.to_string().parse().ok())
        .unwrap_or_else(|| value * f64::from(times))
}

#[cfg(test)]
mod test_http_artillery {
    use std::collections::BTreeSet;

    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Latency, Throughput},
                http::{
                    ConnectErrors, ConnectingTime, DnsLookupTime, DroppedIterations,
                    FailedIterations, FailedRequests, IterationTime, Iterations, RequestTime,
                    Requests, Responses2xx, Responses3xx, Responses4xx, Responses5xx, Timeouts,
                    TlsHandshakingTime,
                },
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_units, assert_value, assert_variant,
            convert_edited_v1, value,
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpArtillery;
    const TEST: &str = "artillery";

    /// The fixture, with the test's own benchmark named as with no prefix.
    fn read(suffix: &str) -> AdapterResults {
        let path = format!("./tool_output/http/artillery/{suffix}");
        let input = std::fs::read_to_string(&path).expect("Failed to read a fixture");
        named(&input).unwrap_or_else(|| panic!("expected {LEAF} to parse {path}"))
    }

    fn named(input: &str) -> Option<AdapterResults> {
        let mut results = LEAF.convert(input, Settings::new(None, BmfVersion::V1))?;
        results.name_nameless(None);
        Some(results)
    }

    fn edited<F: FnOnce(&mut serde_json::Value)>(suffix: &str, edit: F) -> AdapterResults {
        let mut results = convert_edited_v1(LEAF, &format!("http/artillery/{suffix}"), edit);
        results.name_nameless(None);
        results
    }

    const STATISTICS: [&str; 9] = [
        "min", "max", "mean", "median", "p75", "p90", "p95", "p99", "p99_9",
    ];

    /// A summary's statistics in milliseconds, in `STATISTICS`' order, as nanoseconds.
    fn summary(measure: &'static str, millis: [f64; 9]) -> Expected {
        (
            measure,
            STATISTICS
                .iter()
                .copied()
                .zip(millis.map(|millis| millis * 1e6))
                .collect(),
        )
    }

    fn count(measure: &'static str, value: f64) -> Expected {
        (measure, vec![("value", value)])
    }

    /// Kills: `http.response_time` under another measure or missing a statistic, `p50` or
    /// `count` read, `p999` named other than `p99_9`, the rate as `mean` or as a total, a flow
    /// count or a fixed count missing or read from the wrong counter, `http.responses` or
    /// `vusers.created` read, `ETIMEDOUT` or `ECONNREFUSED` left out of `timeouts` or
    /// `connect-errors`, `connect-errors` left out when absent, an error left out of
    /// `failed-requests` or of its own measure, a class summed from another class's codes, and an
    /// endpoint with no error of its own left without its 0 error counts beside endpoints with
    /// errors.
    #[test]
    fn adapter_http_artillery_mixed() {
        let results = read("mixed.json");
        assert_eq!(results.skipped, Skipped::default());
        assert_variant(
            &results,
            TEST,
            "{}",
            &[
                summary(
                    Latency::SLUG_STR,
                    [0.0, 1.0, 0.2, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                ),
                (Throughput::SLUG_STR, vec![("window_mean", 57.0)]),
                count(Requests::SLUG_STR, 443.0),
                count("responses-200", 270.0),
                count("responses-201", 43.0),
                count("responses-404", 17.0),
                count("responses-500", 58.0),
                count(Responses2xx::SLUG_STR, 313.0),
                count(Responses4xx::SLUG_STR, 17.0),
                count(Responses5xx::SLUG_STR, 58.0),
                count(FailedRequests::SLUG_STR, 55.0),
                count(Timeouts::SLUG_STR, 5.0),
                count(ConnectErrors::SLUG_STR, 0.0),
                count("errors-econnreset", 50.0),
                count("errors-etimedout", 5.0),
                count(Iterations::SLUG_STR, 145.0),
                count(DroppedIterations::SLUG_STR, 0.0),
                count(FailedIterations::SLUG_STR, 55.0),
                summary(
                    IterationTime::SLUG_STR,
                    [2.1, 16.4, 3.5, 2.8, 3.2, 3.9, 4.1, 15.6, 15.6],
                ),
            ],
        );
        assert_variant(
            &results,
            "/mixed",
            "{}",
            &[
                summary(
                    Latency::SLUG_STR,
                    [0.0, 1.0, 0.2, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                ),
                count("responses-200", 113.0),
                count("responses-404", 17.0),
                count("responses-500", 15.0),
                count(Responses2xx::SLUG_STR, 113.0),
                count(Responses4xx::SLUG_STR, 17.0),
                count(Responses5xx::SLUG_STR, 15.0),
                count(FailedRequests::SLUG_STR, 12.0),
                count(Timeouts::SLUG_STR, 5.0),
                count(ConnectErrors::SLUG_STR, 0.0),
                count("errors-econnreset", 7.0),
                count("errors-etimedout", 5.0),
            ],
        );
        assert_variant(
            &results,
            "/drop",
            "{}",
            &[
                count(FailedRequests::SLUG_STR, 43.0),
                count(Timeouts::SLUG_STR, 0.0),
                count(ConnectErrors::SLUG_STR, 0.0),
                count("errors-econnreset", 43.0),
            ],
        );
        for endpoint in ["/error", "/fast", "/post"] {
            for measure in [
                FailedRequests::SLUG_STR,
                Timeouts::SLUG_STR,
                ConnectErrors::SLUG_STR,
            ] {
                assert_value(&results, (endpoint, "{}"), (measure, "value"), 0.0);
            }
        }
        assert_eq!(
            results
                .inner
                .keys()
                .map(ToString::to_string)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(
                ["/drop", "/error", "/fast", "/mixed", "/post", TEST].map(str::to_owned)
            )
        );
        assert_units(
            &results,
            &[
                ("errors-econnreset", "errors"),
                ("errors-etimedout", "errors"),
            ],
        );
    }

    /// Kills: a class's response time read onto the plain variant or under another key, a
    /// class Artillery never splits by read as one, and 3xx's codes counted in another class.
    #[test]
    fn adapter_http_artillery_class_response_times() {
        let results = read("codes.json");
        for (class, millis) in [
            ("2xx", [0.0, 1.0, 0.3, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0]),
            ("3xx", [0.0, 1.0, 0.6, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]),
            ("4xx", [0.0, 1.0, 0.3, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0]),
            ("5xx", [0.0, 1.0, 0.3, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0]),
        ] {
            assert_variant(
                &results,
                TEST,
                &format!(r#"{{"@responses":"{class}"}}"#),
                &[summary(Latency::SLUG_STR, millis)],
            );
        }
        let custom = edited("codes.json", |json| {
            let summaries = &mut json["aggregate"]["summaries"];
            summaries["http.response_time.timeout"] = summaries["http.response_time"].clone();
        });
        // A user's own histogram that Artillery's class names do not hold.
        assert_eq!(custom.skipped.measures, 1);
        assert_eq!(
            value(
                &custom,
                TEST,
                r#"{"@responses":"timeout"}"#,
                Latency::SLUG_STR,
                "mean"
            ),
            None
        );
    }

    /// Kills: an endpoint split at the first `.codes.` or `.errors.` rather than the last, an
    /// error counter whose endpoint holds `.codes.` read as no endpoint's, a code IANA does not
    /// assign skipped or made a built-in, and 3xx, 4xx, or 5xx summed from another class's codes.
    #[test]
    fn adapter_http_artillery_codes_and_dotted_endpoints() {
        let results = read("codes.json");
        assert_eq!(results.skipped, Skipped::default());
        for (endpoint, code, class) in [
            (
                "/files/a.codes.200",
                "responses-404",
                Responses4xx::SLUG_STR,
            ),
            (
                "/fast (a.errors.b)",
                "responses-200",
                Responses2xx::SLUG_STR,
            ),
            ("/redirect", "responses-301", Responses3xx::SLUG_STR),
            ("/teapot", "responses-418", Responses4xx::SLUG_STR),
            ("/nonstandard", "responses-599", Responses5xx::SLUG_STR),
        ] {
            assert_value(&results, (endpoint, "{}"), (code, "value"), 20.0);
            assert_value(&results, (endpoint, "{}"), (class, "value"), 20.0);
        }
        assert_variant(
            &results,
            "/hang (x.codes.1.errors.2)",
            "{}",
            &[
                count(FailedRequests::SLUG_STR, 20.0),
                count(Timeouts::SLUG_STR, 20.0),
                count(ConnectErrors::SLUG_STR, 0.0),
                count("errors-etimedout", 20.0),
            ],
        );
        for (measure, expected) in [
            (Responses2xx::SLUG_STR, 20.0),
            (Responses3xx::SLUG_STR, 20.0),
            (Responses4xx::SLUG_STR, 40.0),
            (Responses5xx::SLUG_STR, 20.0),
            ("responses-599", 20.0),
            (Iterations::SLUG_STR, 0.0),
            (FailedIterations::SLUG_STR, 20.0),
        ] {
            assert_value(&results, (TEST, "{}"), (measure, "value"), expected);
        }
        assert_eq!(results.inner.len(), 7);
        assert_units(
            &results,
            &[
                ("errors-etimedout", "errors"),
                ("responses-599", "responses"),
            ],
        );
    }

    /// Kills: `ECONNREFUSED` left out of `connect-errors`, and an iteration count absent because
    /// every flow failed stored as nothing rather than 0.
    #[test]
    fn adapter_http_artillery_refused() {
        let results = read("refused.json");
        assert_eq!(results.skipped, Skipped::default());
        for benchmark in [TEST, "/fast"] {
            assert_value(
                &results,
                (benchmark, "{}"),
                (ConnectErrors::SLUG_STR, "value"),
                10.0,
            );
            assert_value(
                &results,
                (benchmark, "{}"),
                (FailedRequests::SLUG_STR, "value"),
                10.0,
            );
        }
        assert_value(&results, (TEST, "{}"), (Iterations::SLUG_STR, "value"), 0.0);
        assert_eq!(value(&results, TEST, "{}", Latency::SLUG_STR, "mean"), None);
    }

    /// Kills: `vusers.skipped` left unread or read as another count.
    #[test]
    fn adapter_http_artillery_skipped_flows() {
        let results = read("skipped.json");
        for (measure, expected) in [
            (DroppedIterations::SLUG_STR, 16.0),
            (Iterations::SLUG_STR, 8.0),
            (FailedIterations::SLUG_STR, 0.0),
            (Requests::SLUG_STR, 8.0),
        ] {
            assert_value(&results, (TEST, "{}"), (measure, "value"), expected);
        }
    }

    /// A millisecond value scaled in floating point reads 16399999.999999998 ns for 16.4 ms.
    ///
    /// Kills: scaling by a floating point product.
    #[test]
    fn adapter_http_artillery_scales_in_decimal() {
        let results = read("mixed.json");
        for (metric, expected) in [("max", 16_400_000.0), ("p95", 4_100_000.0)] {
            assert_eq!(
                value(&results, TEST, "{}", IterationTime::SLUG_STR, metric),
                Some(expected),
                "{metric}"
            );
        }
    }

    /// A plain HTTP run records no TLS phase, so `http.tls` holds null statistics.
    ///
    /// Kills: `http.dns`, `http.tcp`, or `http.total` left unread or read under another measure,
    /// a summary with null statistics written in part, written as 0, or counted as skipped, and
    /// an endpoint's error counts left out when the run has no error at all.
    #[test]
    fn adapter_http_artillery_extended_metrics() {
        let results = read("extended.json");
        assert_eq!(results.skipped, Skipped::default());
        assert_variant(
            &results,
            TEST,
            "{}",
            &[
                summary(
                    Latency::SLUG_STR,
                    [0.0, 2.0, 0.4, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0],
                ),
                summary(
                    DnsLookupTime::SLUG_STR,
                    [0.0, 1.0, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                ),
                summary(
                    ConnectingTime::SLUG_STR,
                    [0.0, 1.0, 0.3, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0],
                ),
                summary(
                    RequestTime::SLUG_STR,
                    [1.0, 6.0, 2.9, 2.0, 4.0, 5.0, 5.0, 5.0, 5.0],
                ),
                (Throughput::SLUG_STR, vec![("window_mean", 20.0)]),
                count(Requests::SLUG_STR, 20.0),
                count("responses-200", 20.0),
                count(Responses2xx::SLUG_STR, 20.0),
                count(FailedRequests::SLUG_STR, 0.0),
                count(Timeouts::SLUG_STR, 0.0),
                count(ConnectErrors::SLUG_STR, 0.0),
                count(Iterations::SLUG_STR, 20.0),
                count(DroppedIterations::SLUG_STR, 0.0),
                count(FailedIterations::SLUG_STR, 0.0),
                summary(
                    IterationTime::SLUG_STR,
                    [2.7, 15.9, 8.5, 4.4, 13.1, 14.7, 14.7, 14.7, 14.7],
                ),
            ],
        );
        assert_variant(
            &results,
            "/fast",
            "{}",
            &[
                summary(
                    Latency::SLUG_STR,
                    [0.0, 2.0, 0.4, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0],
                ),
                count("responses-200", 20.0),
                count(Responses2xx::SLUG_STR, 20.0),
                count(FailedRequests::SLUG_STR, 0.0),
                count(Timeouts::SLUG_STR, 0.0),
                count(ConnectErrors::SLUG_STR, 0.0),
            ],
        );
        assert_units(&results, &[]);
    }

    /// Kills: `http.tls` left unread or read under another measure.
    #[test]
    fn adapter_http_artillery_extended_metrics_tls() {
        let results = read("extended_tls.json");
        assert_eq!(results.skipped, Skipped::default());
        for (measure, millis) in [
            (
                TlsHandshakingTime::SLUG_STR,
                [0.0, 3.0, 0.9, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            ),
            (DnsLookupTime::SLUG_STR, [0.0; 9]),
            (
                ConnectingTime::SLUG_STR,
                [0.0, 2.0, 0.4, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            ),
            (
                RequestTime::SLUG_STR,
                [42.0, 48.0, 44.9, 44.3, 47.0, 47.0, 47.9, 47.9, 47.9],
            ),
        ] {
            for (statistic, expected) in summary(measure, millis).1 {
                assert_value(&results, (TEST, "{}"), (measure, statistic), expected);
            }
        }
    }

    /// One flow over both HTTP and HTTPS gives `http.tls` a sample that is not a number for the
    /// plain request, which leaves only its `mean` null.
    ///
    /// Kills: a summary read when only its `mean` is null.
    #[test]
    fn adapter_http_artillery_null_mean() {
        let results = read("extended_both.json");
        assert_eq!(results.skipped, Skipped::default());
        for statistic in STATISTICS {
            assert_eq!(
                value(
                    &results,
                    TEST,
                    "{}",
                    TlsHandshakingTime::SLUG_STR,
                    statistic
                ),
                None,
                "{statistic}"
            );
        }
        assert_value(
            &results,
            (TEST, "{}"),
            (ConnectingTime::SLUG_STR, "max"),
            1_053_000_000.0,
        );
    }

    /// Kills: a user's counter left unread, read in a unit, or written over a measure the leaf
    /// writes (`latency`); a user's histogram, a user's rate, or a Server-Timing summary read or
    /// skipped uncounted; and an error message's code left out of `failed-requests` or its own
    /// measure.
    #[test]
    fn adapter_http_artillery_custom_metrics() {
        let results = read("custom.json");
        assert_eq!(
            results.skipped,
            Skipped {
                measures: 4,
                ..Skipped::default()
            }
        );
        for (measure, expected) in [
            ("orders.placed", 40.0),
            (FailedRequests::SLUG_STR, 20.0),
            ("errors-failed-capture-or-match", 20.0),
        ] {
            assert_value(&results, (TEST, "{}"), (measure, "value"), expected);
        }
        assert_value(
            &results,
            (TEST, "{}"),
            (Latency::SLUG_STR, "mean"),
            500_000.0,
        );
        assert_eq!(
            value(&results, TEST, "{}", Latency::SLUG_STR, "value"),
            None
        );
        assert_eq!(
            value(&results, "/timing", "{}", Latency::SLUG_STR, "mean"),
            Some(700_000.0)
        );
        assert_units(&results, &[("errors-failed-capture-or-match", "errors")]);
    }

    /// No capture holds two error codes that make one slug, or a counter or endpoint past a
    /// name's limit, so these edit the codes capture: `errors.etimedout` beside Artillery's
    /// `ETIMEDOUT`, a user's counter named like an error code's measure, one named like a status
    /// code's, one named like each extended summary's measure, and one past the measure name
    /// limit, and an endpoint past the benchmark name limit.
    ///
    /// Kills: a second code with the same slug written over the first or skipped uncounted, a
    /// user's counter written over an error code's measure, a status code's, or an extended
    /// summary's, and a name past a limit skipped uncounted.
    #[test]
    fn adapter_http_artillery_name_clashes_and_limits() {
        let long = format!("/{}", "a".repeat(1024));
        let extended = [
            DnsLookupTime::SLUG_STR,
            ConnectingTime::SLUG_STR,
            TlsHandshakingTime::SLUG_STR,
            RequestTime::SLUG_STR,
        ];
        let results = edited("codes.json", |json| {
            let counters = &mut json["aggregate"]["counters"];
            counters["errors.etimedout"] = 3.into();
            counters["errors-etimedout"] = 4.into();
            counters["responses-200"] = 8.into();
            counters["c".repeat(65).as_str()] = 5.into();
            counters[format!("plugins.metrics-by-endpoint.{long}.codes.200").as_str()] = 6.into();
            for measure in extended {
                counters[measure] = 7.into();
            }
        });
        assert_eq!(
            results.skipped,
            Skipped {
                benchmarks: 1,
                measures: 8,
                ..Skipped::default()
            }
        );
        for measure in extended {
            assert_eq!(value(&results, TEST, "{}", measure, "value"), None);
        }
        assert_value(&results, (TEST, "{}"), ("errors-etimedout", "value"), 20.0);
        assert_value(&results, (TEST, "{}"), ("responses-200", "value"), 20.0);
        assert_value(
            &results,
            (TEST, "{}"),
            (FailedRequests::SLUG_STR, "value"),
            23.0,
        );
    }

    /// A flow of only `think` sends no request, and a phase Artillery skips runs no flow, so its
    /// counters, rates, and summaries are empty.
    ///
    /// Kills: the request count left out when Artillery sent none, and a report with no event
    /// refused or read with a count other than 0.
    #[test]
    fn adapter_http_artillery_no_requests() {
        let zeros = [
            Requests::SLUG_STR,
            FailedRequests::SLUG_STR,
            Timeouts::SLUG_STR,
            ConnectErrors::SLUG_STR,
            DroppedIterations::SLUG_STR,
            FailedIterations::SLUG_STR,
        ]
        .map(|measure| count(measure, 0.0));
        let think = read("think.json");
        let mut expected = zeros.to_vec();
        expected.push(count(Iterations::SLUG_STR, 10.0));
        expected.push(summary(
            IterationTime::SLUG_STR,
            [0.5, 2.6, 1.4, 1.6, 1.6, 2.0, 2.0, 2.0, 2.0],
        ));
        assert_variant(&think, TEST, "{}", &expected);
        assert_eq!(think.inner.len(), 1);
        let idle = read("idle.json");
        let mut expected = zeros.to_vec();
        expected.push(count(Iterations::SLUG_STR, 0.0));
        assert_variant(&idle, TEST, "{}", &expected);
        assert_eq!(idle.inner.len(), 1);
    }

    /// 2.0.1's endpoint plugin counts no endpoint's errors, though the test as a whole has some.
    ///
    /// Kills: requiring the class summaries, which 2.0.1 does not write, and an endpoint's error
    /// counts written as 0 when no endpoint's errors are counted.
    #[test]
    fn adapter_http_artillery_2_0_1() {
        let results = read("mixed_2_0_1.json");
        assert_eq!(results.skipped, Skipped::default());
        assert_variant(
            &results,
            "/post",
            "{}",
            &[
                summary(
                    Latency::SLUG_STR,
                    [0.0, 2.0, 0.6, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
                ),
                count("responses-201", 60.0),
                count(Responses2xx::SLUG_STR, 60.0),
            ],
        );
        for (measure, expected) in [
            (FailedRequests::SLUG_STR, 67.0),
            (Timeouts::SLUG_STR, 2.0),
            (ConnectErrors::SLUG_STR, 0.0),
            ("errors-econnreset", 65.0),
            (Iterations::SLUG_STR, 133.0),
        ] {
            assert_value(&results, (TEST, "{}"), (measure, "value"), expected);
        }
    }

    /// `jq '{aggregate}'` keeps only the aggregate of a long run.
    ///
    /// Kills: requiring `intermediate`, and a request's name dropped from its endpoint.
    #[test]
    fn adapter_http_artillery_aggregate_only() {
        let results = read("large_aggregate.json");
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(results.inner.len(), 101);
        assert_value(
            &results,
            ("/fast?s=1&i=6 (s01-item06)", "{}"),
            (Latency::SLUG_STR, "p99_9"),
            3_000_000.0,
        );
    }

    /// Kills: reading the first of 2 reports one after the other, and reading Artillery 1.x's
    /// report, whose `aggregate` has `counters` but no `summaries`.
    #[test]
    fn adapter_http_artillery_refuses() {
        let mixed = std::fs::read_to_string("./tool_output/http/artillery/mixed.json")
            .expect("Failed to read a fixture");
        assert!(named(&mixed).is_some());
        assert_eq!(named(&format!("{mixed}\n{mixed}")), None);
        let old = std::fs::read_to_string("./tool_output/http/artillery/basic_1_7_9.json")
            .expect("Failed to read a fixture");
        assert_eq!(named(&old), None);
    }

    /// Kills: the test's own benchmark named after another harness.
    #[test]
    fn adapter_http_artillery_ignores_average() {
        assert_ignores_average(LEAF, "http/artillery/basic.json");
        assert!(
            read("basic.json")
                .inner
                .contains_key(&bencher_json::BenchmarkNameId::new_name(
                    TEST.parse().expect("a name")
                ))
        );
    }
}
