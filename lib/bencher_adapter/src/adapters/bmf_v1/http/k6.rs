use bencher_json::{
    MeasureNameId, ParameterSet, Slug,
    project::measure::built_in::{
        BuiltInMeasure as _,
        default::{BytesThroughput, Latency, Throughput},
        http::{
            BlockedTime, CheckFailures, CheckPasses, CheckRatio, ConnectingTime, DroppedIterations,
            FailedRequests, FailureRatio, GroupTime, IterationThroughput, IterationTime,
            Iterations, ReceivingTime, Requests, Responses, SendingTime, TlsHandshakingTime,
            WaitingTime,
        },
    },
};
use bencher_valid::{BYTES, NANOSECONDS, PERCENTAGE};
use rust_decimal::{Decimal, prelude::ToPrimitive as _};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::Finite,
        http::{k6_json::AdapterHttpK6Json, k6_json_summary::AdapterHttpK6JsonSummary},
        leaf::{LeafResults, Variant, parameter_set},
        percentile::percentile_name,
    },
    results::adapter_results::AdapterResults,
};

/// k6 at version 1: its machine-readable summary, then its end-of-test summary data.
///
/// k6 names nothing for the test as a whole, so every metric belongs to one benchmark that the
/// server names by the run's benchmark prefix, else `k6`, and each sub-metric is a variant of it.
pub struct AdapterHttpK6;

impl Adaptable for AdapterHttpK6 {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterHttpK6Json::parse(input, settings)
            .or_else(|| AdapterHttpK6JsonSummary::parse(input, settings))
    }
}

/// One k6 metric or sub-metric, `metric{key:value,...}`, and its values.
pub(super) struct Metric<'a> {
    pub name: &'a str,
    /// What the values hold, which the legacy export does not say.
    pub contains: Option<Contains>,
    pub values: Values,
}

/// What a metric's values hold: plain numbers, times in milliseconds, or bytes.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Contains {
    Default,
    Time,
    Data,
}

/// A metric's k6 type, which the legacy export does not give.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Kind {
    Counter,
    Gauge,
    Rate,
    Trend,
}

/// A metric's values by its k6 type.
pub(super) enum Values {
    /// Each statistic by its `summaryTrendStats` column, such as `avg`, `med`, or `p(95)`.
    Trend(Vec<(String, Finite)>),
    Counter {
        count: Finite,
        rate: Option<Finite>,
    },
    /// The fraction of samples that were not zero, how many were not, and how many there were.
    Rate {
        rate: Finite,
        passes: Finite,
        samples: Finite,
    },
    Gauge {
        value: Finite,
        min: Finite,
        max: Finite,
    },
}

