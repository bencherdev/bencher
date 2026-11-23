use std::{borrow::Cow, collections::HashMap};

use bencher_json::project::report::Adapter;
use serde::{Deserialize, de::IgnoredAny};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        http::locust::{Entry, nanos},
        leaf::LeafResults,
    },
    results::adapter_results::AdapterResults,
};

/// Locust's `--json` output, as Locust 2.14.1 and later print it.
///
/// Locust prints each request's counters and response time histogram (`print_stats_json`,
/// `locust/stats.py:863-864` at 2.47.0), from which the leaf computes Locust's own statistics
/// by the formulas of Locust 2.46.6 and later; an older Locust's median and percentiles can
/// differ by one bucket, and the document does not say which Locust printed it. Under
/// `--processes`, each worker prints `[]` on a line of its own before the master's document,
/// which the leaf reads.
///
/// The document holds no whole-test entry (`serialize_stats`, `stats.py:287-290`) and not the
/// whole test's start, which Locust's rates divide by (`stats.py:476-493`), so it gives no rate.
pub struct AdapterHttpLocustJson;

impl Adaptable for AdapterHttpLocustJson {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let entries = serde_json::from_str::<Vec<StatsEntry>>(last_document(input)).ok()?;
        let mut results = LeafResults::default();
        for entry in &entries {
            entry.entry()?.insert(&mut results);
        }
        results.finish_with_skips(Adapter::HttpLocustJson)
    }
}

/// The input after the `[]` lines that come before another document.
fn last_document(input: &str) -> &str {
    let mut document = input;
    while let Some(rest) = document.strip_prefix(EMPTY).and_then(|rest| {
        rest.strip_prefix('\n')
            .or_else(|| rest.strip_prefix("\r\n"))
    }) && !rest.trim().is_empty()
    {
        document = rest;
    }
    document
}

const EMPTY: &str = "[]";
/// The percentiles Locust reports by default (`PERCENTILES_TO_REPORT`, `stats.py:118`), but the
/// median, which `median_response_time` keeps within the exact `min` and `max`, and the 100th,
/// the histogram's top, which `max` holds exactly.
const PERCENTILES: [(f64, &str); 9] = [
    (0.66, "p66"),
    (0.75, "p75"),
    (0.80, "p80"),
    (0.90, "p90"),
    (0.95, "p95"),
    (0.98, "p98"),
    (0.99, "p99"),
    (0.999, "p99_9"),
    (0.9999, "p99_99"),
];

/// One request's `StatsEntryDict` (`stats.py:54-66`).
#[derive(Deserialize)]
struct StatsEntry {
    name: String,
    method: String,
    num_requests: f64,
    /// Requests that recorded no response time.
    num_none_requests: f64,
    num_failures: f64,
    /// The sum of every response time, in milliseconds.
    total_response_time: f64,
    min_response_time: Option<f64>,
    max_response_time: f64,
    total_content_length: f64,
    /// How many response times Locust rounded to each key (`bucket_response_time`,
    /// `stats.py:124-142`), in milliseconds.
    response_times: HashMap<String, f64>,
    #[serde(rename = "start_time")]
    _start_time: IgnoredAny,
    #[serde(rename = "last_request_timestamp")]
    _last_request_timestamp: IgnoredAny,
    #[serde(rename = "num_reqs_per_sec")]
    _num_reqs_per_sec: IgnoredAny,
    #[serde(rename = "num_fail_per_sec")]
    _num_fail_per_sec: IgnoredAny,
}

