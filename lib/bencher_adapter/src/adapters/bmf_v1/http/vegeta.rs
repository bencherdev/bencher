use bencher_json::{
    MeasureNameId, ParameterSet,
    project::measure::built_in::{
        BuiltInMeasure as _,
        default::{Latency, Throughput},
        http::{
            FailedRequests, Requests, Responses, Responses1xx, Responses2xx, Responses3xx,
            Responses4xx, Responses5xx, SizePerRequest, SuccessRatio,
        },
    },
};
use serde_json::{Map, Value};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        http::{vegeta_json::AdapterHttpVegetaJson, vegeta_text::AdapterHttpVegetaText},
        leaf::{LeafResults, Variant, parameter_set},
    },
    results::adapter_results::AdapterResults,
};

/// Vegeta at version 1: `vegeta report -type=json`, then its text report.
///
/// Vegeta names nothing in a report, so a report is one benchmark that the server names by the
/// run's benchmark prefix, else `vegeta`.
pub struct AdapterHttpVegeta;

impl Adaptable for AdapterHttpVegeta {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterHttpVegetaJson::parse(input, settings)
            .or_else(|| AdapterHttpVegetaText::parse(input, settings))
    }
}

/// What one Vegeta report holds, as either leaf reads it (`lib/metrics.go:12-115` at 12.13.0).
pub(super) struct Run {
    /// The latency statistics in nanoseconds, by metric.
    pub latency: Vec<(&'static str, f64)>,
    /// `None` when every request started at one instant, for which Vegeta writes each count in
    /// place of its rate (`metrics.go:97-105`).
    pub rates: Option<Rates>,
    pub requests: f64,
    /// Responses with a status code from 200 to 399 over all requests, as a percentage.
    pub success_ratio: f64,
    /// The mean bytes received and sent per request.
    pub size_per_request: Directions,
    /// How many requests ended with each status code, `0` for a request with no response.
    pub status_codes: Vec<(u16, f64)>,
}

/// Requests per second.
pub(super) struct Rates {
    /// Every request over the attack time.
    pub all: f64,
    /// The successful requests over the attack time plus the wait for the last responses.
    pub successful: f64,
}

pub(super) struct Directions {
    pub received: f64,
    pub sent: f64,
}

impl Run {
    pub(super) fn insert(self, results: &mut LeafResults) {
        let Self {
            latency,
            rates,
            requests,
            success_ratio,
            size_per_request: Directions { received, sent },
            status_codes,
        } = self;
        let mut run = results.nameless_variant(VEGETA, ParameterSet::default());
        let measure = Latency::name_id();
        for (metric, value) in latency {
            run.insert(measure.clone(), metric, value);
        }
        if let Some(Rates { all, .. }) = rates {
            run.insert(Throughput::name_id(), MEAN, all);
        }
        run.insert(Requests::name_id(), VALUE, requests);
        run.insert(SuccessRatio::name_id(), VALUE, success_ratio);
        insert_status_codes(&mut run, &status_codes);
        for (direction, size) in [("received", received), ("sent", sent)] {
            insert_variant(
                results,
                (DIRECTION, direction),
                SizePerRequest::name_id(),
                size,
            );
        }
        if let Some(Rates { successful, .. }) = rates {
            insert_variant(
                results,
                (RESPONSES, "successful"),
                Throughput::name_id(),
                successful,
            );
        }
    }
}

/// The name of Vegeta's one benchmark when the run sets no prefix.
const VEGETA: &str = "vegeta";
const MEAN: &str = "mean";
const VALUE: &str = "value";
const DIRECTION: &str = "@direction";
const RESPONSES: &str = "@responses";
/// The status code Vegeta records for a request that got no response (`lib/results.go:29`).
const NO_RESPONSE: u16 = 0;

/// Each status code's count as `responses-<code>`, built in for a code IANA assigns, else named
/// at run time, each class's sum as `responses-1xx` to `responses-5xx`, for the classes that hold
/// a code, and the requests with no response as `failed-requests`, 0 when Vegeta records none.
fn insert_status_codes(variant: &mut Variant<'_>, status_codes: &[(u16, f64)]) {
    let mut failed = 0.0;
    let mut classes: [Option<f64>; 5] = [None; 5];
    for &(code, count) in status_codes {
        if code == NO_RESPONSE {
            failed = count;
            continue;
        }
        if let Some(measure) = Responses::name_id(code)
            .or_else(|| variant.named(&Responses::slug(code), Responses::UNITS_STR))
        {
            variant.insert(measure, VALUE, count);
        }
        if let Some(class) = class(code).and_then(|class| classes.get_mut(class)) {
            *class.get_or_insert(0.0) += count;
        }
    }
    variant.insert(FailedRequests::name_id(), VALUE, failed);
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

/// The variant with the one parameter `key`, holding `measure`'s `mean`.
fn insert_variant(
    results: &mut LeafResults,
    (key, value): (&str, &str),
    measure: MeasureNameId,
    mean: f64,
) {
    let Some(parameters) = parameter_set(Map::from_iter([(key.to_owned(), Value::from(value))]))
    else {
        results.skip_variant();
        return;
    };
    results
        .nameless_variant(VEGETA, parameters)
        .insert(measure, MEAN, mean);
}
