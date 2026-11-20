use std::collections::BTreeMap;

use bencher_json::{
    BenchmarkName, MeasureNameId,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Latency, Throughput},
            http::{
                FailedRequests, Requests, Responses1xx, Responses2xx, Responses3xx, Responses4xx,
                Responses5xx,
            },
        },
        report::Adapter,
    },
};
use rust_decimal::Decimal;
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

/// bombardier at version 1: its `-o json` result, one benchmark named by its URL, with the method
/// and the settings it prints as parameters.
pub struct AdapterHttpBombardier;

impl Adaptable for AdapterHttpBombardier {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let report = serde_json::from_str::<Report>(input).ok()?;
        let mut results = LeafResults::default();
        report.insert(&mut results);
        results.finish_with_skips(Adapter::HttpBombardier)
    }
}

/// The keys the leaf reads (`templates.go:68-170` at v2.0.2). The headers, the body, and the
/// certificate paths are never read.
#[derive(Deserialize)]
struct Report {
    spec: Spec,
    result: Outcome,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Spec {
    number_of_connections: Number,
    #[serde(flatten)]
    limit: Limit,
    method: String,
    url: String,
    stream: bool,
    timeout_seconds: Number,
    client: String,
    /// Printed only when `-r` sets it.
    rate: Option<Number>,
}

/// What ends the run.
#[derive(Deserialize)]
#[serde(tag = "testType")]
enum Limit {
    #[serde(rename = "timed", rename_all = "camelCase")]
    Duration { test_duration_seconds: Number },
    #[serde(rename = "number-of-requests", rename_all = "camelCase")]
    Requests { number_of_requests: Number },
}

/// A status code outside 100 to 599, a transport error's included, counts in `others`
/// (`bombardier.go:234-247`).
#[derive(Deserialize)]
struct Outcome {
    req1xx: Finite,
    req2xx: Finite,
    req3xx: Finite,
    req4xx: Finite,
    req5xx: Finite,
    others: Finite,
    /// In microseconds, every response's time, an error's included.
    latency: Statistics,
    /// The rate of each window of about 20 ms (`bombardier.go:292-325`).
    rps: Statistics,
}

/// The percentiles print for `latency` only with `-l`, keyed by the percent.
#[derive(Deserialize)]
struct Statistics {
    mean: Finite,
    stddev: Finite,
    max: Finite,
    #[serde(default)]
    percentiles: BTreeMap<String, Finite>,
}

const VALUE: &str = "value";
const NANOS_PER_MICRO: u32 = 1_000;

impl Report {
    fn insert(self, results: &mut LeafResults) {
        let Ok(name) = self.spec.url.parse::<BenchmarkName>() else {
            results.skip_benchmark();
            return;
        };
        let Some(parameters) = parameter_set(self.spec.parameters()) else {
            results.skip_variant();
            return;
        };
        let mut variant = results.variant(name, parameters);
        let Outcome {
            req1xx,
            req2xx,
            req3xx,
            req4xx,
            req5xx,
            others,
            latency,
            rps,
        } = self.result;
        latency.insert(&mut variant, &Latency::name_id(), "", NANOS_PER_MICRO);
        rps.insert(&mut variant, &Throughput::name_id(), "window_", 1);
        let counts = [req1xx, req2xx, req3xx, req4xx, req5xx, others];
        let requests = counts.into_iter().map(Finite::get).sum::<Option<f64>>();
        for (measure, count) in [
            (Requests::name_id(), Finite::from(requests)),
            (FailedRequests::name_id(), others),
            (Responses1xx::name_id(), req1xx),
            (Responses2xx::name_id(), req2xx),
            (Responses3xx::name_id(), req3xx),
            (Responses4xx::name_id(), req4xx),
            (Responses5xx::name_id(), req5xx),
        ] {
            count.insert(&mut variant, &measure, VALUE, 1.0);
        }
    }
}

impl Spec {
    fn parameters(&self) -> Map<String, Value> {
        let mut parameters = Map::from_iter([
            ("@method".to_owned(), self.method.clone().into()),
            (
                "@connections".to_owned(),
                self.number_of_connections.clone().into(),
            ),
            ("@timeout".to_owned(), self.timeout_seconds.clone().into()),
            ("@client".to_owned(), self.client.clone().into()),
            ("@stream".to_owned(), self.stream.into()),
        ]);
        let (key, limit) = match &self.limit {
            Limit::Duration {
                test_duration_seconds,
            } => ("@duration", test_duration_seconds),
            Limit::Requests { number_of_requests } => ("@requests", number_of_requests),
        };
        parameters.insert(key.to_owned(), limit.clone().into());
        if let Some(rate) = &self.rate {
            parameters.insert("@rate".to_owned(), rate.clone().into());
        }
        parameters
    }
}

impl Statistics {
    /// Each statistic as `measure`'s metric with `prefix` in front, times `times`.
    fn insert(self, variant: &mut Variant<'_>, measure: &MeasureNameId, prefix: &str, times: u32) {
        let statistics = [
            ("mean".to_owned(), self.mean),
            ("std_dev".to_owned(), self.stddev),
            ("max".to_owned(), self.max),
        ]
        .into_iter()
        .chain(
            self.percentiles
                .into_iter()
                .filter_map(|(percent, value)| Some((percentile_name(&percent)?, value))),
        );
        for (metric, value) in statistics {
            Finite::from(value.get().map(|value| scale(value, times))).insert(
                variant,
                measure,
                &format!("{prefix}{metric}"),
                1.0,
            );
        }
    }
}

/// `value` times `times`, in decimal arithmetic on the shortest text that reads back as `value`,
/// so `802.500551502136` µs is `802500.551502136` ns, else in floating point past the decimal
/// range.
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
mod test_http_bombardier {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Latency, Throughput},
                http::{
                    FailedRequests, Requests, Responses1xx, Responses2xx, Responses3xx,
                    Responses4xx, Responses5xx,
                },
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_variant, convert_v1, value, variants,
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpBombardier;
    const FAST: &str = "http://localhost:8080/fast";
    const MIXED: &str = "http://localhost:8080/mixed";
    const COUNT: &str = r#"{"@client":"fasthttp","@connections":10,"@method":"GET","@requests":5000,"@stream":false,"@timeout":2}"#;