impl StatsEntry {
    /// The entry with Locust's own statistics, or `None` when a histogram key is not a number.
    fn entry(&self) -> Option<Entry<'_>> {
        let mut histogram = self
            .response_times
            .iter()
            .map(|(key, count)| Some((key.parse::<f64>().ok()?, key.as_str(), *count)))
            .collect::<Option<Vec<_>>>()?;
        histogram.sort_by(|(a, _, _), (b, _, _)| b.total_cmp(a));
        let sampled = self.num_requests - self.num_none_requests;
        // With no response time, Locust's mean and median fall back to 0 (`stats.py:428-437`).
        let latency = if sampled > 0.0 && !histogram.is_empty() {
            self.latency(&histogram, sampled)?
        } else {
            Vec::new()
        };
        Some(Entry {
            request: Some((self.method.as_str(), self.name.as_str())),
            requests: self.num_requests,
            failures: self.num_failures,
            latency,
            // `avg_content_length` (`stats.py:494-499`).
            size_per_request: (self.num_requests > 0.0)
                .then(|| self.total_content_length / self.num_requests),
            rates: None,
        })
    }

    /// Each latency statistic, in nanoseconds.
    fn latency(
        &self,
        histogram: &[(f64, &str, f64)],
        sampled: f64,
    ) -> Option<Vec<(Cow<'static, str>, f64)>> {
        let mut latency = vec![
            (Cow::Borrowed("median"), self.median(histogram, sampled)?),
            // `avg_response_time` (`stats.py:427-432`).
            (
                Cow::Borrowed("mean"),
                millis(self.total_response_time / sampled)?,
            ),
        ];
        if let Some(min) = self.min_response_time {
            latency.push((Cow::Borrowed("min"), millis(min)?));
        }
        latency.push((Cow::Borrowed("max"), millis(self.max_response_time)?));
        for (percent, metric) in PERCENTILES {
            let (_, time) = percentile(histogram, sampled, percent);
            latency.push((Cow::Borrowed(metric), nanos(time)?));
        }
        Some(latency)
    }

    /// `median_response_time` (`stats.py:434-452`): the histogram's median, kept within the exact
    /// `min` and `max`.
    fn median(&self, histogram: &[(f64, &str, f64)], sampled: f64) -> Option<f64> {
        let (median, time) = percentile(histogram, sampled, 0.5);
        if median > self.max_response_time {
            millis(self.max_response_time)
        } else if let Some(min) = self.min_response_time
            && median < min
        {
            millis(min)
        } else {
            nanos(time)
        }
    }
}

/// `calculate_response_time_percentile` (`stats.py:161-179`): the largest key at or under which
/// Locust's `int(num_requests * percent)` of the `sampled` requests fall, else `0`, with the
/// histogram sorted from its largest key.
fn percentile<'a>(histogram: &[(f64, &'a str, f64)], sampled: f64, percent: f64) -> (f64, &'a str) {
    let within = (sampled * percent).trunc();
    let mut processed = 0.0;
    for &(time, key, count) in histogram {
        processed += count;
        if sampled - processed <= within {
            return (time, key);
        }
    }
    (0.0, "0")
}

/// A time in milliseconds that Locust holds as a float, in nanoseconds.
fn millis(value: f64) -> Option<f64> {
    nanos(&value.to_string())
}

