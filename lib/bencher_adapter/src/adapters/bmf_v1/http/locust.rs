use std::borrow::Cow;

use bencher_json::{
    BenchmarkName, MeasureNameId,
    project::measure::built_in::{
        BuiltInMeasure as _,
        default::{Latency, Throughput},
        http::{FailedRequests, Requests, SizePerRequest},
    },
};
use rust_decimal::Decimal;
use serde_json::{Map, Value};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        http::{locust_csv::AdapterHttpLocustCsv, locust_json::AdapterHttpLocustJson},
        leaf::{LeafResults, parameter_set},
    },
    results::adapter_results::AdapterResults,
};

/// Locust at version 1: its request statistics CSV, then its `--json` output.
///
/// Each request is a benchmark named by its `Name`, with its `Type` as parameters, and Locust's
/// whole-test row, which names nothing, is one benchmark that the server names by the run's
/// benchmark prefix, else `locust`.
pub struct AdapterHttpLocust;

impl Adaptable for AdapterHttpLocust {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterHttpLocustCsv::parse(input, settings)
            .or_else(|| AdapterHttpLocustJson::parse(input, settings))
    }
}

/// What Locust reports for one request, or for the whole test.
pub(super) struct Entry<'a> {
    /// The request's `Type` and `Name`, or `None` for the whole test.
    pub request: Option<(&'a str, &'a str)>,
    pub requests: f64,
    pub failures: f64,
    /// Each latency statistic, in nanoseconds.
    pub latency: Vec<(Cow<'static, str>, f64)>,
    /// The mean bytes received per request.
    pub size_per_request: Option<f64>,
    /// The requests and the failures per second of the whole test.
    pub rates: Option<(f64, f64)>,
}

impl Entry<'_> {
    pub(super) fn insert(self, results: &mut LeafResults) {
        let Self {
            request,
            requests,
            failures,
            latency,
            size_per_request,
            rates,
        } = self;
        let (name, keys) = if let Some((kind, name)) = request {
            let Ok(name) = name.parse::<BenchmarkName>() else {
                results.skip_benchmark();
                return;
            };
            (Some(name), type_keys(kind))
        } else {
            (None, Map::new())
        };
        let Some(parameters) = parameter_set(keys.clone()) else {
            results.skip_variant();
            return;
        };
        let mut request = if let Some(name) = &name {
            // A repeat of a variant, as `get` beside `GET` for one `Name`, is skipped and counted.
            let Some(variant) = results.new_variant(name.clone(), parameters) else {
                return;
            };
            variant
        } else {
            results.nameless_variant(LOCUST, parameters)
        };
        request.insert(Requests::name_id(), VALUE, requests);
        request.insert(FailedRequests::name_id(), VALUE, failures);
        for (metric, value) in latency {
            request.insert(Latency::name_id(), &metric, value);
        }
        if let Some((requests_per_second, _)) = rates {
            request.insert(Throughput::name_id(), MEAN, requests_per_second);
        }
        if let Some((_, failures_per_second)) = rates {
            insert_split(
                results,
                name.as_ref(),
                &keys,
                (RESPONSES, FAILURES),
                (Throughput::name_id(), failures_per_second),
            );
        }
        if let Some(size) = size_per_request {
            insert_split(
                results,
                name.as_ref(),
                &keys,
                (DIRECTION, RECEIVED),
                (SizePerRequest::name_id(), size),
            );
        }
    }
}

/// The name of Locust's whole-test benchmark when the run sets no prefix.
const LOCUST: &str = "locust";
const MEAN: &str = "mean";
const VALUE: &str = "value";
const RESPONSES: &str = "@responses";
const FAILURES: &str = "failures";
const DIRECTION: &str = "@direction";
const RECEIVED: &str = "received";
/// The HTTP verbs, which an HTTP client writes as a request's `Type`.
const HTTP_METHODS: [&str; 9] = [
    "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "CONNECT", "TRACE",
];

/// A request's `Type` as parameters: `@type: http` and `@method` for an HTTP verb in any case,
/// which Locust's HTTP clients send upper-cased, else `@type` as Locust prints it.
fn type_keys(kind: &str) -> Map<String, Value> {
    if let Some(method) = HTTP_METHODS
        .into_iter()
        .find(|method| method.eq_ignore_ascii_case(kind))
    {
        Map::from_iter([
            ("@type".to_owned(), Value::from("http")),
            ("@method".to_owned(), Value::from(method)),
        ])
    } else {
        Map::from_iter([("@type".to_owned(), Value::from(kind))])
    }
}

/// A `mean` on the variant of the request that adds one key to its parameters.
fn insert_split(
    results: &mut LeafResults,
    name: Option<&BenchmarkName>,
    keys: &Map<String, Value>,
    (key, value): (&str, &str),
    (measure, mean): (MeasureNameId, f64),
) {
    let mut keys = keys.clone();
    keys.insert(key.to_owned(), Value::from(value));
    let Some(parameters) = parameter_set(keys) else {
        results.skip_variant();
        return;
    };
    let mut variant = if let Some(name) = name {
        results.variant(name.clone(), parameters)
    } else {
        results.nameless_variant(LOCUST, parameters)
    };
    variant.insert(measure, MEAN, mean);
}

/// A time Locust gives in milliseconds as a decimal number, in nanoseconds, scaled in decimal so
/// that the nanoseconds are the digits Locust printed, and read as the float nearest them.
pub(super) fn nanos(millis: &str) -> Option<f64> {
    let decimal = millis
        .parse::<Decimal>()
        .or_else(|_err| Decimal::from_scientific(millis))
        .ok();
    decimal
        .and_then(|decimal| decimal.checked_mul(Decimal::from(NANOS_PER_MILLI)))
        .and_then(|decimal| decimal.to_string().parse::<f64>().ok())
        .or_else(|| {
            millis
                .parse::<f64>()
                .ok()
                .map(|millis| millis * f64::from(NANOS_PER_MILLI))
        })
}

const NANOS_PER_MILLI: u32 = 1_000_000;

#[cfg(test)]
mod test_http_locust {
    use bencher_json::project::report::Adapter;
    use pretty_assertions::assert_eq;

    use crate::adapters::bmf_v1::test_util::convert_v1;

    /// Kills a node that tries only one of its leaves.
    #[test]
    fn adapter_http_locust_reads_with_each_leaf() {
        for (suffix, leaf) in [
            ("basic_stats.csv", Adapter::HttpLocustCsv),
            ("basic.json", Adapter::HttpLocustJson),
        ] {
            let results = convert_v1(Adapter::HttpLocust, &format!("http/locust/{suffix}"));
            assert_eq!(results.resolved_adapter, Some(leaf), "{suffix}");
        }
    }
}
