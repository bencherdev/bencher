use std::collections::BTreeMap;

use bencher_json::{
    BenchmarkName, MeasureNameId,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{BytesThroughput, Latency, Throughput},
            http::{
                CheckFailures, FailedRequests, Requests, Responses, Responses1xx, Responses2xx,
                Responses3xx, Responses4xx, Responses5xx, Timeouts,
            },
        },
        report::Adapter,
    },
};
use rust_decimal::{Decimal, prelude::ToPrimitive as _};
use serde::Deserialize;
use serde_json::{Map, Number, Value};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::Finite,
        leaf::{LeafResults, Variant, parameter_set},
        percentile::percentile_name,
    },
    results::adapter_results::AdapterResults,
};

/// autocannon at version 1: its `--json` result, one benchmark named by its title, else by its
/// URLs, with the settings it prints as parameters.
pub struct AdapterHttpAutocannon;

impl Adaptable for AdapterHttpAutocannon {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let report = serde_json::from_str::<Report>(input)
            .ok()
            .or_else(|| main_run(input))?;
        let mut results = LeafResults::default();
        report.insert(&mut results);
        results.finish_with_skips(Adapter::HttpAutocannon)
    }
}

/// A `-W` run's main result, printed after the warm-up's own result and with that result nested
/// in it (`lib/init.js:39-60`).
fn main_run(input: &str) -> Option<Report> {
    let mut documents = serde_json::Deserializer::from_str(input).into_iter::<Value>();
    let warmup = documents.next()?.ok()?;
    let report = serde_json::from_str::<Report>(input.get(documents.byte_offset()..)?).ok()?;
    (report.warmup.as_ref() == Some(&warmup)).then_some(report)
}

/// The keys the leaf reads (`lib/aggregateResult.js:44-79` at 8.0.0).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    /// `-T`: text, a number the CLI's parser reads as one, or `true` for a `-T` with no value.
    title: Option<Value>,
    url: Urls,
    connections: Number,
    pipelining: Number,
    /// Absent before 7.0.0, which had no workers.
    workers: Option<Number>,
    /// Absent before 7.7.0, whose windows were always 1 s.
    sample_int: Option<Number>,
    errors: Finite,
    timeouts: Finite,
    mismatches: Finite,
    #[serde(rename = "1xx")]
    informational: Finite,
    #[serde(rename = "2xx")]
    successful: Finite,
    #[serde(rename = "3xx")]
    redirection: Finite,
    #[serde(rename = "4xx")]
    client_error: Finite,
    #[serde(rename = "5xx")]
    server_error: Finite,
    /// Absent before 7.1.0.
    #[serde(default)]
    status_code_stats: BTreeMap<String, Count>,
    latency: Latencies,
    requests: RequestWindows,
    throughput: Histogram,
    /// `-W`'s result, never read as a run.
    warmup: Option<Value>,
}