impl Metric<'_> {
    pub(super) fn insert(self, results: &mut LeafResults) {
        let Self {
            name,
            contains,
            values,
        } = self;
        let Some((metric, tags)) = sub_metric(name) else {
            results.skip_measure();
            return;
        };
        let reading = Reading::of(metric);
        if matches!(reading, Reading::Setting) {
            return;
        }
        let Some(tags) = Tags::new(&tags) else {
            results.skip_variant();
            return;
        };
        if matches!(tags.scope, Scope::Metadata) && !values.proves_samples() {
            results.skip_variant();
            return;
        }
        let empty = values.is_empty();
        match (reading, values) {
            (Reading::Trend(measure), Values::Trend(statistics)) => {
                if !empty && let Some(mut variant) = variant(results, tags.with_outcome()) {
                    insert_trend(&mut variant, &measure, &statistics, NANOS_PER_MILLI);
                }
            },
            (Reading::Requests, Values::Counter { count, rate }) => {
                insert_requests(results, &tags, count, rate);
            },
            (Reading::Iterations, Values::Counter { count, rate }) => {
                if let Some(mut variant) = variant(results, tags.with_outcome()) {
                    count.insert(&mut variant, &Iterations::name_id(), VALUE, 1.0);
                    if let Some(rate) = rate {
                        rate.insert(&mut variant, &IterationThroughput::name_id(), MEAN, 1.0);
                    }
                }
            },
            (Reading::DroppedIterations, Values::Counter { count, .. }) => {
                if let Some(mut variant) = variant(results, tags.with_outcome()) {
                    count.insert(&mut variant, &DroppedIterations::name_id(), VALUE, 1.0);
                }
            },
            // The total is cut, as the rate holds it.
            (Reading::Data(direction), Values::Counter { rate, .. }) => {
                let mut keys = tags.with_outcome();
                keys.insert(DIRECTION.to_owned(), direction.into());
                if let Some(rate) = rate
                    && let Some(mut variant) = variant(results, keys)
                {
                    rate.insert(&mut variant, &BytesThroughput::name_id(), MEAN, 1.0);
                }
            },
            (Reading::Failed, Values::Rate { rate, passes, .. }) => {
                if let Some(mut variant) = variant(results, tags.with_outcome()) {
                    // An empty Rate's ratio is not a measured 0, but its failed count is.
                    if !empty {
                        rate.map(|rate| scale(rate, PERCENT)).insert(
                            &mut variant,
                            &FailureRatio::name_id(),
                            VALUE,
                            1.0,
                        );
                    }
                    passes.insert(&mut variant, &FailedRequests::name_id(), VALUE, 1.0);
                }
            },
            (Reading::Checks, Values::Rate { rate, .. }) => {
                if !empty && let Some(mut variant) = variant(results, tags.with_outcome()) {
                    rate.map(|rate| scale(rate, PERCENT)).insert(
                        &mut variant,
                        &CheckRatio::name_id(),
                        VALUE,
                        1.0,
                    );
                }
            },
            (Reading::Custom, values) => {
                if !empty {
                    insert_custom(results, metric, &tags, contains, values);
                }
            },
            (
                Reading::Setting
                | Reading::Trend(_)
                | Reading::Requests
                | Reading::Iterations
                | Reading::DroppedIterations
                | Reading::Data(_)
                | Reading::Failed
                | Reading::Checks,
                _,
            ) => results.skip_measure(),
        }
    }
}

impl Values {
    /// Whether k6 wrote the metric with no samples, as it writes a thresholded sub-metric that no
    /// sample matched (`internal/metrics/engine/engine.go`), which only a Trend's `count` column
    /// or a Rate's samples show.
    fn is_empty(&self) -> bool {
        match self {
            Self::Trend(statistics) => statistics
                .iter()
                .any(|(column, count)| column == "count" && count.get() == Some(0.0)),
            Self::Rate { samples, .. } => samples.get() == Some(0.0),
            Self::Counter { .. } | Self::Gauge { .. } => false,
        }
    }

    /// An empty Gauge prints its value, min, and max as 0.
    fn proves_samples(&self) -> bool {
        let positive = |value: Finite| value.get().is_some_and(|value| value > 0.0);
        match self {
            Self::Trend(statistics) => statistics
                .iter()
                .any(|(column, count)| column == "count" && positive(*count)),
            Self::Counter { count, .. } => positive(*count),
            Self::Rate { samples, .. } => positive(*samples),
            Self::Gauge { value, min, max } => [value, min, max]
                .into_iter()
                .any(|value| value.get().is_some_and(|value| value != 0.0)),
        }
    }
}

/// A check, by its name, with how many times it passed and failed.
#[derive(Deserialize)]
pub(super) struct Check {
    pub name: String,
    pub passes: Finite,
    pub fails: Finite,
}

/// A check's passes and failures, by the check's name, and its group's path where the summary
/// gives it.
pub(super) fn insert_check(
    results: &mut LeafResults,
    keys: Map<String, Value>,
    passes: Finite,
    fails: Finite,
) {
    if let Some(mut variant) = variant(results, keys) {
        passes.insert(&mut variant, &CheckPasses::name_id(), VALUE, 1.0);
        fails.insert(&mut variant, &CheckFailures::name_id(), VALUE, 1.0);
    }
}

/// No dropped iterations, for a summary without `dropped_iterations`, which k6 writes only once
/// an iteration drops (`lib/executor`).
pub(super) fn insert_no_drops(results: &mut LeafResults) {
    results
        .nameless_variant(K6, ParameterSet::default())
        .insert(DroppedIterations::name_id(), VALUE, 0.0);
}

pub(super) const CHECK: &str = "@check";
pub(super) const GROUP: &str = "@group";
pub(super) const DROPPED_ITERATIONS: &str = "dropped_iterations";

/// The name of k6's one benchmark when the run sets no prefix.
const K6: &str = "k6";
const VALUE: &str = "value";
const MEAN: &str = "mean";
const DIRECTION: &str = "@direction";
const NANOS_PER_MILLI: u32 = 1_000_000;
const PERCENT: u32 = 100;

