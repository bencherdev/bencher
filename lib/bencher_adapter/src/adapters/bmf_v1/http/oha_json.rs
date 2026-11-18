use std::collections::BTreeMap;

use bencher_json::project::{
    measure::built_in::{
        BuiltInMeasure as _,
        default::{BytesThroughput, Latency, Throughput},
        http::{DnsDialupTime, DnsLookupTime, SizePerRequest, TimeToFirstByte},
    },
    report::Adapter,
};
use serde::{Deserialize, Deserializer, de::Error as _};

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

/// oha's JSON summary: `--output-format json`, or `-j` before 1.9.0, whose keys oha 1.8.0 and
/// later share (`printer.rs:144-442` at 1.17.0), the first byte's statistics from 1.13.0 on.
pub struct AdapterHttpOhaJson;

impl Adaptable for AdapterHttpOhaJson {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let report = serde_json::from_str::<Report>(input).ok()?;
        let mut results = LeafResults::default();
        report.into_run().insert(&mut results);
        results.finish(Adapter::HttpOhaJson)
    }
}

/// The keys the leaf reads. The histograms and the `metrics` object, a rounded copy, are not.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    summary: Totals,
    latency_percentiles: Percentiles,
    first_byte_percentiles: Option<Percentiles>,
    latency_percentiles_successful: Option<Percentiles>,
    latency_percentiles_not_successful: Option<Percentiles>,
    rps: Windows,
    details: Details,
    status_code_distribution: BTreeMap<u16, f64>,
    error_distribution: BTreeMap<String, f64>,
}

/// The `summary` object, times in seconds.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Totals {
    success_rate: Finite,
    slowest: Finite,
    fastest: Finite,
    average: Finite,
    requests_per_sec: Finite,
    size_per_request: Finite,
    size_per_sec: Finite,
}

/// The request rates of the run's 10 ms windows (`printer.rs:353-394` at 1.17.0).
#[derive(Deserialize)]
struct Windows {
    mean: Finite,
    stddev: Finite,
    max: Finite,
    min: Finite,
    percentiles: Percentiles,
}

#[derive(Deserialize)]
struct Details {
    #[serde(rename = "DNSDialup")]
    dns_dialup: Triple,
    #[serde(rename = "DNSLookup")]
    dns_lookup: Triple,
    #[serde(rename = "firstByte")]
    first_byte: Option<Triple>,
}

/// A time's mean, min, and max, in seconds.
#[derive(Deserialize)]
struct Triple {
    average: Finite,
    fastest: Finite,
    slowest: Finite,
}

impl Report {
    fn into_run(self) -> Run {
        let Self {
            summary,
            latency_percentiles,
            first_byte_percentiles,
            latency_percentiles_successful,
            latency_percentiles_not_successful,
            rps,
            details,
            status_code_distribution,
            error_distribution,
        } = self;
        let mut latency = Triple {
            average: summary.average,
            fastest: summary.fastest,
            slowest: summary.slowest,
        }
        .metrics();
        latency.extend(latency_percentiles.nanos());
        let mut measures = vec![(Latency::name_id(), latency)];
        if let Some(first_byte) = details.first_byte {
            let mut metrics = first_byte.metrics();
            metrics.extend(
                first_byte_percentiles
                    .map(Percentiles::nanos)
                    .unwrap_or_default(),
            );
            measures.push((TimeToFirstByte::name_id(), metrics));
        }
        measures.push((DnsDialupTime::name_id(), details.dns_dialup.metrics()));
        measures.push((DnsLookupTime::name_id(), details.dns_lookup.metrics()));
        measures.push((Throughput::name_id(), rps.metrics(summary.requests_per_sec)));
        Run {
            measures,
            received: vec![
                (
                    BytesThroughput::name_id(),
                    vec![(MEAN.to_owned(), summary.size_per_sec)],
                ),
                (
                    SizePerRequest::name_id(),
                    vec![(MEAN.to_owned(), summary.size_per_request)],
                ),
            ],
            breakdown: latency_percentiles_successful
                .zip(latency_percentiles_not_successful)
                .map(|(successful, not_successful)| (successful.nanos(), not_successful.nanos())),
            success_ratio: summary.success_rate.map(|rate| scale(rate, PERCENT)),
            status_codes: status_code_distribution.into_iter().collect(),
            errors: error_distribution.into_iter().collect(),
        }
    }
}

