use std::collections::BTreeMap;

use bencher_json::project::report::Adapter;
use serde::Deserialize;

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        http::{
            k6::scale,
            vegeta::{Directions, Rates, Run},
        },
        leaf::LeafResults,
    },
    results::adapter_results::AdapterResults,
};

/// `vegeta report -type=json` (`lib/reporters.go:134-139` at 12.13.0), the same keys since 12.8.0.
///
/// A file of several reports, as `-every` writes, is not one JSON document and is refused.
pub struct AdapterHttpVegetaJson;

impl Adaptable for AdapterHttpVegetaJson {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let report = serde_json::from_str::<Report>(input).ok()?;
        let mut results = LeafResults::default();
        report.into_run().insert(&mut results);
        results.finish(Adapter::HttpVegetaJson)
    }
}

/// The keys the leaf reads (`lib/metrics.go:12-45`). The totals, the times, the error set, and the
/// histogram of `-buckets` are not read.
#[derive(Deserialize)]
struct Report {
    latencies: Latencies,
    bytes_in: Bytes,
    bytes_out: Bytes,
    /// The attack time in nanoseconds.
    duration: u64,
    requests: f64,
    rate: f64,
    throughput: f64,
    success: f64,
    status_codes: BTreeMap<u16, f64>,
}

/// Latencies in nanoseconds.
#[derive(Deserialize)]
struct Latencies {
    min: f64,
    mean: f64,
    #[serde(rename = "50th")]
    median: f64,
    #[serde(rename = "90th")]
    p90: f64,
    #[serde(rename = "95th")]
    p95: f64,
    #[serde(rename = "99th")]
    p99: f64,
    max: f64,
}

/// The mean bytes per request.
#[derive(Deserialize)]
struct Bytes {
    mean: f64,
}

impl Report {
    fn into_run(self) -> Run {
        let Self {
            latencies:
                Latencies {
                    min,
                    mean,
                    median,
                    p90,
                    p95,
                    p99,
                    max,
                },
            bytes_in,
            bytes_out,
            duration,
            requests,
            rate,
            throughput,
            success,
            status_codes,
        } = self;
        Run {
            latency: vec![
                ("min", min),
                ("mean", mean),
                ("median", median),
                ("p90", p90),
                ("p95", p95),
                ("p99", p99),
                ("max", max),
            ],
            rates: (duration > 0).then_some(Rates {
                all: rate,
                successful: throughput,
            }),
            requests,
            success_ratio: scale(success, PERCENT),
            size_per_request: Directions {
                received: bytes_in.mean,
                sent: bytes_out.mean,
            },
            status_codes: status_codes.into_iter().collect(),
        }
    }
}

const PERCENT: u32 = 100;

#[cfg(test)]
mod test_http_vegeta_json {
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
        adapters::bmf_v1::test_util::{
            assert_units, assert_value, assert_variant, convert_v1, value, variants,
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpVegetaJson;

    fn named(suffix: &str) -> AdapterResults {
        let mut results = convert_v1(LEAF, &format!("http/vegeta/{suffix}"));
        results.name_nameless(None);
        results
    }

    /// Kills a leaf that names the benchmark itself, which a prefix would then go in front of,
    /// one that records another leaf, and a skip counted for a key the leaf does not read.
    #[test]
    fn adapter_http_vegeta_json_is_one_nameless_benchmark() {
        for suffix in ["fast.json", "drop.json", "mixed.json"] {
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

    /// Kills `50th` or a percentile misnamed, the latency total read, the request rate read
    /// from `throughput`, the success ratio left a fraction, the requests with no response
    /// written as `responses-0` or left out of `failed-requests`, and a status code left out of
    /// its class.
    #[test]
    fn adapter_http_vegeta_json_measures() {
        let results = named("mixed.json");
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
                        ("mean", 56_441_899.0),
                        ("median", 143_701.0),
                        ("p90", 366_847.0),
                        ("p95", 960_141_569.0),
                        ("p99", 1_000_961_182.0),
                        ("max", 1_001_253_511.0),
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 200.228_729_487_070_98)]),
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
        assert_units(&results, &[]);
    }

    /// Kills the sizes without their direction or swapped, a byte total read for the mean, and
    /// the successful rate written over the rate of every request or without its parameter.
    #[test]
    fn adapter_http_vegeta_json_variants() {
        let results = named("mixed.json");
        for (direction, size) in [("received", 2.1975), ("sent", 0.0)] {
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
            &[(Throughput::SLUG_STR, vec![("mean", 118.373_571_258_113_77)])],
        );
    }

    /// Kills the success fraction scaled in floating point, which gives `28.999999999999996`
    /// for Vegeta's `0.29`.
    #[test]
    fn adapter_http_vegeta_json_scales_in_decimal() {
        assert_eq!(
            value(
                &named("ratio.json"),
                "vegeta",
                "{}",
                SuccessRatio::SLUG_STR,
                "value"
            ),
            Some(29.0)
        );
    }

    /// Kills no 0 for the requests with no response, which Vegeta records only once one fails.
    #[test]
    fn adapter_http_vegeta_json_no_failure_is_0() {
        assert_value(
            &named("fast.json"),
            ("vegeta", "{}"),
            (FailedRequests::SLUG_STR, "value"),
            0.0,
        );
    }

    /// Kills a status code or class filled with 0 when every request failed, and the rate of
    /// successful requests left out when it is 0.
    #[test]
    fn adapter_http_vegeta_json_all_failed() {
        let results = named("drop.json");
        assert_eq!(variants(&results), 4);
        assert_eq!(
            value(&results, "vegeta", "{}", Responses2xx::SLUG_STR, "value"),
            None
        );
        assert_value(
            &results,
            ("vegeta", "{}"),
            (FailedRequests::SLUG_STR, "value"),
            1_000.0,
        );
        assert_value(
            &results,
            ("vegeta", r#"{"@responses":"successful"}"#),
            (Throughput::SLUG_STR, "mean"),
            0.0,
        );
    }

    /// Kills the counts Vegeta writes in place of the rates of an attack whose requests all
    /// started at one instant read as rates.
    #[test]
    fn adapter_http_vegeta_json_one_instant_has_no_rate() {
        let results = named("one_request.json");
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
    fn adapter_http_vegeta_json_informational_and_redirect_classes() {
        let results = named("codes.json");
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

    /// Kills a status code IANA does not assign dropped or given a built-in, one past 599 put in
    /// a class, and a class that leaves out a code IANA does not assign.
    #[test]
    fn adapter_http_vegeta_json_status_codes_iana_does_not_assign() {
        let results = named("unassigned.json");
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

    /// Kills a leaf that reads the first or the last of the reports `-every` writes, to a file
    /// or to a terminal, which clears it before each report but the last.
    #[test]
    fn adapter_http_vegeta_json_every_is_refused() {
        for suffix in ["every_output.json", "every.json"] {
            let input = std::fs::read_to_string(format!("./tool_output/http/vegeta/{suffix}"))
                .expect("Failed to read a fixture");
            assert_eq!(
                LEAF.convert(&input, Settings::new(None, BmfVersion::V1)),
                None,
                "{suffix}"
            );
        }
    }
}
