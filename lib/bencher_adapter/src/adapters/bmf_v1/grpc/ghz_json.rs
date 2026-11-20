use std::{collections::BTreeMap, fmt};

use bencher_json::{BenchmarkName, project::report::Adapter};
use serde::{
    Deserialize, Deserializer,
    de::{Error as _, IgnoredAny, MapAccess, Visitor},
};
use serde_json::{Map, Value};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        grpc::ghz::{Distribution, Run},
        leaf::{LeafResults, parameter_set},
        percentile::percentile_name,
    },
    results::adapter_results::AdapterResults,
};

/// ghz's JSON report, `-O json` or `-O pretty`, as ghz v0.80.0 and later write it
/// (`runner/reporter.go:27-101` at v0.121.0). Its `details`, one entry per call, are not read.
pub struct AdapterGrpcGhzJson;

impl Adaptable for AdapterGrpcGhzJson {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let report = serde_json::from_str::<Report>(input).ok()?;
        let mut results = LeafResults::default();
        if let Some(run) = report.into_run(&mut results) {
            run.insert(&mut results);
        }
        results.finish_with_skips(Adapter::GrpcGhzJson)
    }
}

/// The keys the leaf reads, times in nanoseconds.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    name: Option<String>,
    options: Options,
    count: f64,
    average: f64,
    fastest: f64,
    slowest: f64,
    rps: f64,
    status_code_distribution: BTreeMap<String, f64>,
    latency_distribution: LatencyDistribution,
    tags: Option<BTreeMap<String, String>>,
}

/// The percentiles, `null` when ghz counted no call in them.
#[derive(Deserialize)]
struct LatencyDistribution(Option<Vec<Percentile>>);

#[derive(Deserialize)]
struct Percentile {
    percentage: u32,
    latency: f64,
}

impl Report {
    /// The run, or `None`, counted as skipped, when its name is not a benchmark name, its
    /// parameters pass the key cap, or a user's tag is spelled like a key the leaf writes.
    fn into_run(self, results: &mut LeafResults) -> Option<Run> {
        let Self {
            name,
            options,
            count,
            average,
            fastest,
            slowest,
            rps,
            status_code_distribution,
            latency_distribution: LatencyDistribution(percentiles),
            tags,
        } = self;
        let Ok(benchmark) = name
            .unwrap_or_else(|| format!("{}/{}", options.host, options.call))
            .parse::<BenchmarkName>()
        else {
            results.skip_benchmark();
            return None;
        };
        let mut parameters = options.parameters();
        for (key, value) in tags.unwrap_or_default() {
            if key
                .strip_prefix('@')
                .is_some_and(|key| allowed(key).is_some())
            {
                results.skip_variant();
                return None;
            }
            parameters.insert(key, value.into());
        }
        let Some(parameters) = parameter_set(parameters) else {
            results.skip_variant();
            return None;
        };
        Some(Run {
            benchmark: Some(benchmark),
            parameters,
            count,
            average,
            distribution: percentiles.map(|percentiles| Distribution {
                fastest,
                slowest,
                percentiles: percentiles
                    .into_iter()
                    // ghz leaves a percentile it has no latency for at 0 % (`reporter.go:305-311`).
                    .filter(|percentile| percentile.percentage > 0)
                    .filter_map(|Percentile { percentage, latency }| {
                        Some((percentile_name(&percentage.to_string())?, latency))
                    })
                    .collect(),
            }),
            rps,
            statuses: status_code_distribution.into_iter().collect(),
        })
    }
}

/// ghz's settings that a variant carries, each as ghz prints it, by ghz's key. ghz prints most of
/// them only when they are set.
const SETTINGS: [&str; 9] = [
    "concurrency",
    "connections",
    "timeout",
    "total",
    "duration",
    "rps",
    "async",
    "skipFirst",
    "count-errors",
];
/// The load schedule and its parameters, carried only when the schedule is not constant.
const LOAD_SCHEDULE: [&str; 6] = [
    "load-schedule",
    "load-start",
    "load-end",
    "load-step",
    "load-step-duration",
    "load-max-duration",
];
/// The concurrency schedule and its parameters, carried only when the schedule is not constant.
const CONCURRENCY_SCHEDULE: [&str; 6] = [
    "concurrency-schedule",
    "concurrency-start",
    "concurrency-end",
    "concurrency-step",
    "concurrency-step-duration",
    "concurrency-max-duration",
];
const CONSTANT: &str = "const";