/// One URL, or several, whose run is one combined result (`lib/run.js:52-55`).
#[derive(Deserialize)]
#[serde(untagged)]
enum Urls {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
struct Count {
    count: Finite,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Latencies {
    /// Absent before 6.0.0.
    total_count: Option<Finite>,
    #[serde(flatten)]
    histogram: Histogram,
}

/// The responses per window, with every request sent.
#[derive(Deserialize)]
struct RequestWindows {
    sent: Finite,
    #[serde(flatten)]
    histogram: Histogram,
}

/// A histogram's statistics (`hdr-histogram-percentiles-obj` 3.0.0), and its percentiles keyed
/// `p` and the percent with `_` for its point. `average` is a copy of `mean`.
#[derive(Deserialize)]
struct Histogram {
    mean: Finite,
    stddev: Finite,
    min: Finite,
    max: Finite,
    #[serde(flatten)]
    rest: BTreeMap<String, Finite>,
}

const VALUE: &str = "value";
const DIRECTION: &str = "@direction";
const NANOS_PER_MILLI: u32 = 1_000_000;
const MILLIS_PER_SECOND: u32 = 1_000;
/// The window before 7.7.0 (`lib/run.js:172` at 6.5.0).
const SAMPLE_INT: u32 = 1_000;

impl Report {
    fn insert(self, results: &mut LeafResults) {
        let Ok(name) = self.name().parse::<BenchmarkName>() else {
            results.skip_benchmark();
            return;
        };
        let parameters = self.parameters();
        let per = self.window().filter(|per| *per > Decimal::ZERO);
        if let Some(mut run) = variant(results, &name, parameters.clone()) {
            if self.latency.has_samples() {
                self.latency.histogram.insert(
                    &mut run,
                    &Latency::name_id(),
                    "",
                    (NANOS_PER_MILLI, Decimal::ONE),
                );
            }
            if let Some(per) = per {
                self.requests.histogram.insert(
                    &mut run,
                    &Throughput::name_id(),
                    "window_",
                    (MILLIS_PER_SECOND, per),
                );
            } else {
                run.skip_measure();
            }
            for (measure, count) in [
                (Requests::name_id(), self.requests.sent),
                (FailedRequests::name_id(), self.errors),
                (Timeouts::name_id(), self.timeouts),
                (CheckFailures::name_id(), self.mismatches),
                (Responses1xx::name_id(), self.informational),
                (Responses2xx::name_id(), self.successful),
                (Responses3xx::name_id(), self.redirection),
                (Responses4xx::name_id(), self.client_error),
                (Responses5xx::name_id(), self.server_error),
            ] {
                count.insert(&mut run, &measure, VALUE, 1.0);
            }
            insert_status_codes(&mut run, &self.status_code_stats);
        }
        let mut parameters = parameters;
        parameters.insert(DIRECTION.to_owned(), "received".into());
        if let Some(mut received) = variant(results, &name, parameters) {
            if let Some(per) = per {
                self.throughput.insert(
                    &mut received,
                    &BytesThroughput::name_id(),
                    "window_",
                    (MILLIS_PER_SECOND, per),
                );
            } else {
                received.skip_measure();
            }
        }
    }

    /// The title when it is text or a number, else the URLs joined by a space.
    fn name(&self) -> String {
        let title = match &self.title {
            Some(Value::String(title)) => Some(title.clone()),
            Some(Value::Number(title)) => Some(title.to_string()),
            _ => None,
        };
        title
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| match &self.url {
                Urls::One(url) => url.clone(),
                Urls::Many(urls) => urls.join(" "),
            })
    }

    fn parameters(&self) -> Map<String, Value> {
        Map::from_iter([
            ("@connections".to_owned(), self.connections.clone().into()),
            ("@pipelining".to_owned(), self.pipelining.clone().into()),
            (
                "@workers".to_owned(),
                self.workers.clone().unwrap_or_else(|| 0.into()).into(),
            ),
            (
                "@sampleInt".to_owned(),
                self.sample_int
                    .clone()
                    .unwrap_or_else(|| SAMPLE_INT.into())
                    .into(),
            ),
        ])
    }

    /// The window in milliseconds.
    fn window(&self) -> Option<Decimal> {
        self.sample_int
            .as_ref()
            .map_or(Some(Decimal::from(SAMPLE_INT)), |sample_int| {
                sample_int.to_string().parse().ok()
            })
    }
}

/// The benchmark's variant of `parameters`, or `None`, counted as skipped, past the key cap.
fn variant<'a>(
    results: &'a mut LeafResults,
    name: &BenchmarkName,
    parameters: Map<String, Value>,
) -> Option<Variant<'a>> {
    let Some(parameters) = parameter_set(parameters) else {
        results.skip_variant();
        return None;
    };
    Some(results.variant(name.clone(), parameters))
}

impl Latencies {
    /// Whether a response's time was recorded, as an empty histogram prints every statistic as
    /// 0. Before 6.0.0 there is no sample count, but every recorded time has a fraction of a
    /// millisecond (`lib/httpClient.js:102` at 4.6.0), so the max is not 0.
    fn has_samples(&self) -> bool {
        self.total_count
            .unwrap_or(self.histogram.max)
            .get()
            .is_some_and(|value| value > 0.0)
    }
}