const MEAN: &str = "mean";
const NANOS_PER_SECOND: u32 = 1_000_000_000;
const PERCENT: u32 = 100;

impl Triple {
    fn metrics(self) -> Metrics {
        let Self {
            average,
            fastest,
            slowest,
        } = self;
        [(MEAN, average), ("min", fastest), ("max", slowest)]
            .into_iter()
            .map(|(metric, seconds)| (metric.to_owned(), nanos(seconds)))
            .collect()
    }
}

impl Windows {
    /// The run's rate as `mean`, and the windows' statistics each with `window_` in front.
    fn metrics(self, run_rate: Finite) -> Metrics {
        let Self {
            mean,
            stddev,
            max,
            min,
            percentiles: Percentiles(percentiles),
        } = self;
        let windows = [
            (MEAN.to_owned(), mean),
            ("std_dev".to_owned(), stddev),
            ("max".to_owned(), max),
            ("min".to_owned(), min),
        ]
        .into_iter()
        .chain(percentiles)
        .map(|(metric, rate)| (format!("window_{metric}"), rate));
        [(MEAN.to_owned(), run_rate)]
            .into_iter()
            .chain(windows)
            .collect()
    }
}

/// oha's percentiles (`p10` to `p99.99`), each by its metric name.
struct Percentiles(Metrics);

impl Percentiles {
    fn nanos(self) -> Metrics {
        self.0
            .into_iter()
            .map(|(metric, seconds)| (metric, nanos(seconds)))
            .collect()
    }
}

impl<'de> Deserialize<'de> for Percentiles {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        BTreeMap::<String, Finite>::deserialize(deserializer)?
            .into_iter()
            .map(|(key, value)| {
                let metric = key
                    .strip_prefix('p')
                    .and_then(percentile_name)
                    .ok_or_else(|| D::Error::custom(format!("not a percentile: {key}")))?;
                Ok((metric, value))
            })
            .collect::<Result<_, _>>()
            .map(Self)
    }
}

fn nanos(seconds: Finite) -> Finite {
    seconds.map(|seconds| scale(seconds, NANOS_PER_SECOND))
}