/// The run's target and the settings the leaf may write. Every other option, the request's `data`
/// and `metadata` among them, is skipped unread.
struct Options {
    call: String,
    host: String,
    settings: BTreeMap<&'static str, Value>,
}

impl Options {
    /// The settings as `@` parameters: `duration` in place of `total`, which a `-z` run prints as
    /// the largest 32-bit integer (`runner/options.go:178-180`), and a schedule's keys only when
    /// it is not constant.
    fn parameters(self) -> Map<String, Value> {
        let Self { mut settings, .. } = self;
        if settings.contains_key("duration") {
            settings.remove("total");
        }
        for keys in [LOAD_SCHEDULE, CONCURRENCY_SCHEDULE] {
            if keys
                .first()
                .and_then(|schedule| settings.get(schedule))
                .is_none_or(|schedule| schedule == CONSTANT)
            {
                for key in keys {
                    settings.remove(key);
                }
            }
        }
        settings
            .into_iter()
            .map(|(key, value)| (format!("@{key}"), value))
            .collect()
    }
}

/// ghz's key for `key`, if the leaf may write it.
fn allowed(key: &str) -> Option<&'static str> {
    SETTINGS
        .iter()
        .chain(&LOAD_SCHEDULE)
        .chain(&CONCURRENCY_SCHEDULE)
        .find(|allowed| **allowed == key)
        .copied()
}

impl<'de> Deserialize<'de> for Options {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(OptionsVisitor)
    }
}

struct OptionsVisitor;

impl<'de> Visitor<'de> for OptionsVisitor {
    type Value = Options;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ghz's options")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let (mut call, mut host, mut settings) = (None, None, BTreeMap::new());
        while let Some(key) = map.next_key::<String>()? {
            if key == "call" {
                call = Some(map.next_value()?);
            } else if key == "host" {
                host = Some(map.next_value()?);
            } else if let Some(key) = allowed(&key) {
                settings.insert(key, map.next_value()?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(Options {
            call: call.ok_or_else(|| A::Error::missing_field("call"))?,
            host: host.ok_or_else(|| A::Error::missing_field("host"))?,
            settings,
        })
    }
}

#[cfg(test)]
mod test_grpc_ghz_json {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Latency, Throughput},
                grpc::{
                    StatusAborted, StatusAlreadyExists, StatusCancelled, StatusDataLoss,
                    StatusDeadlineExceeded, StatusFailedPrecondition, StatusInternal,
                    StatusInvalidArgument, StatusNotFound, StatusOk, StatusOutOfRange,
                    StatusPermissionDenied, StatusResourceExhausted, StatusUnauthenticated,
                    StatusUnavailable, StatusUnimplemented, StatusUnknown,
                },
                http::{FailedRequests, Requests},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::{
            grpc::ghz_json::AdapterGrpcGhzJson,
            test_util::{
                Expected, assert_units, assert_value, assert_variant, convert_edited_v1,
                convert_v1, value, variants,
            },
        },
        results::adapter_results::Skipped,
    };

    const LEAF: Adapter = Adapter::GrpcGhzJson;
    const CALL: &str = "localhost:50051/helloworld.Greeter.SayHello";
    /// The settings every capture of 2,000 calls at a concurrency of 10 carries.
    const OK_RUN: &str =
        r#"{"@concurrency":10,"@connections":1,"@timeout":20000000000,"@total":2000}"#;

    fn fixture(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/grpc/ghz/{suffix}"))
            .expect("Failed to read a fixture")
    }