impl Histogram {
    /// Each statistic as `measure`'s metric with `prefix` in front, times `times` over `per`.
    fn insert(
        &self,
        variant: &mut Variant<'_>,
        measure: &MeasureNameId,
        prefix: &str,
        (times, per): (u32, Decimal),
    ) {
        let statistics = [
            ("mean".to_owned(), self.mean),
            ("std_dev".to_owned(), self.stddev),
            ("min".to_owned(), self.min),
            ("max".to_owned(), self.max),
        ]
        .into_iter()
        .chain(self.rest.iter().filter_map(|(key, value)| {
            let metric = key
                .strip_prefix('p')
                .and_then(|percent| percentile_name(&percent.replace('_', ".")))?;
            Some((metric, *value))
        }));
        for (metric, value) in statistics {
            Finite::from(value.get().and_then(|value| scale(value, times, per))).insert(
                variant,
                measure,
                &format!("{prefix}{metric}"),
                1.0,
            );
        }
    }
}

/// `value` times `times` over `per`, in decimal arithmetic on the shortest text that reads back
/// as `value`, so `0.01` ms is `10000` ns, else in floating point past the decimal range.
fn scale(value: f64, times: u32, per: Decimal) -> Option<f64> {
    value
        .to_string()
        .parse::<Decimal>()
        .ok()
        .and_then(|decimal| decimal.checked_mul(Decimal::from(times)))
        .and_then(|decimal| decimal.checked_div(per))
        .and_then(|decimal| decimal.to_f64())
        .or_else(|| Some(value * f64::from(times) / per.to_f64()?))
}

/// Each status code's count as `responses-<code>`, built in for a code IANA assigns, else named
/// at run time.
fn insert_status_codes(variant: &mut Variant<'_>, status_codes: &BTreeMap<String, Count>) {
    for (code, Count { count }) in status_codes {
        let Ok(code) = code.parse::<u16>() else {
            variant.skip_measure();
            continue;
        };
        if let Some(measure) = Responses::name_id(code)
            .or_else(|| variant.named(&Responses::slug(code), Responses::UNITS_STR))
        {
            count.insert(variant, &measure, VALUE, 1.0);
        }
    }
}