#[cfg(test)]
mod test_http_locust_json {
    use std::collections::BTreeSet;

    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::Latency,
                http::{FailedRequests, Requests, SizePerRequest},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::{
            http::locust_json::AdapterHttpLocustJson,
            test_util::{Expected, assert_variant, convert_v1, value},
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpLocustJson;
    const GET: &str = r#"{"@method":"GET","@type":"http"}"#;
    const GET_RECEIVED: &str = r#"{"@direction":"received","@method":"GET","@type":"http"}"#;

    fn fixture(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/http/locust/{suffix}"))
            .expect("Failed to read a fixture")
    }

    fn parse(input: &str) -> Option<AdapterResults> {
        AdapterHttpLocustJson::parse(input, Settings::new(None, BmfVersion::V1))
    }

    /// Milliseconds as Locust computes them, in nanoseconds.
    fn ms(millis: f64) -> f64 {
        millis * 1e6
    }

    /// `latency` as Locust's own `StatsEntry` computes it from the same document (the
    /// research fixtures' `oracle/oracle.tsv`): its median, mean, min, max, and the default
    /// levels from `p66` to `p99_99`, in milliseconds.
    fn latency(median: f64, mean: f64, min: f64, max: f64, levels: [f64; 9]) -> Expected {
        let mut metrics = vec![
            ("median", ms(median)),
            ("mean", ms(mean)),
            ("min", ms(min)),
            ("max", ms(max)),
        ];
        metrics.extend(
            [
                "p66", "p75", "p80", "p90", "p95", "p98", "p99", "p99_9", "p99_99",
            ]
            .into_iter()
            .zip(levels.map(ms)),
        );
        (Latency::SLUG_STR, metrics)
    }

    fn count(measure: &'static str, count: f64) -> Expected {
        (measure, vec![("value", count)])
    }

    fn size(mean: f64) -> Expected {
        (SizePerRequest::SLUG_STR, vec![("mean", mean)])
    }

    /// Each entry, its statistics computed as Locust's own `StatsEntry` computes them from the
    /// same document: a median kept up to `min`, one at a key past 100 ms, keys that Locust
    /// 2.14 writes as floats, a custom client's type, and percentiles whose share of requests
    /// ends in a fraction and falls on a key's last request.
    ///
    /// Kills a median not kept within `min`, a percentile that rounds Locust's
    /// `int(num_requests * percent)` or stops one key early or late, a float key refused, a mean
    /// over the requests that recorded no time, and a size per request over another count.
    #[test]
    fn adapter_http_locust_json_computes_locust_own_statistics() {
        let thousand = 1_000.0;
        for (suffix, benchmark, parameters, expected) in [
            (
                "multi.json",
                "/mixed",
                GET,
                [
                    latency(
                        0.222_820_999_965_733_8,
                        41.636_095_359_077_28,
                        0.222_820_999_965_733_8,
                        1_001.796_645_000_013_1,
                        [
                            1.0, 1.0, 1.0, 1.0, 2.0, thousand, thousand, thousand, thousand,
                        ],
                    ),
                    count(Requests::SLUG_STR, 997.0),
                    count(FailedRequests::SLUG_STR, 303.0),
                ],
            ),
            (
                "multi.json",
                "/post",
                r#"{"@method":"POST","@type":"http"}"#,
                [
                    latency(
                        0.243_935_000_014_516_85,
                        0.559_693_507_968_623_7,
                        0.243_935_000_014_516_85,
                        5.554_420_000_009_941,
                        [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 6.0, 6.0],
                    ),
                    count(Requests::SLUG_STR, 502.0),
                    count(FailedRequests::SLUG_STR, 0.0),
                ],
            ),
            (
                "basic.json",
                "/slow",
                GET,
                [
                    latency(
                        51.0,
                        51.420_983_829_950_586,
                        50.366_783_000_015_4,
                        73.303_293_000_037_68,
                        [51.0, 52.0, 52.0, 52.0, 52.0, 53.0, 54.0, 73.0, 73.0],
                    ),
                    count(Requests::SLUG_STR, 1_823.0),
                    count(FailedRequests::SLUG_STR, 0.0),
                ],
            ),
            (
                "v2_14_1_multi.json",
                "/mixed",
                GET,
                [
                    latency(
                        1.0,
                        52.021_930_404_984_91,
                        0.259_940_978_139_638_9,
                        1_002.422_032_994_218_2,
                        [
                            1.0, 1.0, 1.0, 2.0, thousand, thousand, thousand, thousand, thousand,
                        ],
                    ),
                    count(Requests::SLUG_STR, 898.0),
                    count(FailedRequests::SLUG_STR, 266.0),
                ],
            ),
            (
                "scenarios.json",
                "select one",
                r#"{"@type":"SQL"}"#,
                [
                    latency(
                        2.003_352_999_963_681,
                        2.479_690_673_014_402_6,
                        2.003_352_999_963_681,
                        13.896_038_000_041_244,
                        [3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 4.0, 14.0, 14.0],
                    ),
                    count(Requests::SLUG_STR, 1_523.0),
                    count(FailedRequests::SLUG_STR, 0.0),
                ],
            ),
        ] {
            let results = convert_v1(LEAF, &format!("http/locust/{suffix}"));
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
            assert_variant(&results, benchmark, parameters, &expected);
        }
        let results = convert_v1(LEAF, "http/locust/multi.json");
        assert_variant(
            &results,
            "/mixed",
            GET_RECEIVED,
            &[size(2.088_264_794_383_149_6)],
        );
    }

    /// Locust's median is kept down to the exact `max` when a few slow requests round up past
    /// it. Edited from a real capture to one request of 147 ms, which Locust rounds to the
    /// 150 ms key.
    ///
    /// Kills a median not kept within `max`.
    #[test]
    fn adapter_http_locust_json_keeps_the_median_within_max() {
        let mut json = serde_json::from_str::<serde_json::Value>(&fixture("basic.json")).unwrap();
        let entry = &mut json[1];
        entry["num_requests"] = 1.into();
        entry["total_response_time"] = 147.0.into();
        entry["min_response_time"] = 147.0.into();
        entry["max_response_time"] = 147.0.into();
        entry["response_times"] = serde_json::json!({"150": 1});
        let results = parse(&json.to_string()).unwrap();
        assert_eq!(
            value(&results, "/slow", GET, Latency::SLUG_STR, "median"),
            Some(ms(147.0))
        );
        assert_eq!(
            value(&results, "/slow", GET, Latency::SLUG_STR, "p99"),
            Some(ms(150.0))
        );
    }

    /// Locust's times leave out the requests that recorded none (`num_none_requests`), which no
    /// capture holds. Edited from a real capture to add 100 such requests, which keep the
    /// histogram and the time total as they are.
    ///
    /// Kills the mean, the median, or a percentile over every request, and the size per request
    /// over only the requests that recorded a time.
    #[test]
    fn adapter_http_locust_json_leaves_out_requests_with_no_time() {
        let input = fixture("multi.json");
        let unedited = parse(&input).unwrap();
        let mut json = serde_json::from_str::<serde_json::Value>(&input).unwrap();
        let entry = &mut json[2];
        assert_eq!(entry["name"], "/mixed");
        entry["num_requests"] = 1_097.into();
        entry["num_none_requests"] = 100.into();
        let edited = parse(&json.to_string()).unwrap();
        let latency = |results: &AdapterResults| {
            results.inner.iter().find_map(|(benchmark, variants)| {
                (benchmark.to_string() == "/mixed")
                    .then(|| variants[&GET.parse().unwrap()].inner[&Latency::name_id()].clone())
            })
        };
        assert_eq!(latency(&edited), latency(&unedited));
        assert_eq!(
            value(&edited, "/mixed", GET, Requests::SLUG_STR, "value"),
            Some(1_097.0)
        );
        let total = json[2]["total_content_length"].as_f64().unwrap();
        assert_eq!(
            value(
                &edited,
                "/mixed",
                GET_RECEIVED,
                SizePerRequest::SLUG_STR,
                "mean"
            ),
            Some(total / 1_097.0)
        );
    }

    /// Kills a mean scaled through a float, which gives 511774.42404039996 ns for the
    /// 0.5117744240404 ms Locust computes.
    #[test]
    fn adapter_http_locust_json_scales_the_decimal() {
        let results = convert_v1(LEAF, "http/locust/multi.json");
        let mean = value(&results, "/fast", GET, Latency::SLUG_STR, "mean").unwrap();
        assert_eq!(mean.to_bits(), 511_774.424_040_4f64.to_bits(), "{mean}");
    }

    /// Kills a rate, a whole-test benchmark, or a failure split made up from the document, and a
    /// custom client's type read as HTTP.
    #[test]
    fn adapter_http_locust_json_writes_what_the_document_holds() {
        let results = convert_v1(LEAF, "http/locust/scenarios.json");
        assert!(results.nameless.is_none());
        let shape = results
            .inner
            .iter()
            .flat_map(|(benchmark, variants)| {
                variants
                    .keys()
                    .map(move |parameters| (benchmark.to_string(), parameters.canonical()))
            })
            .collect::<BTreeSet<_>>();
        let post = r#"{"@method":"POST","@type":"http"}"#;
        let post_received = r#"{"@direction":"received","@method":"POST","@type":"http"}"#;
        let sql = r#"{"@type":"SQL"}"#;
        let sql_received = r#"{"@direction":"received","@type":"SQL"}"#;
        assert_eq!(
            shape,
            BTreeSet::from(
                [
                    ("home", GET),
                    ("home", GET_RECEIVED),
                    ("pay", post),
                    ("pay", post_received),
                    ("refund", GET),
                    ("refund", GET_RECEIVED),
                    ("select one", sql),
                    ("select one", sql_received),
                ]
                .map(|(benchmark, parameters)| (benchmark.to_owned(), parameters.to_owned()))
            )
        );
        assert_variant(&results, "select one", sql_received, &[size(8.0)]);
    }

    /// Under `--processes`, each worker prints `[]` before the master's document.
    ///
    /// Kills a leaf that refuses the `[]` lines, or reads one of them in place of the master's
    /// document.
    #[test]
    fn adapter_http_locust_json_reads_the_master_document() {
        let input = fixture("processes.json");
        assert!(input.starts_with("[]\n[]\n[]\n[]\n[\n"));
        let results = parse(&input).unwrap();
        assert_eq!(results.inner.len(), 5);
        assert_eq!(
            value(
                &results,
                "/post",
                r#"{"@method":"POST","@type":"http"}"#,
                Requests::SLUG_STR,
                "value"
            ),
            Some(869.0)
        );
    }

    /// Edited from real captures: two runs' documents joined, and a document after the
    /// master's.
    ///
    /// Kills a leaf that reads the first or the last of several documents.
    #[test]
    fn adapter_http_locust_json_refuses_several_documents() {
        let basic = fixture("basic.json");
        let multi = fixture("multi.json");
        assert_eq!(parse(&format!("{basic}{multi}")), None);
        assert_eq!(parse(&format!("{basic}[]\n")), None);
    }
}