/// How the leaf reads a k6 metric, by its name.
enum Reading {
    /// `vus` and `vus_max`, the virtual users the run resolved.
    Setting,
    Trend(MeasureNameId),
    Requests,
    Iterations,
    DroppedIterations,
    Data(&'static str),
    Failed,
    Checks,
    Custom,
}

impl Reading {
    fn of(metric: &str) -> Self {
        match metric {
            "vus" | "vus_max" => Self::Setting,
            "http_req_duration" => Self::Trend(Latency::name_id()),
            "http_req_blocked" => Self::Trend(BlockedTime::name_id()),
            "http_req_connecting" => Self::Trend(ConnectingTime::name_id()),
            "http_req_tls_handshaking" => Self::Trend(TlsHandshakingTime::name_id()),
            "http_req_sending" => Self::Trend(SendingTime::name_id()),
            "http_req_waiting" => Self::Trend(WaitingTime::name_id()),
            "http_req_receiving" => Self::Trend(ReceivingTime::name_id()),
            "iteration_duration" => Self::Trend(IterationTime::name_id()),
            "group_duration" => Self::Trend(GroupTime::name_id()),
            "http_reqs" => Self::Requests,
            "iterations" => Self::Iterations,
            DROPPED_ITERATIONS => Self::DroppedIterations,
            "data_received" => Self::Data("received"),
            "data_sent" => Self::Data("sent"),
            "http_req_failed" => Self::Failed,
            "checks" => Self::Checks,
            _ => Self::Custom,
        }
    }
}

/// A metric's name and its tags, split as k6 splits a sub-metric's name (`metrics/metric.go`): at
/// `,`, then at the first `:`, each side trimmed of spaces and then of quotes.
fn sub_metric(name: &str) -> Option<(&str, Vec<(&str, &str)>)> {
    let Some((metric, tags)) = name.split_once('{') else {
        return Some((name, Vec::new()));
    };
    let tags = tags
        .strip_suffix('}')?
        .split(',')
        .filter(|tag| !tag.is_empty())
        .map(|tag| {
            let (key, value) = tag.split_once(':').unwrap_or((tag, ""));
            (trim(key), trim(value))
        })
        .collect();
    Some((metric, tags))
}

fn trim(text: &str) -> &str {
    text.trim().trim_matches(['"', '\''])
}

/// A sub-metric's tags: `status` and `expected_response` as the response outcome, every other k6
/// system tag with `@` in front, and a user's own tag as written.
struct Tags {
    keys: Map<String, Value>,
    responses: Option<Outcome>,
    scope: Scope,
}

/// The responses a sub-metric covers, by status code or by whether k6 expected them.
enum Outcome {
    Status(String),
    Expected,
    Unexpected,
}

/// What a sub-metric's tags select on. k6 keeps `vu` and `iter` as metadata on most samples
/// (`metrics/system_tag.go`), so a sub-metric on one prints zeros unless a sample carries it as
/// a tag, as a user's own tag does, and `dropped_iterations` under `per-vu-iterations` when the
/// system tags hold `vu` (`lib/executor/base_executor.go`).
enum Scope {
    Tags,
    Metadata,
}

impl Tags {
    /// The tags, or `None` when two of them give the outcome or a user's tag is spelled like a
    /// key the leaf writes.
    fn new(tags: &[(&str, &str)]) -> Option<Self> {
        let mut keys = Map::new();
        let mut responses = None;
        let mut scope = Scope::Tags;
        for &(key, value) in tags {
            if matches!(key, "vu" | "iter") {
                scope = Scope::Metadata;
            }
            let outcome = match (key, value) {
                ("status", status) => Outcome::Status(status.to_owned()),
                ("expected_response", "true") => Outcome::Expected,
                ("expected_response", "false") => Outcome::Unexpected,
                ("expected_response", _) => return None,
                _ => {
                    let key = if SYSTEM_TAGS.contains(&key) {
                        format!("@{key}")
                    } else if is_reserved(key) {
                        return None;
                    } else {
                        key.to_owned()
                    };
                    keys.insert(key, value.into());
                    continue;
                },
            };
            if responses.replace(outcome).is_some() {
                return None;
            }
        }
        Some(Self {
            keys,
            responses,
            scope,
        })
    }

