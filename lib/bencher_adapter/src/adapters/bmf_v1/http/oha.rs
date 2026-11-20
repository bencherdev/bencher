use bencher_json::{
    MeasureNameId, ParameterSet,
    project::measure::built_in::{
        BuiltInMeasure as _,
        default::Latency,
        http::{
            FailedRequests, Responses, Responses1xx, Responses2xx, Responses3xx, Responses4xx,
            Responses5xx, SuccessRatio, Timeouts,
        },
    },
};
use serde_json::{Map, Value};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::Finite,
        http::{oha_json::AdapterHttpOhaJson, oha_text::AdapterHttpOhaText},
        leaf::{LeafResults, Variant, parameter_set},
    },
    results::adapter_results::AdapterResults,
};

/// oha at version 1: its JSON summary, then its text summary.
///
/// oha names nothing for a run, so a run is one benchmark that the server names by the run's
/// benchmark prefix, else `oha`.
pub struct AdapterHttpOha;

impl Adaptable for AdapterHttpOha {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterHttpOhaJson::parse(input, settings)
            .or_else(|| AdapterHttpOhaText::parse(input, settings))
    }
}

/// What one oha run reports, as either leaf reads it. A value oha prints for no samples, `null`
/// in its JSON and `NaN` or an infinity in its text, is not finite and is left out uncounted.
pub(super) struct Run {
    /// The statistics of the run as a whole, by measure.
    pub measures: Vec<(MeasureNameId, Metrics)>,
    /// The statistics of the bytes received, by measure.
    pub received: Vec<(MeasureNameId, Metrics)>,
    /// The latency percentiles of the 2xx responses and of the rest, which oha prints only with
    /// `--stats-success-breakdown`.
    pub breakdown: Option<(Metrics, Metrics)>,
    /// Responses over requests that did not end at the run's deadline, as a percentage.
    pub success_ratio: Finite,
    /// How many responses had each status code.
    pub status_codes: Vec<(u16, f64)>,
    /// How many requests failed with each error, by oha's message.
    pub errors: Vec<(String, f64)>,
}

/// Each metric of a measure, by its name.
pub(super) type Metrics = Vec<(String, Finite)>;

impl Run {
    pub(super) fn insert(self, results: &mut LeafResults) {
        let Self {
            measures,
            received,
            breakdown,
            success_ratio,
            status_codes,
            errors,
        } = self;
        let mut run = results.nameless_variant(OHA, ParameterSet::default());
        for (measure, metrics) in &measures {
            insert_metrics(&mut run, measure, metrics);
        }
        if let Some(ratio) = success_ratio.get() {
            run.insert(SuccessRatio::name_id(), VALUE, ratio);
        }
        insert_errors(&mut run, &errors);
        insert_status_codes(&mut run, &status_codes);
        insert_variant(results, (DIRECTION, "received"), &received);
        if let Some((successful, not_successful)) = breakdown {
            let latency = Latency::name_id();
            for (outcome, metrics) in [
                ("successful", successful),
                ("not_successful", not_successful),
            ] {
                insert_variant(results, (RESPONSES, outcome), &[(latency.clone(), metrics)]);
            }
        }
    }
}

/// The name of oha's one benchmark when the run sets no prefix.
const OHA: &str = "oha";
const VALUE: &str = "value";
const DIRECTION: &str = "@direction";
const RESPONSES: &str = "@responses";
/// oha's message for a request past its `-t` timeout (`client.rs:193`).
const TIMEOUT: &str = "timeout";
/// oha's message for a request still in flight at the end of a `-z` run (`client.rs:195`), which
/// its own success rate leaves out (`result_data.rs:85-97`).
const DEADLINE: &str = "aborted due to deadline";

/// The finite metrics, each under `measure`.
fn insert_metrics(variant: &mut Variant<'_>, measure: &MeasureNameId, metrics: &Metrics) {
    for (metric, value) in metrics {
        if let Some(value) = value.get() {
            variant.insert(measure.clone(), metric, value);
        }
    }
}

/// `timeouts` and `failed-requests`, 0 when oha prints no error: every error but the deadline's
/// is a failed request, as oha's success rate counts it.
fn insert_errors(variant: &mut Variant<'_>, errors: &[(String, f64)]) {
    let (mut timeouts, mut failed) = (0.0, 0.0);
    for (error, count) in errors {
        if error == TIMEOUT {
            timeouts += count;
        }
        if error != DEADLINE {
            failed += count;
        }
    }
    variant.insert(Timeouts::name_id(), VALUE, timeouts);
    variant.insert(FailedRequests::name_id(), VALUE, failed);
}

/// Each status code's count as `responses-<code>`, built in for a code IANA assigns, else named
/// at run time, and each class's sum as `responses-1xx` to `responses-5xx`, for the classes that
/// hold a code.
fn insert_status_codes(variant: &mut Variant<'_>, status_codes: &[(u16, f64)]) {
    let mut classes: [Option<f64>; 5] = [None; 5];
    for &(code, count) in status_codes {
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

/// The variant with the one parameter `key`, built only when one of its metrics is finite.
fn insert_variant(
    results: &mut LeafResults,
    (key, value): (&str, &str),
    measures: &[(MeasureNameId, Metrics)],
) {
    if !measures
        .iter()
        .any(|(_, metrics)| metrics.iter().any(|(_, value)| value.get().is_some()))
    {
        return;
    }
    let Some(parameters) = parameter_set(Map::from_iter([(key.to_owned(), Value::from(value))]))
    else {
        results.skip_variant();
        return;
    };
    let mut variant = results.nameless_variant(OHA, parameters);
    for (measure, metrics) in measures {
        insert_metrics(&mut variant, measure, metrics);
    }
}