#[cfg(test)]
mod test_http_oha_json {
    use bencher_json::project::{
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
    };
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::bmf_v1::test_util::{
            Expected, assert_units, assert_value, assert_variant, convert_v1, value, variants,
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpOhaJson;

    fn named(suffix: &str) -> AdapterResults {
        let mut results = convert_v1(LEAF, &format!("http/oha/{suffix}"));
        results.name_nameless(None);
        results
    }

    /// The nine percentiles oha prints, in order from `p10` to `p99_99`.
    fn percentiles(measure: &'static str, values: [f64; 9]) -> Expected {
        (
            measure,
            [
                "p10", "p25", "median", "p75", "p90", "p95", "p99", "p99_9", "p99_99",
            ]
            .into_iter()
            .zip(values)
            .collect(),
        )
    }

    /// A measure's percentiles and its mean, min, and max.
    fn statistics(measure: &'static str, [mean, min, max]: [f64; 3], values: [f64; 9]) -> Expected {
        let (measure, mut metrics) = percentiles(measure, values);
        metrics.extend([("mean", mean), ("min", min), ("max", max)]);
        (measure, metrics)
    }

    /// Kills a leaf that names the benchmark itself, which a prefix would then go in front of,
    /// one that records another leaf, and a skip counted for a `null` statistic, a histogram,
    /// or the `metrics` copy.
    #[test]
    fn adapter_http_oha_json_is_one_nameless_benchmark() {
        for suffix in ["fast.json", "drop.json", "mixed_1_8_0.json"] {
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

    /// Kills a time left in seconds, `p50` or a decimal percentile misnamed, a statistic of the
    /// windows without its `window_` prefix or written over the run's rate, the success ratio
    /// left a fraction, a timeout left out of the failed requests, and a status code left out of
    /// its class.
    #[test]
    fn adapter_http_oha_json_measures() {
        let results = named("mixed_timeout_breakdown.json");
        assert_eq!(variants(&results), 4);
        assert_variant(
            &results,
            "oha",
            "{}",
            &[
                statistics(
                    Latency::SLUG_STR,
                    [58_442.181_985_294_16, 10_877.0, 471_480.0],
                    [
                        22_765.0, 28_505.0, 36_588.0, 59_853.0, 111_976.0, 206_674.0, 324_739.0,
                        471_480.0, 471_480.0,
                    ],
                ),
                statistics(
                    TimeToFirstByte::SLUG_STR,
                    [58_825.637_019_230_74, 10_827.0, 470_899.0],
                    [
                        23_587.0, 28_956.0, 37_459.0, 60_495.0, 110_272.0, 206_584.0, 302_275.0,
                        470_899.0, 470_899.0,
                    ],
                ),
                (
                    DnsDialupTime::SLUG_STR,
                    vec![("mean", 67_480.54), ("min", 14_312.0), ("max", 274_049.0)],
                ),
                (
                    DnsLookupTime::SLUG_STR,
                    vec![
                        ("mean", 13_517.199_999_999_998),
                        ("min", 1_031.0),
                        ("max", 45_191.0),
                    ],
                ),
                (
                    Throughput::SLUG_STR,
                    vec![
                        ("mean", 299.337_665_276_516_76),
                        ("window_mean", 62_804.522_165_956_37),
                        ("window_std_dev", 54_461.059_803_716_53),
                        ("window_max", 96_998.316_421_181_31),
                        ("window_min", 1.002_315_072_180_091),
                        ("window_p10", 1.002_315_072_180_091),
                        ("window_p25", 1.002_315_072_180_091),
                        ("window_median", 91_414.247_761_615_62),
                        ("window_p75", 96_998.316_421_181_31),
                        ("window_p90", 96_998.316_421_181_31),
                        ("window_p95", 96_998.316_421_181_31),
                        ("window_p99", 96_998.316_421_181_31),
                        ("window_p99_9", 96_998.316_421_181_31),
                        ("window_p99_99", 96_998.316_421_181_31),
                    ],
                ),
                (
                    SuccessRatio::SLUG_STR,
                    vec![("value", 90.666_666_666_666_66)],
                ),
                (Timeouts::SLUG_STR, vec![("value", 27.0)]),
                (FailedRequests::SLUG_STR, vec![("value", 56.0)]),
                ("responses-200", vec![("value", 416.0)]),
                ("responses-404", vec![("value", 69.0)]),
                ("responses-500", vec![("value", 59.0)]),
                (Responses2xx::SLUG_STR, vec![("value", 416.0)]),
                (Responses4xx::SLUG_STR, vec![("value", 69.0)]),
                (Responses5xx::SLUG_STR, vec![("value", 59.0)]),
            ],
        );
        assert_units(&results, &[]);
    }

    /// Kills the byte rate or the size without their direction, and the two halves of the success
    /// breakdown swapped or read from the whole run.
    #[test]
    fn adapter_http_oha_json_variants() {
        let results = named("mixed_timeout_breakdown.json");
        assert_variant(
            &results,
            "oha",
            r#"{"@direction":"received"}"#,
            &[
                (
                    BytesThroughput::SLUG_STR,
                    vec![("mean", 622.622_343_775_154_8)],
                ),
                (SizePerRequest::SLUG_STR, vec![("mean", 2.0)]),
            ],
        );
        assert_variant(
            &results,
            "oha",
            r#"{"@responses":"successful"}"#,
            &[percentiles(
                Latency::SLUG_STR,
                [
                    23_636.0, 29_026.0, 37_600.0, 60_716.0, 110_333.0, 206_674.0, 302_685.0,
                    471_480.0, 471_480.0,
                ],
            )],
        );
        assert_variant(
            &results,
            "oha",
            r#"{"@responses":"not_successful"}"#,
            &[percentiles(
                Latency::SLUG_STR,
                [
                    20_904.0, 25_992.0, 33_363.0, 57_150.0, 119_638.0, 228_379.0, 324_739.0,
                    378_203.0, 378_203.0,
                ],
            )],
        );
    }

    /// Kills seconds and fractions scaled in floating point, which gives `58442.181985294155`
    /// for oha's `0.00005844218198529416` s.
    #[test]
    fn adapter_http_oha_json_scales_in_decimal() {
        let results = named("mixed_timeout_breakdown.json");
        assert_eq!(
            value(&results, "oha", "{}", Latency::SLUG_STR, "mean"),
            Some(58_442.181_985_294_16)
        );
    }

    /// Kills a request at the end of a `-z` run counted as failed, which oha's own success rate
    /// leaves out, and no 0 for the timeouts oha prints only once one times out.
    #[test]
    fn adapter_http_oha_json_deadline_is_no_failure() {
        let results = named("post_rate.json");
        for (measure, expected) in [
            (FailedRequests::SLUG_STR, 0.0),
            (Timeouts::SLUG_STR, 0.0),
            (SuccessRatio::SLUG_STR, 100.0),
            ("responses-201", 1_500.0),
            (Responses2xx::SLUG_STR, 1_500.0),
        ] {
            assert_value(&results, ("oha", "{}"), (measure, "value"), expected);
        }
    }

    /// Kills a `null` statistic of a run with no response written as 0 or counted as skipped,
    /// a status code or class filled with 0, and a variant built with no metric.
    #[test]
    fn adapter_http_oha_json_all_errors() {
        let results = named("drop.json");
        assert_eq!(variants(&results), 2);
        assert_variant(
            &results,
            "oha",
            "{}",
            &[
                (Throughput::SLUG_STR, vec![("mean", 68_221.712_378_625_04)]),
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

    /// Kills the first byte's statistics required, which oha writes only from 1.13.0, and the
    /// success breakdown of `-j` before 1.9.0 left unread.
    #[test]
    fn adapter_http_oha_json_before_the_first_byte() {
        for (suffix, variants_expected) in [
            ("mixed_1_8_0.json", 2),
            ("mixed_1_12_0.json", 2),
            ("mixed_breakdown_1_8_0.json", 4),
        ] {
            let results = named(suffix);
            assert_eq!(variants(&results), variants_expected, "{suffix}");
            assert_eq!(
                value(&results, "oha", "{}", TimeToFirstByte::SLUG_STR, "mean"),
                None,
                "{suffix}"
            );
        }
        let results = named("mixed_breakdown_1_8_0.json");
        assert_value(
            &results,
            ("oha", r#"{"@responses":"not_successful"}"#),
            (Latency::SLUG_STR, "p99_99"),
            929_648.0,
        );
        assert_value(&results, ("oha", "{}"), (Timeouts::SLUG_STR, "value"), 35.0);
    }

    /// Kills a 1xx code counted in the 3xx class, a 3xx code counted in the 1xx class, and the
    /// two classes swapped.
    #[test]
    fn adapter_http_oha_json_informational_and_redirect_classes() {
        let results = named("codes_1xx_3xx.json");
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
    fn adapter_http_oha_json_status_codes_iana_does_not_assign() {
        let results = named("codes_599_999.json");
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

    /// Kills a variant built for a half of the success breakdown with no response, which oha
    /// prints as `null` when every response is 2xx.
    #[test]
    fn adapter_http_oha_json_breakdown_with_no_other_responses() {
        let results = named("fast_breakdown.json");
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