    /// The keys with the outcome as `@responses`.
    fn with_outcome(&self) -> Map<String, Value> {
        let mut keys = self.keys.clone();
        if let Some(outcome) = &self.responses {
            let outcome = match outcome {
                Outcome::Status(status) => status.as_str(),
                Outcome::Expected => "expected",
                Outcome::Unexpected => "unexpected",
            };
            keys.insert(RESPONSES.to_owned(), outcome.into());
        }
        keys
    }
}

/// k6's system tags (`metrics/system_tag.go`), the default ones and those `--system-tags` adds.
const SYSTEM_TAGS: [&str; 18] = [
    "proto",
    "subproto",
    "status",
    "method",
    "url",
    "name",
    "group",
    "check",
    "error",
    "error_code",
    "tls_version",
    "scenario",
    "service",
    "expected_response",
    "iter",
    "vu",
    "ocsp_status",
    "ip",
];

const RESPONSES: &str = "@responses";

/// Whether a user's tag is spelled like a key the leaf writes.
fn is_reserved(key: &str) -> bool {
    [RESPONSES, DIRECTION].contains(&key)
        || key
            .strip_prefix('@')
            .is_some_and(|tag| SYSTEM_TAGS.contains(&tag))
}

/// The k6 benchmark's variant of `keys`, or `None`, counted as skipped, past the key cap.
fn variant(results: &mut LeafResults, keys: Map<String, Value>) -> Option<Variant<'_>> {
    let Some(parameters) = parameter_set(keys) else {
        results.skip_variant();
        return None;
    };
    Some(results.nameless_variant(K6, parameters))
}

/// `http_reqs`: the count is `requests`, but a count by status code is `responses-<code>`, and
/// the rate is `throughput`, by the outcome.
fn insert_requests(results: &mut LeafResults, tags: &Tags, count: Finite, rate: Option<Finite>) {
    let Some(Outcome::Status(status)) = &tags.responses else {
        if let Some(mut variant) = variant(results, tags.with_outcome()) {
            count.insert(&mut variant, &Requests::name_id(), VALUE, 1.0);
            if let Some(rate) = rate {
                rate.insert(&mut variant, &Throughput::name_id(), MEAN, 1.0);
            }
        }
        return;
    };
    if let Some(mut variant) = variant(results, tags.keys.clone())
        && let Some(measure) = responses(&mut variant, status)
    {
        count.insert(&mut variant, &measure, VALUE, 1.0);
    }
    if let Some(rate) = rate
        && let Some(mut variant) = variant(results, tags.with_outcome())
    {
        rate.insert(&mut variant, &Throughput::name_id(), MEAN, 1.0);
    }
}

/// The measure of the responses with `status`: built in for a code IANA assigns, else named at
/// run time.
fn responses(variant: &mut Variant<'_>, status: &str) -> Option<MeasureNameId> {
    let Ok(code) = status.parse::<u16>() else {
        variant.skip_measure();
        return None;
    };
    Responses::name_id(code).or_else(|| variant.named(&Responses::slug(code), Responses::UNITS_STR))
}

/// A user's own metric, a measure named by the metric in the unit its values hold.
fn insert_custom(
    results: &mut LeafResults,
    metric: &str,
    tags: &Tags,
    contains: Option<Contains>,
    values: Values,
) {
    if writes(metric) {
        results.skip_measure();
        return;
    }
    match (values, contains) {
        (Values::Trend(statistics), Some(contains)) => {
            if let Some(mut variant) = variant(results, tags.with_outcome())
                && let Some((measure, factor)) = custom_measure(&mut variant, metric, contains)
            {
                insert_trend(&mut variant, &measure, &statistics, factor);
            }
        },
        (Values::Gauge { value, min, max }, Some(contains)) => {
            if let Some(mut variant) = variant(results, tags.with_outcome())
                && let Some((measure, factor)) = custom_measure(&mut variant, metric, contains)
            {
                for (name, value) in [(VALUE, value), ("min", min), ("max", max)] {
                    value.map(|value| scale(value, factor)).insert(
                        &mut variant,
                        &measure,
                        name,
                        1.0,
                    );
                }
            }
        },
        // The legacy export does not say whether these hold times.
        (Values::Trend(_) | Values::Gauge { .. }, None) => results.skip_measure(),
        (Values::Counter { count, .. }, None | Some(Contains::Default)) => {
            if let Some(mut variant) = variant(results, tags.with_outcome())
                && let Some((measure, _)) = custom_measure(&mut variant, metric, Contains::Default)
            {
                count.insert(&mut variant, &measure, VALUE, 1.0);
            }
        },
        // The total is cut, as the rate holds it.
        (Values::Counter { rate, .. }, Some(Contains::Data)) => {
            if let Some(rate) = rate
                && let Some(mut variant) = variant(results, tags.with_outcome())
                && let Some(measure) = variant.named(metric, BytesThroughput::UNITS_STR)
            {
                rate.insert(&mut variant, &measure, MEAN, 1.0);
            }
        },
        // A sum of times, which is cut.
        (Values::Counter { .. }, Some(Contains::Time)) => {},
        (Values::Rate { rate, .. }, _) => {
            if let Some(mut variant) = variant(results, tags.with_outcome())
                && let Some(measure) = variant.named(metric, PERCENTAGE)
            {
                rate.map(|rate| scale(rate, PERCENT))
                    .insert(&mut variant, &measure, VALUE, 1.0);
            }
        },
    }
}