    fn read(suffix: &str) -> AdapterResults {
        convert_v1(LEAF, &format!("http/bombardier/{suffix}"))
    }

    fn fixture(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/http/bombardier/{suffix}"))
            .expect("Failed to read a fixture")
    }

    fn metrics(measure: &'static str, metrics: &[(&'static str, f64)]) -> Expected {
        (measure, metrics.to_vec())
    }

    fn count(measure: &'static str, value: f64) -> Expected {
        (measure, vec![("value", value)])
    }

    /// The counts of a run: every request, then `1xx` to `5xx`, then `others`.
    fn counts(requests: f64, responses: [f64; 5], others: f64) -> [Expected; 7] {
        let [
            informational,
            successful,
            redirection,
            client_error,
            server_error,
        ] = responses;
        [
            count(Requests::SLUG_STR, requests),
            count(FailedRequests::SLUG_STR, others),
            count(Responses1xx::SLUG_STR, informational),
            count(Responses2xx::SLUG_STR, successful),
            count(Responses3xx::SLUG_STR, redirection),
            count(Responses4xx::SLUG_STR, client_error),
            count(Responses5xx::SLUG_STR, server_error),
        ]
    }

    /// Kills a latency left in microseconds, a percentile key not read as its canonical name,
    /// a window statistic written without `window_` or as the run's rate, a class count dropped
    /// or put in another class, `requests` read from one counter, and the byte totals or the
    /// time taken written.
    #[test]
    fn adapter_http_bombardier_measures() {
        let results = read("bombardier_mixed.json");
        assert_eq!(results.resolved_adapter, Some(LEAF));
        assert_eq!(results.version, BmfVersion::V1);
        assert!(results.nameless.is_none());
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(variants(&results), 1);
        let mut expected = vec![
            metrics(
                Latency::SLUG_STR,
                &[
                    ("mean", 72_178_411.627_906_97),
                    ("std_dev", 291_029_744.620_708_9),
                    ("max", 3_003_068_000.0),
                    ("median", 33_000.0),
                    ("p75", 53_000.0),
                    ("p90", 180_000.0),
                    ("p95", 1_000_166_000.0),
                    ("p99", 1_001_147_000.0),
                ],
            ),
            metrics(
                Throughput::SLUG_STR,
                &[
                    ("window_mean", 142.359_126_463_285_92),
                    ("window_std_dev", 1_089.900_366_364_546_7),
                    ("window_max", 10_901.029_864_314_673),
                    ("window_median", 0.0),
                    ("window_p75", 0.0),
                    ("window_p90", 0.0),
                    ("window_p95", 0.0),
                    ("window_p99", 7_413.756_569),
                ],
            ),
        ];
        expected.extend(counts(430.0, [0.0, 323.0, 0.0, 59.0, 48.0], 0.0));
        assert_variant(
            &results,
            MIXED,
            r#"{"@client":"fasthttp","@connections":10,"@duration":3,"@method":"GET","@stream":false,"@timeout":1}"#,
            &expected,
        );
    }

    /// Kills `failed-requests` read from another counter, `others` left out of `requests`, and a
    /// `timeouts` count read from the error list, which holds 50 timeouts here.
    #[test]
    fn adapter_http_bombardier_others_are_failed_requests() {
        let results = read("mixed_http1.json");
        let mut expected = vec![
            metrics(
                Latency::SLUG_STR,
                &[
                    ("mean", 48_101_976.989_453_5),
                    ("std_dev", 207_676_647.712_832_9),
                    ("max", 1_001_599_000.0),
                    ("median", 71_000.0),
                    ("p75", 113_000.0),
                    ("p90", 384_000.0),
                    ("p95", 8_657_000.0),
                    ("p99", 1_000_712_000.0),
                ],
            ),
            metrics(
                Throughput::SLUG_STR,
                &[
                    ("window_mean", 207.309_591_555_029_7),
                    ("window_std_dev", 1_356.453_883_942_799_2),
                    ("window_max", 12_239.879_956_215_314),
                    ("window_median", 0.0),
                    ("window_p75", 0.0),
                    ("window_p90", 0.0),
                    ("window_p95", 0.0),
                    ("window_p99", 9_638.366_965),
                ],
            ),
        ];
        expected.extend(counts(1_043.0, [0.0, 734.0, 0.0, 103.0, 107.0], 99.0));
        assert_variant(
            &results,
            MIXED,
            r#"{"@client":"net/http.v1","@connections":10,"@duration":5,"@method":"GET","@stream":false,"@timeout":1}"#,
            &expected,
        );
    }

    /// Kills the latency percentiles required, so a run without `-l` is refused, or written
    /// when bombardier prints none.
    #[test]
    fn adapter_http_bombardier_percentiles_need_l() {
        let results = read("fast_no_l.json");
        let mut expected = vec![
            metrics(
                Latency::SLUG_STR,
                &[
                    ("mean", 57_564.687_399_592_75),
                    ("std_dev", 10_951.482_544_373_047),
                    ("max", 1_232_000.0),
                ],
            ),
            metrics(
                Throughput::SLUG_STR,
                &[
                    ("window_mean", 169_109.371_917_985_93),
                    ("window_std_dev", 5_831.097_918_113_135),
                    ("window_max", 181_799.249_416_666_98),
                    ("window_median", 169_850.136_902),
                    ("window_p75", 172_664.497_897),
                    ("window_p90", 174_987.569_754),
                    ("window_p95", 176_058.798_049),
                    ("window_p99", 179_053.133_43),
                ],
            ),
        ];
        expected.extend(counts(507_309.0, [0.0, 507_309.0, 0.0, 0.0, 0.0], 0.0));
        assert_variant(
            &results,
            FAST,
            r#"{"@client":"fasthttp","@connections":10,"@duration":3,"@method":"GET","@stream":false,"@timeout":2}"#,
            &expected,
        );
    }

    /// Kills a setting dropped, misnamed, or read from another key: `@requests` for a run
    /// with `-n` in place of `@duration`, `@rate` only when `-r` prints it, a timeout under
    /// 1 s cut to a whole number, the method, `@stream`, and each client.
    #[test]
    fn adapter_http_bombardier_parameters() {
        for (suffix, name, parameters) in [
            ("count.json", FAST, COUNT),
            (
                "stream.json",
                FAST,
                r#"{"@client":"fasthttp","@connections":2,"@method":"GET","@requests":200,"@stream":true,"@timeout":2}"#,
            ),
            (
                "rate.json",
                FAST,
                r#"{"@client":"fasthttp","@connections":10,"@duration":3,"@method":"GET","@rate":500,"@stream":false,"@timeout":2}"#,
            ),
            (
                "timeout_500ms.json",
                FAST,
                r#"{"@client":"fasthttp","@connections":10,"@duration":1,"@method":"GET","@stream":false,"@timeout":0.5}"#,
            ),
            (
                "mixed_http2.json",
                "https://localhost:8443/mixed",
                r#"{"@client":"net/http.v2","@connections":10,"@duration":5,"@method":"GET","@stream":false,"@timeout":1}"#,
            ),
            (
                "post_header.json",
                "http://localhost:8080/post",
                r#"{"@client":"fasthttp","@connections":10,"@duration":3,"@method":"POST","@stream":false,"@timeout":2}"#,
            ),
        ] {
            let results = read(suffix);
            assert_eq!(variants(&results), 1, "{suffix}");
            assert!(
                value(&results, name, parameters, Requests::SLUG_STR, "value").is_some(),
                "{suffix}"
            );
        }
    }

    /// Kills `requests` summed without the 3xx counter, and `responses-3xx` and `responses-1xx`
    /// read from each other's counter.
    #[test]
    fn adapter_http_bombardier_counts_redirects() {
        let results = read("redirect.json");
        for (measure, metrics) in counts(200.0, [0.0, 0.0, 200.0, 0.0, 0.0], 0.0) {
            for (metric, expected) in metrics {
                assert_eq!(
                    value(
                        &results,
                        "http://localhost:8080/dir",
                        r#"{"@client":"fasthttp","@connections":2,"@method":"GET","@requests":200,"@stream":false,"@timeout":2}"#,
                        measure,
                        metric
                    ),
                    Some(expected),
                    "{measure}"
                );
            }
        }
    }

    /// Kills a printed `std_dev` of 0 skipped: bombardier prints 0 for 2 windows or fewer.
    #[test]
    fn adapter_http_bombardier_writes_a_zero_std_dev() {
        assert_eq!(
            value(
                &read("count.json"),
                FAST,
                COUNT,
                Throughput::SLUG_STR,
                "window_std_dev"
            ),
            Some(0.0)
        );
    }

    /// Kills a request header or the body copied into a parameter or a name.
    #[test]
    fn adapter_http_bombardier_never_reads_headers_or_body() {
        for suffix in ["post_header.json", "old_1_2_6_post_rate.json"] {
            assert!(fixture(suffix).contains("Bearer SECRET"), "{suffix}");
            let results = read(suffix);
            assert!(!format!("{results:?}").contains("SECRET"), "{suffix}");
        }
    }

    /// Kills a reader that needs a key v1.2.6 does not print, and a `-m POST -r` run of v1.2.6
    /// read with other parameters than v2.0.2 gives it. The keys are the same since v1.2.
    #[test]
    fn adapter_http_bombardier_reads_1_2_6() {
        let results = read("old_1_2_6.json");
        assert_eq!(results.skipped, Skipped::default());
        let mut expected = vec![
            metrics(
                Latency::SLUG_STR,
                &[
                    ("mean", 8_104_279.0),
                    ("std_dev", 64_852_795.118_149_54),
                    ("max", 1_049_034_000.0),
                    ("median", 3_557_000.0),
                    ("p75", 4_873_000.0),
                    ("p90", 6_549_000.0),
                    ("p95", 7_747_000.0),
                    ("p99", 10_789_000.0),
                ],
            ),
            metrics(
                Throughput::SLUG_STR,
                &[
                    ("window_mean", 934.988_991_923_947_8),
                    ("window_std_dev", 545.209_549_439_326_1),
                    ("window_max", 2_359.541_869_342_527_7),
                    ("window_median", 1_203.845_7),
                    ("window_p75", 1_562.741_737),
                    ("window_p90", 1_795.769_555),
                    ("window_p95", 2_058.207_824),
                    ("window_p99", 2_345.475_929),
                ],
            ),
        ];
        expected.extend(counts(2_000.0, [0.0, 2_000.0, 0.0, 0.0, 0.0], 0.0));
        assert_variant(
            &results,
            FAST,
            r#"{"@client":"fasthttp","@connections":10,"@method":"GET","@requests":2000,"@stream":false,"@timeout":2}"#,
            &expected,
        );
        let results = read("old_1_2_6_post_rate.json");
        assert_eq!(variants(&results), 1);
        assert!(
            value(
                &results,
                FAST,
                r#"{"@client":"fasthttp","@connections":10,"@duration":1,"@method":"POST","@rate":300,"@stream":false,"@timeout":0.5}"#,
                Responses5xx::SLUG_STR,
                "value"
            )
            .is_some()
        );
    }

    /// Kills a URL past the benchmark name limit read, cut, or skipped uncounted.
    #[test]
    fn adapter_http_bombardier_long_url_is_skipped_and_counted() {
        let results = read("long_url.json");
        assert!(results.inner.is_empty());
        assert_eq!(
            results.skipped,
            Skipped {
                benchmarks: 1,
                ..Skipped::default()
            }
        );
    }

    /// Kills a reader that takes the first of several results, or that skips bombardier's
    /// intro and progress lines before the JSON.
    #[test]
    fn adapter_http_bombardier_refuses_anything_but_one_result() {
        let settings = Settings::new(None, BmfVersion::V1);
        for suffix in [
            "concat.json",
            "count_intro.json",
            "default_print_json.stdout",
        ] {
            assert_eq!(LEAF.convert(&fixture(suffix), settings), None, "{suffix}");
        }
    }

    /// Kills a time scaled in floating point alone, which reads 802.500551502136 µs as
    /// 802500.5515021359 ns, and a decimal product read back with `to_f64`, which reads
    /// 3872.1122645136484 µs as 3872112.264513648 ns.
    #[test]
    fn adapter_http_bombardier_scales_in_decimal() {
        assert_eq!(
            value(
                &read("old_1_2_6_post_rate.json"),
                FAST,
                r#"{"@client":"fasthttp","@connections":10,"@duration":1,"@method":"POST","@rate":300,"@stream":false,"@timeout":0.5}"#,
                Latency::SLUG_STR,
                "std_dev"
            ),
            Some(802_500.551_502_136)
        );
        assert_eq!(
            value(
                &read("timeout_500ms.json"),
                FAST,
                r#"{"@client":"fasthttp","@connections":10,"@duration":1,"@method":"GET","@stream":false,"@timeout":0.5}"#,
                Latency::SLUG_STR,
                "mean"
            ),
            Some(3_872_112.264_513_648_6)
        );
    }

    /// Kills a statistic picked by the run's average setting.
    #[test]
    fn adapter_http_bombardier_ignores_average() {
        assert_ignores_average(LEAF, "http/bombardier/bombardier_mixed.json");
    }
}