    /// `latency` with ghz's mean, min, max, and the seven percentiles it prints.
    fn latency([mean, min, max]: [f64; 3], percentiles: [f64; 7]) -> Expected {
        let mut metrics = vec![("mean", mean), ("min", min), ("max", max)];
        metrics.extend(
            ["p10", "p25", "median", "p75", "p90", "p95", "p99"]
                .into_iter()
                .zip(percentiles),
        );
        (Latency::SLUG_STR, metrics)
    }

    fn count(measure: &'static str, value: f64) -> Expected {
        (measure, vec![("value", value)])
    }

    /// Kills a leaf that names the run other than by its host and call, writes a setting outside
    /// the allowlist (the dial timeout, `CPUs`, the proto paths, a constant schedule's keys) or
    /// leaves one out, swaps `fastest` and `slowest`, misnames a percentile, writes
    /// `failed-requests` other than 0 for a run with no error, records another leaf, or counts a
    /// skip for `details`, `histogram`, or `errorDistribution`.
    #[test]
    fn adapter_grpc_ghz_json_ok() {
        let results = convert_v1(LEAF, "grpc/ghz/ok.json");
        assert_eq!(results.resolved_adapter, Some(LEAF));
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(variants(&results), 1);
        assert_variant(
            &results,
            CALL,
            OK_RUN,
            &[
                latency(
                    [488_810.0, 52_089.0, 3_004_499.0],
                    [
                        174_638.0,
                        261_411.0,
                        412_752.0,
                        639_642.0,
                        872_067.0,
                        1_052_165.0,
                        1_478_608.0,
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 11_588.216_123_799_879)]),
                count(Requests::SLUG_STR, 2_000.0),
                count(FailedRequests::SLUG_STR, 0.0),
                count(StatusOk::SLUG_STR, 2_000.0),
            ],
        );
        assert_units(&results, &[]);
    }

    /// Kills a report without `details`, as `jq -c 'del(.details)'` leaves a run past the 4 MiB
    /// body limit, refused or read in part; and reads a report of v0.80.0, the oldest the leaf
    /// takes, as one of v0.121.0.
    #[test]
    fn adapter_grpc_ghz_json_old_version_and_no_details() {
        for (suffix, run, expected, rate, calls) in [
            (
                "ok_0_80_0.json",
                OK_RUN,
                latency(
                    [196_869.0, 53_363.0, 1_022_398.0],
                    [
                        100_245.0, 126_556.0, 161_801.0, 221_041.0, 344_282.0, 441_353.0, 680_030.0,
                    ],
                ),
                44_126.712_502_553_83,
                2_000.0,
            ),
            (
                "large_200k_no_details.json",
                r#"{"@concurrency":50,"@connections":1,"@timeout":20000000000,"@total":200000}"#,
                latency(
                    [741_297.0, 66_021.0, 3_289_885.0],
                    [
                        353_075.0,
                        497_416.0,
                        711_676.0,
                        933_909.0,
                        1_162_559.0,
                        1_310_077.0,
                        1_620_727.0,
                    ],
                ),
                50_770.243_371_190_22,
                200_000.0,
            ),
        ] {
            let results = convert_v1(LEAF, &format!("grpc/ghz/{suffix}"));
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
            assert_variant(
                &results,
                CALL,
                run,
                &[
                    expected,
                    (Throughput::SLUG_STR, vec![("mean", rate)]),
                    count(Requests::SLUG_STR, calls),
                    count(FailedRequests::SLUG_STR, 0.0),
                    count(StatusOk::SLUG_STR, calls),
                ],
            );
        }
    }

    /// Kills `--name` ignored for the host and call, and a user's tag dropped, renamed with `@`,
    /// or written as other than its string.
    #[test]
    fn adapter_grpc_ghz_json_name_and_tags() {
        let results = convert_v1(LEAF, "grpc/ghz/name_tags.json");
        assert_eq!(variants(&results), 1);
        assert_value(
            &results,
            (
                "greeter ok",
                r#"{"@concurrency":10,"@connections":4,"@timeout":20000000000,"@total":2000,"env":"ci","team":"api"}"#,
            ),
            (Latency::SLUG_STR, "median"),
            205_336.0,
        );
    }

    /// Kills a leaf that copies the request's `metadata` or `data` into a parameter or a name: a
    /// run whose metadata holds a placeholder secret reads exactly as the same run with other
    /// metadata and data, and with none.
    #[test]
    fn adapter_grpc_ghz_json_never_reads_metadata_or_data() {
        let results = convert_v1(LEAF, "grpc/ghz/metadata_secret.json");
        assert_eq!(variants(&results), 1);
        assert!(value(&results, CALL, OK_RUN, Latency::SLUG_STR, "mean").is_some());
        for edit in [
            |json: &mut serde_json::Value| {
                json["options"]["metadata"] = serde_json::json!({"authorization": "Bearer OTHER"});
                json["options"]["data"] = serde_json::json!({"name": "other"});
            },
            |json: &mut serde_json::Value| {
                if let Some(options) = json["options"].as_object_mut() {
                    options.remove("metadata");
                    options.remove("data");
                }
            },
        ] {
            assert_eq!(
                convert_edited_v1(LEAF, "grpc/ghz/metadata_secret.json", edit),
                results
            );
        }
    }

    /// Kills `average` left out or taken from the OK calls alone, an OK status counted as failed,
    /// a status other than OK left out of the failed requests, and a status mapped to another's
    /// measure.
    #[test]
    fn adapter_grpc_ghz_json_mixed_statuses() {
        let results = convert_v1(LEAF, "grpc/ghz/mixed_timeout.json");
        assert_variant(
            &results,
            CALL,
            r#"{"@concurrency":10,"@connections":1,"@timeout":1000000000,"@total":2000}"#,
            &[
                latency(
                    [42_181_441.0, 36_286.0, 1_364_164.0],
                    [
                        81_314.0, 103_350.0, 138_653.0, 209_873.0, 323_467.0, 414_477.0, 791_425.0,
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 220.611_033_708_761_68)]),
                count(Requests::SLUG_STR, 2_000.0),
                count(FailedRequests::SLUG_STR, 610.0),
                count(StatusOk::SLUG_STR, 1_390.0),
                count(StatusDeadlineExceeded::SLUG_STR, 84.0),
                count(StatusInternal::SLUG_STR, 208.0),
                count(StatusNotFound::SLUG_STR, 207.0),
                count(StatusResourceExhausted::SLUG_STR, 111.0),
            ],
        );
    }

    /// Kills ghz's placeholder `fastest` and `slowest` of 0 written, or counted as skipped, for a
    /// run with no OK call, and its mean, rate, and counts left out with them; and, with
    /// `--count-errors`, its real `fastest` and `slowest` over failed calls dropped as if they
    /// were placeholders.
    #[test]
    fn adapter_grpc_ghz_json_no_ok_call() {
        for (suffix, benchmark, run, [mean, rate, calls]) in [
            (
                "unavailable.json",
                CALL,
                r#"{"@concurrency":10,"@connections":1,"@timeout":20000000000,"@total":2000}"#,
                [255_501.0, 15_670.533_059_900_29, 2_000.0],
            ),
            (
                "refused.json",
                "localhost:9/helloworld.Greeter.SayHello",
                r#"{"@concurrency":10,"@connections":1,"@timeout":20000000000,"@total":200}"#,
                [26_350.0, 14_448.432_532_899_802, 200.0],
            ),
        ] {
            let results = convert_v1(LEAF, &format!("grpc/ghz/{suffix}"));
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
            assert_variant(
                &results,
                benchmark,
                run,
                &[
                    (Latency::SLUG_STR, vec![("mean", mean)]),
                    (Throughput::SLUG_STR, vec![("mean", rate)]),
                    count(Requests::SLUG_STR, calls),
                    count(FailedRequests::SLUG_STR, calls),
                    count(StatusUnavailable::SLUG_STR, calls),
                ],
            );
        }
        let results = convert_v1(LEAF, "grpc/ghz/unavailable_count_errors.json");
        let run = r#"{"@concurrency":10,"@connections":1,"@count-errors":true,"@timeout":20000000000,"@total":200}"#;
        assert_value(&results, (CALL, run), (Latency::SLUG_STR, "min"), 44_168.0);
        assert_value(
            &results,
            (CALL, run),
            (Latency::SLUG_STR, "max"),
            1_702_630.0,
        );
        assert_value(&results, (CALL, run), (Latency::SLUG_STR, "p99"), 853_458.0);
    }

    /// Kills ghz's placeholder `average` of 0 written as a mean for a run that counted no call,
    /// and its rate and counts left out with it.
    #[test]
    fn adapter_grpc_ghz_json_no_call_counted() {
        let results = convert_v1(LEAF, "grpc/ghz/skip_all.json");
        assert_eq!(results.skipped, Skipped::default());
        assert_variant(
            &results,
            CALL,
            r#"{"@concurrency":10,"@connections":1,"@skipFirst":100,"@timeout":20000000000,"@total":100}"#,
            &[
                (Throughput::SLUG_STR, vec![("mean", 0.0)]),
                count(Requests::SLUG_STR, 0.0),
                count(FailedRequests::SLUG_STR, 0.0),
            ],
        );
    }

    /// Kills any status Go prints mapped to another code's measure, Go's `Canceled` not mapped to
    /// the spec's `cancelled`, and a code gRPC does not register dropped, given a built-in, or
    /// named at run time other than by its number.
    #[test]
    fn adapter_grpc_ghz_json_every_status_code() {
        let results = convert_v1(LEAF, "grpc/ghz/codes.json");
        let run = r#"{"@concurrency":1,"@connections":1,"@timeout":20000000000,"@total":180}"#;
        for measure in [
            StatusOk::SLUG_STR,
            StatusCancelled::SLUG_STR,
            StatusUnknown::SLUG_STR,
            StatusInvalidArgument::SLUG_STR,
            StatusDeadlineExceeded::SLUG_STR,
            StatusNotFound::SLUG_STR,
            StatusAlreadyExists::SLUG_STR,
            StatusPermissionDenied::SLUG_STR,
            StatusResourceExhausted::SLUG_STR,
            StatusFailedPrecondition::SLUG_STR,
            StatusAborted::SLUG_STR,
            StatusOutOfRange::SLUG_STR,
            StatusUnimplemented::SLUG_STR,
            StatusInternal::SLUG_STR,
            StatusUnavailable::SLUG_STR,
            StatusDataLoss::SLUG_STR,
            StatusUnauthenticated::SLUG_STR,
            "grpc-status-17",
        ] {
            assert_value(&results, (CALL, run), (measure, "value"), 10.0);
        }
        assert_value(
            &results,
            (CALL, run),
            (FailedRequests::SLUG_STR, "value"),
            170.0,
        );
        assert_units(&results, &[("grpc-status-17", "responses")]);
    }

    /// Kills a status ghz records as empty, for an error that carries no gRPC status, left out of
    /// the failed requests, written as a measure, or dropped uncounted. No capture holds one, so
    /// the test empties a status of a real capture, as `runner/stats_handler.go:46-50` records it.
    #[test]
    fn adapter_grpc_ghz_json_empty_status() {
        let results = convert_edited_v1(LEAF, "grpc/ghz/mixed_timeout.json", |json| {
            let statuses = &mut json["statusCodeDistribution"];
            if let Some(statuses) = statuses.as_object_mut()
                && let Some(calls) = statuses.remove("Internal")
            {
                statuses.insert(String::new(), calls);
            }
        });
        let run = r#"{"@concurrency":10,"@connections":1,"@timeout":1000000000,"@total":2000}"#;
        assert_eq!(
            results.skipped,
            Skipped {
                measures: 1,
                ..Skipped::default()
            }
        );
        assert_value(
            &results,
            (CALL, run),
            (FailedRequests::SLUG_STR, "value"),
            610.0,
        );
        assert_eq!(
            value(&results, CALL, run, StatusInternal::SLUG_STR, "value"),
            None
        );
        assert_units(&results, &[]);
    }

    /// Kills `@total` written beside `@duration` for a `-z` run, which prints the largest 32-bit
    /// integer as its total; `@rps`, `@async`, or `@skipFirst` left out when printed; a schedule's
    /// keys left out when it is not constant; and `@timeout` written for `-t 0`, which ghz omits.
    #[test]
    fn adapter_grpc_ghz_json_settings() {
        for (suffix, run) in [
            (
                "rps.json",
                r#"{"@concurrency":10,"@connections":1,"@duration":3000000000,"@rps":200,"@timeout":20000000000}"#,
            ),
            (
                "async.json",
                r#"{"@async":true,"@concurrency":10,"@connections":1,"@rps":200,"@timeout":20000000000,"@total":400}"#,
            ),
            (
                "skip_first.json",
                r#"{"@concurrency":10,"@connections":1,"@skipFirst":100,"@timeout":20000000000,"@total":2000}"#,
            ),
            (
                "mixed_count_errors.json",
                r#"{"@concurrency":10,"@connections":1,"@count-errors":true,"@timeout":1000000000,"@total":2000}"#,
            ),
            (
                "no_timeout.json",
                r#"{"@concurrency":10,"@connections":1,"@total":200}"#,
            ),
            (
                "step.json",
                r#"{"@concurrency":10,"@connections":1,"@load-end":0,"@load-max-duration":4000000000,"@load-schedule":"step","@load-start":50,"@load-step":50,"@load-step-duration":1000000000,"@timeout":20000000000,"@total":200}"#,
            ),
            (
                "concurrency_step.json",
                r#"{"@concurrency":50,"@concurrency-end":10,"@concurrency-max-duration":2000000000,"@concurrency-schedule":"step","@concurrency-start":2,"@concurrency-step":2,"@concurrency-step-duration":1000000000,"@connections":1,"@rps":200,"@timeout":20000000000,"@total":1000}"#,
            ),
        ] {
            let results = convert_v1(LEAF, &format!("grpc/ghz/{suffix}"));
            assert_eq!(variants(&results), 1, "{suffix}");
            assert!(
                value(&results, CALL, run, Requests::SLUG_STR, "value").is_some(),
                "{suffix}"
            );
        }
    }

    /// Kills a run with both schedules and a tag, 17 keys, written past the 16-key cap or dropped
    /// uncounted, and a user's tag spelled like a setting key the leaf writes stored over it or
    /// beside it.
    #[test]
    fn adapter_grpc_ghz_json_skipped_runs() {
        for suffix in ["schedules_tags.json", "tag_clash.json"] {
            let results = convert_v1(LEAF, &format!("grpc/ghz/{suffix}"));
            assert!(results.inner.is_empty(), "{suffix}");
            assert_eq!(
                results.skipped,
                Skipped {
                    variants: 1,
                    ..Skipped::default()
                },
                "{suffix}"
            );
        }
    }

    /// Kills a percentile slot ghz leaves at 0 % for a latency of 0 (`runner/reporter.go:305-311`)
    /// read as `min` over the run's `fastest`, or counted as skipped. No capture holds one, so the
    /// test sets a slot of a real capture to it.
    #[test]
    fn adapter_grpc_ghz_json_unset_percentile_slot() {
        let results = convert_edited_v1(LEAF, "grpc/ghz/ok.json", |json| {
            json["latencyDistribution"][0] = serde_json::json!({"percentage": 0, "latency": 0});
        });
        assert_eq!(results.skipped, Skipped::default());
        assert_value(
            &results,
            (CALL, OK_RUN),
            (Latency::SLUG_STR, "min"),
            52_089.0,
        );
        assert_eq!(
            value(&results, CALL, OK_RUN, Latency::SLUG_STR, "p10"),
            None
        );
    }

    /// Kills a leaf that reads the first of two reports and drops the second, as a wrapper that
    /// runs ghz once per call into one output leaves them. No capture holds two, so the test
    /// joins two real captures.
    #[test]
    fn adapter_grpc_ghz_json_refuses_two_reports() {
        let input = format!("{}\n{}", fixture("ok.json"), fixture("rps.json"));
        assert_eq!(
            AdapterGrpcGhzJson::parse(&input, Settings::new(None, BmfVersion::V1)),
            None
        );
    }
}