#[cfg(test)]
mod test_http_autocannon {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{BytesThroughput, Latency, Throughput},
                http::{
                    CheckFailures, FailedRequests, Requests, Responses1xx, Responses2xx,
                    Responses3xx, Responses4xx, Responses5xx, Timeouts,
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
            convert_edited_v1, convert_v1, value, variants,
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpAutocannon;
    const MIXED: &str = "http://localhost:8080/mixed";
    const FAST: &str = "http://localhost:8080/fast";
    const CODES: &str = "http://localhost:18182/";
    const DEFAULTS: &str = r#"{"@connections":10,"@pipelining":1,"@sampleInt":1000,"@workers":0}"#;
    const RECEIVED: &str = r#"{"@connections":10,"@direction":"received","@pipelining":1,"@sampleInt":1000,"@workers":0}"#;
    const ONE: &str = r#"{"@connections":1,"@pipelining":1,"@sampleInt":1000,"@workers":0}"#;

    fn read(suffix: &str) -> AdapterResults {
        convert_v1(LEAF, &format!("http/autocannon/{suffix}"))
    }

    const LATENCY: [&str; 19] = [
        "mean", "std_dev", "min", "max", "p0_001", "p0_01", "p0_1", "p1", "p2_5", "p10", "p25",
        "median", "p75", "p90", "p97_5", "p99", "p99_9", "p99_99", "p99_999",
    ];
    const WINDOWS: [&str; 19] = [
        "window_mean",
        "window_std_dev",
        "window_min",
        "window_max",
        "window_p0_001",
        "window_p0_01",
        "window_p0_1",
        "window_p1",
        "window_p2_5",
        "window_p10",
        "window_p25",
        "window_median",
        "window_p75",
        "window_p90",
        "window_p97_5",
        "window_p99",
        "window_p99_9",
        "window_p99_99",
        "window_p99_999",
    ];

    /// Every metric of a histogram: the mean, the standard deviation, the min, the max, then
    /// the 15 percentiles.
    fn histogram(
        measure: &'static str,
        metrics: &[&'static str; 19],
        values: [f64; 19],
    ) -> Expected {
        (measure, metrics.iter().copied().zip(values).collect())
    }

    fn count(measure: &'static str, value: f64) -> Expected {
        (measure, vec![("value", value)])
    }

    /// Kills a time left in milliseconds, `p50` not read as `median`, `average`, `totalCount`,
    /// `total`, or `non2xx` written, a window statistic written without `window_`, the bytes
    /// without their direction or in the run's variant, `requests` read from `total`, a class or a
    /// code dropped or put in another class, and `mismatches` or `timeouts` dropped.
    #[test]
    fn adapter_http_autocannon_measures() {
        let results = read("mixed_timeout.json");
        assert_eq!(results.resolved_adapter, Some(LEAF));
        assert!(results.nameless.is_none());
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(variants(&results), 2);
        assert_variant(
            &results,
            MIXED,
            DEFAULTS,
            &[
                histogram(
                    Latency::SLUG_STR,
                    &LATENCY,
                    [
                        100_000.0, 450_000.0, 1e6, 4e6, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                        0.0, 0.0, 1e6, 3e6, 4e6, 4e6, 4e6,
                    ],
                ),
                histogram(
                    Throughput::SLUG_STR,
                    &WINDOWS,
                    [
                        156.6, 35.15, 105.0, 201.0, 105.0, 105.0, 105.0, 105.0, 105.0, 105.0,
                        128.0, 168.0, 181.0, 201.0, 201.0, 201.0, 201.0, 201.0, 201.0,
                    ],
                ),
                count(Requests::SLUG_STR, 884.0),
                count(FailedRequests::SLUG_STR, 40.0),
                count(Timeouts::SLUG_STR, 40.0),
                count(CheckFailures::SLUG_STR, 0.0),
                count(Responses1xx::SLUG_STR, 0.0),
                count(Responses2xx::SLUG_STR, 595.0),
                count(Responses3xx::SLUG_STR, 0.0),
                count(Responses4xx::SLUG_STR, 98.0),
                count(Responses5xx::SLUG_STR, 90.0),
                count("responses-200", 595.0),
                count("responses-404", 98.0),
                count("responses-500", 90.0),
            ],
        );
        assert_variant(
            &results,
            MIXED,
            RECEIVED,
            &[histogram(
                BytesThroughput::SLUG_STR,
                &WINDOWS,
                [
                    17_462.41, 3_971.79, 11_579.0, 22_357.0, 11_583.0, 11_583.0, 11_583.0,
                    11_583.0, 11_583.0, 11_583.0, 14_255.0, 18_799.0, 20_335.0, 22_367.0, 22_367.0,
                    22_367.0, 22_367.0, 22_367.0, 22_367.0,
                ],
            )],
        );
    }

    /// Kills `errors` and `timeouts` swapped, and every request sent read from the responses.
    #[test]
    fn adapter_http_autocannon_errors_are_failed_requests() {
        let results = read("refused.json");
        for (measure, expected) in [
            (FailedRequests::SLUG_STR, 118_138.0),
            (Timeouts::SLUG_STR, 0.0),
            (Requests::SLUG_STR, 118_148.0),
        ] {
            assert_value(
                &results,
                ("http://localhost:9/none", DEFAULTS),
                (measure, "value"),
                expected,
            );
        }
    }

    /// Kills `mismatches` dropped or written to another measure.
    #[test]
    fn adapter_http_autocannon_mismatches_are_check_failures() {
        let results = read("expect_body.json");
        assert_value(
            &results,
            ("http://localhost:18184/", ONE),
            (CheckFailures::SLUG_STR, "value"),
            20.0,
        );
    }

    /// Kills the 1xx and 3xx classes swapped or summed from the codes, a code IANA does not
    /// assign dropped or given a built-in, and a code outside every class put in one.
    #[test]
    fn adapter_http_autocannon_status_codes() {
        let results = read("codes_1xx_3xx.json");
        for (measure, expected) in [
            (Responses1xx::SLUG_STR, 8.0),
            (Responses2xx::SLUG_STR, 8.0),
            (Responses3xx::SLUG_STR, 16.0),
            ("responses-101", 8.0),
            ("responses-302", 8.0),
            ("responses-304", 8.0),
        ] {
            assert_value(
                &results,
                ("http://localhost:18183/", ONE),
                (measure, "value"),
                expected,
            );
        }
        let results = read("codes_599_999.json");
        for (measure, expected) in [
            (Responses5xx::SLUG_STR, 10.0),
            ("responses-599", 10.0),
            ("responses-999", 10.0),
            ("responses-200", 20.0),
        ] {
            assert_value(&results, (CODES, ONE), (measure, "value"), expected);
        }
        assert_units(
            &results,
            &[
                ("responses-599", "responses"),
                ("responses-999", "responses"),
            ],
        );
    }

    /// Kills the title not read, a URL array refused or joined without a space, a title the
    /// CLI read as a number refused, a `-T` with no value read as the name `true`, and an empty
    /// `--title=` read as a name.
    #[test]
    fn adapter_http_autocannon_names() {
        for (suffix, name, parameters) in [
            ("title.json", "checkout", DEFAULTS),
            (
                "multi_url.json",
                "http://localhost:8080/fast http://localhost:8080/slow",
                DEFAULTS,
            ),
            (
                "multi_url_9_0_0.json",
                "http://localhost:8080/fast http://localhost:8080/slow",
                DEFAULTS,
            ),
            ("fast.json", FAST, DEFAULTS),
            ("title_number.json", "123", ONE),
            ("title_bare.json", CODES, ONE),
            ("title_empty.json", CODES, ONE),
        ] {
            let results = read(suffix);
            assert_eq!(results.inner.len(), 1, "{suffix}");
            assert!(
                value(&results, name, parameters, Requests::SLUG_STR, "value").is_some(),
                "{suffix}"
            );
        }
    }

    /// Kills a name past the benchmark name limit read, or skipped uncounted.
    #[test]
    fn adapter_http_autocannon_long_name_is_skipped_and_counted() {
        let results = read("long_urls.json");
        assert!(results.inner.is_empty());
        assert_eq!(
            results.skipped,
            Skipped {
                benchmarks: 1,
                ..Skipped::default()
            }
        );
    }

    /// Kills a printed setting dropped or misnamed, the measured duration written, and the
    /// settings an older run does not print left out: no workers before 7.0.0, and 1 s windows
    /// before 7.7.0.
    #[test]
    fn adapter_http_autocannon_parameters() {
        for (suffix, name, parameters) in [
            (
                "workers.json",
                FAST,
                r#"{"@connections":10,"@pipelining":1,"@sampleInt":1000,"@workers":2}"#,
            ),
            (
                "pipelining.json",
                FAST,
                r#"{"@connections":10,"@pipelining":10,"@sampleInt":1000,"@workers":0}"#,
            ),
            (
                "sample_int_250.json",
                FAST,
                r#"{"@connections":10,"@pipelining":1,"@sampleInt":250,"@workers":0}"#,
            ),
            ("old_6_5_0_mixed.json", MIXED, DEFAULTS),
            ("old_4_6_0_mixed.json", MIXED, DEFAULTS),
        ] {
            let results = read(suffix);
            assert_eq!(variants(&results), 2, "{suffix}");
            assert!(
                value(&results, name, parameters, Requests::SLUG_STR, "value").is_some(),
                "{suffix}"
            );
        }
    }

    /// Kills a window count left per window, scaled by the window in seconds, or split across
    /// workers, whose counts autocannon sums for each window, the bytes left unscaled, and a
    /// window other than 1 s before 7.7.0.
    #[test]
    fn adapter_http_autocannon_windows_are_per_second() {
        assert_value(
            &read("old_6_5_0_mixed.json"),
            (MIXED, DEFAULTS),
            (Throughput::SLUG_STR, "window_mean"),
            75.67,
        );
        assert_value(
            &read("workers_sample_int_250.json"),
            (
                CODES,
                r#"{"@connections":10,"@pipelining":1,"@sampleInt":250,"@workers":2}"#,
            ),
            (Throughput::SLUG_STR, "window_mean"),
            243.36,
        );
        let results = read("sample_int_250.json");
        let parameters = r#"{"@connections":10,"@pipelining":1,"@sampleInt":250,"@workers":0}"#;
        let received = r#"{"@connections":10,"@direction":"received","@pipelining":1,"@sampleInt":250,"@workers":0}"#;
        for (parameters, measure, metric, expected) in [
            (parameters, Throughput::SLUG_STR, "window_mean", 109_056.0),
            (parameters, Throughput::SLUG_STR, "window_min", 87_692.0),
            (parameters, Throughput::SLUG_STR, "window_p10", 97_404.0),
            (
                received,
                BytesThroughput::SLUG_STR,
                "window_mean",
                12_977_493.36,
            ),
            (
                received,
                BytesThroughput::SLUG_STR,
                "window_std_dev",
                994_911.2,
            ),
        ] {
            assert_value(&results, (FAST, parameters), (measure, metric), expected);
        }
    }

    /// Kills windows of 0 ms divided by, or skipped uncounted.
    #[test]
    fn adapter_http_autocannon_zero_window_is_skipped_and_counted() {
        let results = read("sample_int_0.json");
        assert_eq!(
            results.skipped,
            Skipped {
                measures: 2,
                ..Skipped::default()
            }
        );
        let parameters = r#"{"@connections":1,"@pipelining":1,"@sampleInt":0,"@workers":0}"#;
        assert_eq!(
            value(
                &results,
                CODES,
                parameters,
                Throughput::SLUG_STR,
                "window_mean"
            ),
            None
        );
        assert_value(
            &results,
            (CODES, parameters),
            (Latency::SLUG_STR, "mean"),
            39_600_000.0,
        );
    }

    /// Kills the zeros of a latency histogram with no response written as measured, in each
    /// version's shape, the skip counted, and the counts and windows dropped with it.
    #[test]
    fn adapter_http_autocannon_skips_latency_with_no_response() {
        for (suffix, name, sent) in [
            ("drop.json", "http://localhost:8080/drop", 73_174.0),
            ("refused.json", "http://localhost:9/none", 118_148.0),
            ("refused_4_6_0.json", "http://localhost:9/none", 77_688.0),
        ] {
            let results = read(suffix);
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
            assert_eq!(
                value(&results, name, DEFAULTS, Latency::SLUG_STR, "max"),
                None,
                "{suffix}"
            );
            assert_eq!(
                value(&results, name, DEFAULTS, Requests::SLUG_STR, "value"),
                Some(sent),
                "{suffix}"
            );
            assert_eq!(
                value(
                    &results,
                    name,
                    DEFAULTS,
                    Throughput::SLUG_STR,
                    "window_mean"
                ),
                Some(0.0),
                "{suffix}"
            );
        }
    }

    /// Kills latency before 6.0.0 read when a response was counted but its time was not
    /// recorded, as `-x` records only a 2xx's, or dropped when every recorded time is a 5xx's.
    #[test]
    fn adapter_http_autocannon_reads_latency_before_6_0_0_by_its_max() {
        const ERRORS: &str = "http://localhost:18191/";
        let results = read("exclude_500_4_6_0.json");
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(value(&results, ERRORS, ONE, Latency::SLUG_STR, "max"), None);
        assert_value(
            &results,
            (ERRORS, ONE),
            (Responses5xx::SLUG_STR, "value"),
            49.0,
        );
        assert_value(
            &read("all_500_4_6_0.json"),
            (ERRORS, ONE),
            (Latency::SLUG_STR, "max"),
            46_905_957.0,
        );
    }

    /// Kills latency read only when its median is not 0, which a run whose responses were
    /// nearly all faster than 1 ms records as 0.
    #[test]
    fn adapter_http_autocannon_reads_sub_millisecond_latency() {
        let results = read("fast.json");
        assert_value(
            &results,
            (FAST, DEFAULTS),
            (Latency::SLUG_STR, "mean"),
            30_000.0,
        );
        assert_eq!(
            value(&results, FAST, DEFAULTS, Latency::SLUG_STR, "median"),
            Some(0.0)
        );
    }

    /// Kills a run before 6.0.0, with no sample count and no codes, refused, or its fractional
    /// latency rounded.
    #[test]
    fn adapter_http_autocannon_reads_4_6_0() {
        let results = read("old_4_6_0_mixed.json");
        assert_eq!(results.skipped, Skipped::default());
        for (measure, metric, expected) in [
            (Latency::SLUG_STR, "max", 3_427_846.0),
            (Latency::SLUG_STR, "min", 0.0),
            (Responses5xx::SLUG_STR, "value", 30.0),
            (Requests::SLUG_STR, "value", 257.0),
        ] {
            assert_value(&results, (MIXED, DEFAULTS), (measure, metric), expected);
        }
        assert_eq!(
            value(&results, MIXED, DEFAULTS, "responses-500", "value"),
            None
        );
    }

    /// Kills a warm-up run refused, its warm-up read in place of its main run, or stored beside it.
    #[test]
    fn adapter_http_autocannon_reads_main_run_after_warmup() {
        let results = read("warmup.json");
        assert_eq!(results.resolved_adapter, Some(LEAF));
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(variants(&results), 2);
        for (measure, metric, expected) in [
            (Requests::SLUG_STR, "value", 309_511.0),
            (Responses2xx::SLUG_STR, "value", 309_501.0),
            (Throughput::SLUG_STR, "window_mean", 103_178.67),
        ] {
            assert_value(&results, (FAST, DEFAULTS), (measure, metric), expected);
        }
    }

    /// Kills a leaf that reads one of several documents: 2 runs one after the other, and a
    /// warm-up run with a third document after it.
    #[test]
    fn adapter_http_autocannon_refuses_several_documents() {
        let settings = Settings::new(None, BmfVersion::V1);
        let concat = fixture("concat.json");
        assert_eq!(LEAF.convert(&concat, settings), None);
        let run = concat.lines().next().expect("no line");
        assert!(LEAF.convert(run, settings).is_some());
        let warmup = fixture("warmup.json");
        assert_eq!(LEAF.convert(&format!("{warmup}{run}\n"), settings), None);
    }

    /// Kills a main run read after a warm-up that its nested warm-up does not equal.
    #[test]
    fn adapter_http_autocannon_refuses_another_warmup() {
        let settings = Settings::new(None, BmfVersion::V1);
        let warmup = fixture("warmup.json");
        let (first, second) = warmup.split_once('\n').expect("no second document");
        let mut main = serde_json::from_str::<serde_json::Value>(second).expect("not JSON");
        let unedited = format!("{first}\n{main}\n");
        assert!(LEAF.convert(&unedited, settings).is_some());
        // The same main run, whose nested warm-up ran with 3 connections, not the 2 printed first.
        main["warmup"]["connections"] = 3.into();
        let edited = format!("{first}\n{main}\n");
        assert_ne!(edited, unedited);
        assert_eq!(LEAF.convert(&edited, settings), None);
    }

    fn fixture(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/http/autocannon/{suffix}"))
            .expect("Failed to read a fixture")
    }

    /// Kills a time scaled in floating point alone.
    #[test]
    fn adapter_http_autocannon_scales_in_decimal() {
        let results = convert_edited_v1(LEAF, "http/autocannon/mixed_timeout.json", |json| {
            // No real capture has a time that floating point scales off, and 2.01 ms is
            // 2009999.9999999998 ns in floating point.
            json["latency"]["mean"] = 2.01.into();
        });
        assert_eq!(
            value(&results, MIXED, DEFAULTS, Latency::SLUG_STR, "mean"),
            Some(2_010_000.0)
        );
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_http_autocannon_ignores_average() {
        assert_ignores_average(LEAF, "http/autocannon/mixed_timeout.json");
    }
}