/// Whether a user's metric names the slug of a measure the leaf writes, which it would overwrite.
fn writes(metric: &str) -> bool {
    Slug::new(metric).is_some_and(|slug| {
        let slug = slug.as_ref();
        WRITTEN.contains(&slug)
            || slug
                .strip_prefix("responses-")
                .is_some_and(|code| !code.is_empty() && code.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// Every measure the leaf writes but the run-time `responses-<code>`.
const WRITTEN: [&str; 20] = [
    Latency::SLUG_STR,
    Throughput::SLUG_STR,
    BytesThroughput::SLUG_STR,
    Requests::SLUG_STR,
    FailedRequests::SLUG_STR,
    FailureRatio::SLUG_STR,
    CheckRatio::SLUG_STR,
    CheckPasses::SLUG_STR,
    CheckFailures::SLUG_STR,
    Iterations::SLUG_STR,
    DroppedIterations::SLUG_STR,
    IterationThroughput::SLUG_STR,
    BlockedTime::SLUG_STR,
    ConnectingTime::SLUG_STR,
    TlsHandshakingTime::SLUG_STR,
    SendingTime::SLUG_STR,
    WaitingTime::SLUG_STR,
    ReceivingTime::SLUG_STR,
    IterationTime::SLUG_STR,
    GroupTime::SLUG_STR,
];

/// A user's metric as a measure in the unit `contains` gives, with the factor to it, or `None`,
/// counted as skipped, if its name is not a valid measure.
fn custom_measure(
    variant: &mut Variant<'_>,
    metric: &str,
    contains: Contains,
) -> Option<(MeasureNameId, u32)> {
    match contains {
        Contains::Default => {
            let Ok(measure) = metric.parse() else {
                variant.skip_measure();
                return None;
            };
            Some((measure, 1))
        },
        Contains::Time => Some((variant.named(metric, NANOSECONDS)?, NANOS_PER_MILLI)),
        Contains::Data => Some((variant.named(metric, BYTES)?, 1)),
    }
}

/// A Trend's statistics as `measure`'s metrics, each times `factor`.
fn insert_trend(
    variant: &mut Variant<'_>,
    measure: &MeasureNameId,
    statistics: &[(String, Finite)],
    factor: u32,
) {
    for (column, value) in statistics {
        match Column::of(column) {
            Column::Metric(metric) => value
                .map(|value| scale(value, factor))
                .insert(variant, measure, &metric, 1.0),
            Column::SampleCount => {},
            Column::Unknown => variant.skip_metric(),
        }
    }
}

/// What a `summaryTrendStats` column is (`metrics/sample.go`).
enum Column {
    Metric(String),
    /// `count`, how many samples the Trend holds.
    SampleCount,
    Unknown,
}

impl Column {
    fn of(column: &str) -> Self {
        let metric = match column {
            "avg" => "mean".to_owned(),
            "med" => "median".to_owned(),
            "min" | "max" => column.to_owned(),
            "count" => return Self::SampleCount,
            _ => {
                let Some(metric) = column
                    .strip_prefix("p(")
                    .and_then(|percent| percent.strip_suffix(')'))
                    .and_then(percentile_name)
                else {
                    return Self::Unknown;
                };
                metric
            },
        };
        Self::Metric(metric)
    }
}

/// `value` times `factor`, in decimal arithmetic on the shortest text that reads back as `value`,
/// so a fraction such as `0.29` is `29`, not `28.999999999999996`.
pub(super) fn scale(value: f64, factor: u32) -> f64 {
    value
        .to_string()
        .parse::<Decimal>()
        .ok()
        .and_then(|decimal| decimal.checked_mul(Decimal::from(factor)))
        .and_then(|decimal| decimal.to_f64())
        .unwrap_or(value * f64::from(factor))
}

#[cfg(test)]
mod test_http_k6 {
    use bencher_json::{
        MAX_PARAMETER_KEYS,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::Latency,
                http::{DroppedIterations, Iterations, Requests},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::bmf_v1::test_util::{assert_value, convert_edited_v1, convert_v1, value},
        results::adapter_results::{AdapterResults, Skipped},
    };

    fn named_edited<F: FnOnce(&mut serde_json::Value)>(suffix: &str, edit: F) -> AdapterResults {
        let mut results = convert_edited_v1(
            Adapter::HttpK6JsonSummary,
            &format!("http/k6/{suffix}"),
            edit,
        );
        results.name_nameless(None);
        results
    }

    /// Kills a sub-metric past the key cap kept, or skipped uncounted.
    #[test]
    fn adapter_http_k6_sub_metric_past_the_key_cap() {
        for (count, skipped) in [(MAX_PARAMETER_KEYS, 0), (MAX_PARAMETER_KEYS + 1, 1)] {
            let tags = (0..count)
                .map(|key| format!("k{key}:{key}"))
                .collect::<Vec<_>>()
                .join(",");
            let results = named_edited("export_simple.json", |json| {
                let duration = json["metrics"]["http_req_duration"].clone();
                json["metrics"][format!("http_req_duration{{{tags}}}").as_str()] = duration;
            });
            assert_eq!(results.skipped.variants, skipped, "{count} keys");
        }
    }

    /// Kills a user's tag spelled like a k6 key (`@vu` and `@iter` included) or a key the leaf
    /// writes kept, where it would stand for k6's own, a user's `@` tag the leaf never writes
    /// skipped, and a sub-metric that gives two outcomes read with either.
    #[test]
    fn adapter_http_k6_reserved_tags_and_two_outcomes() {
        let results = named_edited("export_simple.json", |json| {
            let duration = json["metrics"]["http_req_duration"].clone();
            for name in [
                "http_req_duration{@method:GET}",
                "http_req_duration{@responses:200}",
                "http_req_duration{@direction:sent}",
                "http_req_duration{@vu:1}",
                "http_req_duration{@iter:0}",
                "http_req_duration{status:200,expected_response:true}",
                "http_req_duration{@own:yes}",
            ] {
                json["metrics"][name] = duration.clone();
            }
        });
        assert_eq!(results.skipped.variants, 6);
        assert_value(
            &results,
            ("k6", r#"{"@own":"yes"}"#),
            (Latency::SLUG_STR, "min"),
            17_878.0,
        );
    }

    /// Kills `ip` or `ocsp_status`, which k6 adds only with `--system-tags`, read as a user's own
    /// tag.
    #[test]
    fn adapter_http_k6_optional_system_tags() {
        let mut results = convert_v1(
            Adapter::HttpK6JsonSummary,
            "http/k6/export_sub_metrics.json",
        );
        results.name_nameless(None);
        assert_value(
            &results,
            ("k6", r#"{"@ip":"127.0.0.1"}"#),
            (Latency::SLUG_STR, "max"),
            41_759_840.0,
        );
        assert_value(
            &results,
            ("k6", r#"{"@ocsp_status":"unknown"}"#),
            (Requests::SLUG_STR, "value"),
            0.0,
        );
    }

    /// Kills a request count of 0 on `vu` or `iter`, which proves no sample, stored as a measured
    /// 0, or skipped uncounted, in each of the 3 formats.
    #[test]
    fn adapter_http_k6_skips_vu_and_iter_sub_metrics_without_samples() {
        for (adapter, suffix) in [
            (Adapter::HttpK6JsonSummary, "export_sub_metrics.json"),
            (Adapter::HttpK6JsonSummary, "data_sub_metrics.json"),
            (Adapter::HttpK6Json, "machine_readable_sub_metrics.json"),
        ] {
            let mut results = convert_v1(adapter, &format!("http/k6/{suffix}"));
            results.name_nameless(None);
            assert_eq!(results.skipped.variants, 2, "{suffix}");
            for variants in results.inner.values() {
                for parameters in variants.keys() {
                    let parameters = serde_json::to_value(parameters).unwrap();
                    for key in ["vu", "iter", "@vu", "@iter"] {
                        assert!(parameters.get(key).is_none(), "{suffix} {parameters}");
                    }
                }
            }
        }
    }

    /// Kills a sub-metric on `vu` read when it proves no sample (a Counter, a Trend's `count`, or
    /// a Rate's samples at 0, a Trend with no `count` column, or a Gauge all at 0) or skipped
    /// uncounted, and one on `vu` or `iter` skipped when it proves samples, as
    /// `dropped_iterations` under `per-vu-iterations` and a user's own tag named `vu` or `iter`
    /// do, in each of the 3 formats, a Gauge with only its min or max not 0 and a Rate with no
    /// true sample included.
    #[test]
    fn adapter_http_k6_reads_vu_sub_metrics_with_samples() {
        for (adapter, suffix, skipped) in [
            (
                Adapter::HttpK6JsonSummary,
                "data_vu_tag.json",
                Skipped {
                    variants: 4,
                    ..Skipped::default()
                },
            ),
            (
                Adapter::HttpK6JsonSummary,
                "export_vu_tag.json",
                Skipped {
                    variants: 1,
                    measures: 2,
                    ..Skipped::default()
                },
            ),
            (
                Adapter::HttpK6Json,
                "machine_readable_vu_tag.json",
                Skipped {
                    variants: 2,
                    metrics: 8,
                    ..Skipped::default()
                },
            ),
        ] {
            let mut results = convert_v1(adapter, &format!("http/k6/{suffix}"));
            results.name_nameless(None);
            assert_eq!(results.skipped, skipped, "{suffix}");
            assert_value(
                &results,
                ("k6", r#"{"@vu":"1"}"#),
                (DroppedIterations::SLUG_STR, "value"),
                30.0,
            );
            assert_eq!(
                value(
                    &results,
                    "k6",
                    r#"{"@vu":"1"}"#,
                    Iterations::SLUG_STR,
                    "value"
                ),
                None,
                "{suffix}"
            );
        }
        let mut results = convert_v1(Adapter::HttpK6JsonSummary, "http/k6/data_vu_tag.json");
        results.name_nameless(None);
        for (parameters, measure, metric, expected) in [
            (r#"{"@vu":"a"}"#, "my_time", "max", 5_000_000.0),
            (r#"{"@vu":"a"}"#, "my_rate", "value", 100.0),
            (r#"{"@vu":"e"}"#, "my_rate", "value", 0.0),
            (r#"{"@iter":"z"}"#, "my_time", "max", 7_000_000.0),
            (r#"{"@vu":"a"}"#, "my_gauge", "value", 3.0),
            (r#"{"@vu":"c"}"#, "my_gauge", "max", 5.0),
            (r#"{"@vu":"d"}"#, "my_gauge", "min", -2.0),
        ] {
            assert_value(&results, ("k6", parameters), (measure, metric), expected);
        }
        for (parameters, measure, metric) in [
            (r#"{"@vu":"b"}"#, "my_time", "max"),
            (r#"{"@vu":"b"}"#, "my_gauge", "value"),
            (r#"{"@vu":"b"}"#, "my_rate", "value"),
        ] {
            assert_eq!(
                value(&results, "k6", parameters, measure, metric),
                None,
                "{parameters} {measure}"
            );
        }
    }

    /// Kills a user's metric whose name makes the slug of a measure the leaf writes written over
    /// k6's own, or skipped uncounted.
    #[test]
    fn adapter_http_k6_custom_metric_named_like_a_measure() {
        let results = named_edited("data_summary.json", |json| {
            let trend = json["metrics"]["order_time"].clone();
            for name in ["latency", "Group_Time", "responses_200", "Throughput"] {
                json["metrics"][name] = trend.clone();
            }
        });
        assert_eq!(results.skipped.measures, 4);
        assert_value(
            &results,
            ("k6", "{}"),
            (Latency::SLUG_STR, "min"),
            790_066.0,
        );
    }
}
