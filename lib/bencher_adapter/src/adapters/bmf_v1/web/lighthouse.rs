use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
};

use bencher_json::{
    BenchmarkName, MeasureNameId,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            web::{
                CategoryScore, CumulativeLayoutShift, DomElements, FirstContentfulPaint,
                LargestContentfulPaint, MainThreadTime, MaxPotentialFirstInputDelay,
                ScriptExecutionTime, ServerResponseTime, SpeedIndex, TimeToInteractive,
                TotalBlockingTime, TransferSize,
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
    adapters::bmf_v1::leaf::{LeafResults, parameter_set},
    results::adapter_results::AdapterResults,
};

/// The hosts of a local server, whose port a page's name drops, since Lighthouse CI's own server
/// binds a new port on every run.
const LOOPBACK_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];
const NANOS_PER_MILLI: f64 = 1e6;

/// The Lighthouse Result of one navigation, as `lighthouse --output json` and Lighthouse CI write
/// it: each metric audit and each category score is one `value` of the page.
///
/// A result with a runtime error is refused, and [`AdapterWebLighthouse::failure`] names the
/// error for the 400.
pub struct AdapterWebLighthouse;

impl Adaptable for AdapterWebLighthouse {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let result = LighthouseResult::navigation(input)?;
        if result.runtime_error.is_some() {
            return None;
        }
        let mut results = LeafResults::default();
        let Ok(benchmark) = page_name(&result.requested_url).parse::<BenchmarkName>() else {
            results.skip_benchmark();
            return results.finish_with_skips(Adapter::WebLighthouse);
        };
        let settings = result.config_settings.parameters();
        let mut metrics = Vec::new();
        for (measure, audits, unit) in metric_audits() {
            match result.read(audits, unit) {
                Reading::Value(value) => metrics.push((measure, value)),
                Reading::Skipped => results.skip_measure(),
                Reading::Absent => {},
            }
        }
        if !metrics.is_empty() {
            write(&mut results, &benchmark, settings.clone(), metrics);
        }
        for (category, Category { score }) in result.categories {
            let Some(score) = score.as_ref().and_then(percent) else {
                results.skip_measure();
                continue;
            };
            let mut parameters = settings.clone();
            parameters.insert("@category".to_owned(), category.into());
            write(
                &mut results,
                &benchmark,
                parameters,
                vec![(CategoryScore::name_id(), score)],
            );
        }
        results.finish_with_skips(Adapter::WebLighthouse)
    }
}

impl AdapterWebLighthouse {
    /// Lighthouse's reason a navigation's result holds no measurement: the runtime error it
    /// writes when it could not load the page (`core/runner.js:471-497`).
    pub(crate) fn failure(input: &str) -> Option<String> {
        let LighthouseResult {
            lighthouse_version,
            requested_url,
            runtime_error,
            ..
        } = LighthouseResult::navigation(input)?;
        let RuntimeError { code, message } = runtime_error?;
        Some(format!(
            "Lighthouse {lighthouse_version} could not measure {requested_url}: {code}: {message}"
        ))
    }
}

/// The page's `requestedUrl`, which Lighthouse writes as `new URL(url).href`
/// (`core/lib/url-utils.js:272`), with the port dropped for a loopback host.
fn page_name(requested_url: &str) -> Cow<'_, str> {
    loopback_port(requested_url)
        .and_then(|(start, end)| {
            Some(format!(
                "{}{}",
                requested_url.get(..start)?,
                requested_url.get(end..)?
            ))
        })
        .map_or(Cow::Borrowed(requested_url), Cow::Owned)
}

/// A score from 0 to 1 as a percentage, in decimal arithmetic, so each score Lighthouse rounds to
/// 2 decimals (`core/scoring.js:14`) gives its exact percentage.
fn percent(score: &Number) -> Option<f64> {
    let score = score.to_string();
    let score = score
        .parse::<Decimal>()
        .or_else(|_| Decimal::from_scientific(&score))
        .ok()?;
    score
        .checked_mul(Decimal::ONE_HUNDRED)?
        .normalize()
        .to_f64()
}

/// Each metric audit's measure, the audits that write it in the order they are read, and the unit
/// Lighthouse writes it in.
fn metric_audits() -> [(MeasureNameId, &'static [&'static str], NumericUnit); 12] {
    [
        (
            FirstContentfulPaint::name_id(),
            &["first-contentful-paint"],
            NumericUnit::Millisecond,
        ),
        (
            LargestContentfulPaint::name_id(),
            &["largest-contentful-paint"],
            NumericUnit::Millisecond,
        ),
        (
            TotalBlockingTime::name_id(),
            &["total-blocking-time"],
            NumericUnit::Millisecond,
        ),
        (
            CumulativeLayoutShift::name_id(),
            &["cumulative-layout-shift"],
            NumericUnit::Unitless,
        ),
        (
            SpeedIndex::name_id(),
            &["speed-index"],
            NumericUnit::Millisecond,
        ),
        (
            TimeToInteractive::name_id(),
            &["interactive"],
            NumericUnit::Millisecond,
        ),
        (
            MaxPotentialFirstInputDelay::name_id(),
            &["max-potential-fid"],
            NumericUnit::Millisecond,
        ),
        (
            ServerResponseTime::name_id(),
            &["server-response-time"],
            NumericUnit::Millisecond,
        ),
        (
            TransferSize::name_id(),
            &["total-byte-weight"],
            NumericUnit::Byte,
        ),
        // Lighthouse 13 counts elements in `dom-size-insight` alone, where 12 writes it with no
        // value beside `dom-size`.
        (
            DomElements::name_id(),
            &["dom-size-insight", "dom-size"],
            NumericUnit::Element,
        ),
        (
            MainThreadTime::name_id(),
            &["mainthread-work-breakdown"],
            NumericUnit::Millisecond,
        ),
        (
            ScriptExecutionTime::name_id(),
            &["bootup-time"],
            NumericUnit::Millisecond,
        ),
    ]
}

/// One variant of the page, or a counted skip when its parameters are not valid.
fn write(
    results: &mut LeafResults,
    benchmark: &BenchmarkName,
    parameters: Map<String, Value>,
    metrics: Vec<(MeasureNameId, f64)>,
) {
    let Some(parameters) = parameter_set(parameters) else {
        results.skip_variant();
        return;
    };
    let mut variant = results.variant(benchmark.clone(), parameters);
    for (measure, value) in metrics {
        variant.insert(measure, "value", value);
    }
}

/// The byte range of the `:port` in a loopback host's URL, if it has one.
fn loopback_port(url: &str) -> Option<(usize, usize)> {
    let authority_start = url.find("://")? + "://".len();
    let rest = url.get(authority_start..)?;
    let authority = rest.get(..rest.find(['/', '?', '#']).unwrap_or(rest.len()))?;
    let host_start = authority.rfind('@').map_or(0, |at| at + 1);
    let host_port = authority.get(host_start..)?;
    let host_len = if host_port.starts_with('[') {
        host_port.find(']')? + 1
    } else {
        host_port.find(':')?
    };
    let (host, port) = host_port.split_at_checked(host_len)?;
    (LOOPBACK_HOSTS.contains(&host) && !port.is_empty()).then(|| {
        let start = authority_start + host_start + host_len;
        (start, start + port.len())
    })
}

/// A Lighthouse Result (`types/lhr/lhr.d.ts:14-70`), whose `requestedUrl` Lighthouse writes only
/// for a navigation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LighthouseResult {
    lighthouse_version: String,
    gather_mode: GatherMode,
    requested_url: String,
    runtime_error: Option<RuntimeError>,
    config_settings: ConfigSettings,
    audits: HashMap<String, Audit>,
    categories: BTreeMap<String, Category>,
}

impl LighthouseResult {
    /// The result, if it is of a navigation: a timespan or a snapshot loads no page.
    fn navigation(input: &str) -> Option<Self> {
        serde_json::from_str::<Self>(input)
            .ok()
            .filter(|result| result.gather_mode == GatherMode::Navigation)
    }

    /// The value of the first of `audits` that holds one, in Bencher's unit, or a counted skip
    /// when none does and one of them errored or holds another unit.
    fn read(&self, audits: &[&str], unit: NumericUnit) -> Reading {
        let mut reading = Reading::Absent;
        for audit in audits.iter().filter_map(|id| self.audits.get(*id)) {
            match audit.score_display_mode {
                ScoreDisplayMode::NotApplicable => {},
                ScoreDisplayMode::Error => reading = Reading::Skipped,
                ScoreDisplayMode::Scored => {
                    let Some(value) = audit.numeric_value else {
                        continue;
                    };
                    if audit.numeric_unit != Some(unit) {
                        reading = Reading::Skipped;
                        continue;
                    }
                    return Reading::Value(value * unit.scale());
                },
            }
        }
        reading
    }
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum GatherMode {
    Navigation,
    Timespan,
    Snapshot,
}

#[derive(Deserialize)]
struct RuntimeError {
    code: String,
    message: String,
}

/// The resolved settings, every one written whether the user set it or not
/// (`core/config/constants.js:52-89`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigSettings {
    form_factor: String,
    throttling_method: String,
    throttling: Map<String, Value>,
    screen_emulation: Map<String, Value>,
}

impl ConfigSettings {
    /// The settings that change the metrics: the form factor, the throttling method and only the
    /// throttling values it applies (`types/lhr/settings.d.ts:12-27`), since `provided` applies
    /// none though every value is still written, and the screen, or `disabled`.
    fn parameters(&self) -> Map<String, Value> {
        let mut parameters = Map::new();
        parameters.insert("@formFactor".to_owned(), self.form_factor.clone().into());
        parameters.insert(
            "@throttlingMethod".to_owned(),
            self.throttling_method.clone().into(),
        );
        let throttling: &[&str] = match self.throttling_method.as_str() {
            "simulate" => &["rttMs", "throughputKbps", "cpuSlowdownMultiplier"],
            "devtools" => &[
                "requestLatencyMs",
                "downloadThroughputKbps",
                "uploadThroughputKbps",
                "cpuSlowdownMultiplier",
            ],
            _ => &[],
        };
        let screen: &[&str] = if self.screen_emulation.get("disabled") == Some(&Value::Bool(true)) {
            parameters.insert("@screenEmulation".to_owned(), "disabled".into());
            &[]
        } else {
            &["width", "height", "deviceScaleFactor"]
        };
        for (settings, keys) in [
            (&self.throttling, throttling),
            (&self.screen_emulation, screen),
        ] {
            for key in keys {
                if let Some(value) = settings.get(*key) {
                    parameters.insert(format!("@{key}"), value.clone());
                }
            }
        }
        parameters
    }
}

/// An audit (`types/lhr/audit-result.d.ts:37-81`), with a value only where it measures a number.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Audit {
    score_display_mode: ScoreDisplayMode,
    numeric_value: Option<f64>,
    numeric_unit: Option<NumericUnit>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum ScoreDisplayMode {
    NotApplicable,
    Error,
    #[serde(other)]
    Scored,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum NumericUnit {
    Millisecond,
    Byte,
    Element,
    Unitless,
    #[serde(other)]
    Other,
}

impl NumericUnit {
    fn scale(self) -> f64 {
        if self == Self::Millisecond {
            NANOS_PER_MILLI
        } else {
            1.0
        }
    }
}

/// A category, whose score is null when any audit it weighs has none (`core/scoring.js:26`).
#[derive(Deserialize)]
struct Category {
    score: Option<Number>,
}

enum Reading {
    Value(f64),
    Skipped,
    Absent,
}

#[cfg(test)]
mod test_web_lighthouse {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                web::{
                    CategoryScore, CumulativeLayoutShift, DomElements, FirstContentfulPaint,
                    LargestContentfulPaint, MainThreadTime, MaxPotentialFirstInputDelay,
                    ScriptExecutionTime, ServerResponseTime, SpeedIndex, TimeToInteractive,
                    TotalBlockingTime, TransferSize,
                },
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;
    use serde_json::{Value, json};

    use super::{page_name, percent};
    use crate::{
        Adaptable as _, AdapterError, AdapterResultsArray, Settings,
        adapters::bmf_v1::test_util::{
            assert_value, assert_variant, convert_edited_v1, convert_v1, value, variants,
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LIGHTHOUSE: &str = "web/lighthouse";
    const PAGE: &str = "http://localhost/";

    fn convert(fixture: &str) -> AdapterResults {
        convert_v1(Adapter::WebLighthouse, &format!("{LIGHTHOUSE}/{fixture}"))
    }

    fn convert_edited<F: FnOnce(&mut Value)>(fixture: &str, edit: F) -> AdapterResults {
        convert_edited_v1(
            Adapter::WebLighthouse,
            &format!("{LIGHTHOUSE}/{fixture}"),
            edit,
        )
    }

    fn read(fixture: &str) -> Value {
        let path = format!("./tool_output/{LIGHTHOUSE}/{fixture}");
        let input =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {path}: {e}"));
        serde_json::from_str(&input).unwrap_or_else(|e| panic!("Failed to parse {path}: {e}"))
    }

    /// Lighthouse's default settings: a mobile run under `simulate`.
    fn mobile_simulate() -> Value {
        json!({
            "@formFactor": "mobile",
            "@throttlingMethod": "simulate",
            "@rttMs": 150,
            "@throughputKbps": 1638.4,
            "@cpuSlowdownMultiplier": 4,
            "@width": 412,
            "@height": 823,
            "@deviceScaleFactor": 1.75,
        })
    }

    /// A mobile run under `devtools`, which applies its own throttling values.
    fn mobile_devtools() -> Value {
        json!({
            "@formFactor": "mobile",
            "@throttlingMethod": "devtools",
            "@requestLatencyMs": 562.5,
            "@downloadThroughputKbps": 1_474.560_000_000_000_2,
            "@uploadThroughputKbps": 675,
            "@cpuSlowdownMultiplier": 4,
            "@width": 412,
            "@height": 823,
            "@deviceScaleFactor": 1.75,
        })
    }

    fn with_category(settings: &Value, category: &str) -> String {
        let mut parameters = settings.clone();
        parameters["@category"] = category.into();
        parameters.to_string()
    }

    /// Every variant's parameters, as canonical JSON.
    fn parameter_sets(results: &AdapterResults) -> Vec<String> {
        let mut sets: Vec<String> = results
            .inner
            .values()
            .flat_map(|variants| variants.keys().map(ToString::to_string))
            .collect();
        sets.sort();
        sets
    }

    fn canonical(parameters: &str) -> String {
        parameters
            .parse::<bencher_json::ParameterSet>()
            .expect("Failed to parse parameters")
            .to_string()
    }

    fn category_score(results: &AdapterResults, settings: &Value, category: &str) -> Option<f64> {
        value(
            results,
            PAGE,
            &with_category(settings, category),
            CategoryScore::SLUG_STR,
            "value",
        )
    }

    /// Each of the 12 metric audits is one `value` of its measure, a time in ns, under the run's
    /// settings, and nothing else of the result is written.
    ///
    /// Kills: a time left in ms, an audit read into another measure, a measure dropped, and a
    /// savings estimate, `network-rtt`, or the `metrics` audit written.
    #[test]
    fn web_lighthouse_reads_each_metric_audit_as_its_measure() {
        let results = convert("perf_only.json");
        assert_variant(
            &results,
            PAGE,
            &mobile_simulate().to_string(),
            &[
                (
                    FirstContentfulPaint::SLUG_STR,
                    vec![("value", 751_255_500.0)],
                ),
                (
                    LargestContentfulPaint::SLUG_STR,
                    vec![("value", 2_101_255_500.0)],
                ),
                (TotalBlockingTime::SLUG_STR, vec![("value", 953_000_000.0)]),
                (
                    CumulativeLayoutShift::SLUG_STR,
                    vec![("value", 0.125_135_888_559_527_06)],
                ),
                (SpeedIndex::SLUG_STR, vec![("value", 778_000_914.000_007_2)]),
                (
                    TimeToInteractive::SLUG_STR,
                    vec![("value", 2_101_255_500.0)],
                ),
                (
                    MaxPotentialFirstInputDelay::SLUG_STR,
                    vec![("value", 1_003_000_000.0)],
                ),
                (ServerResponseTime::SLUG_STR, vec![("value", 0.0)]),
                (TransferSize::SLUG_STR, vec![("value", 226_852.0)]),
                (DomElements::SLUG_STR, vec![("value", 210.0)]),
                (
                    MainThreadTime::SLUG_STR,
                    vec![("value", 1_152_579_999.999_999_5)],
                ),
                (
                    ScriptExecutionTime::SLUG_STR,
                    vec![("value", 1_011_843_999.999_999_8)],
                ),
            ],
        );
        assert_eq!(
            category_score(&results, &mobile_simulate(), "performance"),
            Some(73.0)
        );
        assert_eq!((results.inner.len(), variants(&results)), (1, 2));
        assert_eq!(results.skipped, Skipped::default());
        // Under `simulate` the Time to Interactive equals the Largest Contentful Paint here, so a
        // `devtools` run tells the 2 audits apart.
        let devtools = convert("devtools.json");
        for (measure, expected) in [
            (LargestContentfulPaint::SLUG_STR, 2_392_257_000.0),
            (TimeToInteractive::SLUG_STR, 1_477_664_000.0),
        ] {
            assert_value(
                &devtools,
                (PAGE, &mobile_devtools().to_string()),
                (measure, "value"),
                expected,
            );
        }
    }

    /// Each category's score is one `category-score` in %, its id the `@category`, plugin and
    /// minor-release categories such as `agentic-browsing` included.
    ///
    /// Kills: a score left from 0 to 1, a category dropped, and a measure per category.
    #[test]
    fn web_lighthouse_writes_each_category_score_as_a_percentage() {
        let results = convert("lhci_about.json");
        let settings = mobile_simulate();
        let mut sets = vec![canonical(&settings.to_string())];
        for (category, score) in [
            ("performance", 100.0),
            ("accessibility", 96.0),
            ("best-practices", 96.0),
            ("seo", 100.0),
            ("agentic-browsing", 100.0),
        ] {
            assert_eq!(
                value(
                    &results,
                    "http://localhost/about.html",
                    &with_category(&settings, category),
                    CategoryScore::SLUG_STR,
                    "value",
                ),
                Some(score),
                "{category}"
            );
            sets.push(canonical(&with_category(&settings, category)));
        }
        sets.sort();
        assert_eq!(parameter_sets(&results), sets);
    }

    /// A score is multiplied by 100 in decimal arithmetic, so each 2-decimal score Lighthouse
    /// prints is its exact percentage, where f64 gives 28.999999999999996 for 0.29.
    ///
    /// Kills: the percentage computed in f64.
    #[test]
    fn web_lighthouse_scores_in_decimal_arithmetic() {
        for (score, expected) in [
            ("0.07", 7.0),
            ("0.29", 29.0),
            ("0.57", 57.0),
            ("1", 100.0),
            ("0", 0.0),
        ] {
            let score =
                serde_json::from_str::<serde_json::Number>(score).expect("Failed to parse a score");
            assert_eq!(percent(&score), Some(expected), "{score}");
        }
    }

    /// Lighthouse 12 counts DOM elements in `dom-size`, where its `dom-size-insight` has no
    /// value, and 13 counts them in `dom-size-insight` alone.
    ///
    /// Kills: `dom-elements` read from only one of the 2 audits.
    #[test]
    fn web_lighthouse_reads_dom_elements_from_either_version() {
        for (fixture, elements) in [("v12_mobile.json", 208.0), ("perf_only.json", 210.0)] {
            let results = convert(fixture);
            assert_eq!(
                value(
                    &results,
                    PAGE,
                    &mobile_simulate().to_string(),
                    DomElements::SLUG_STR,
                    "value"
                ),
                Some(elements),
                "{fixture}"
            );
            assert_eq!(results.skipped, Skipped::default(), "{fixture}");
        }
    }

    /// Lighthouse 12 with `--skip-audits=dom-size` writes no `dom-size`, and its `dom-size-insight`
    /// holds no value, so `dom-elements` is absent and nothing is counted.
    ///
    /// Kills: an audit with no value outside `error` counted as a skip.
    #[test]
    fn web_lighthouse_skips_a_valueless_audit_uncounted() {
        let results = convert_edited("v12_mobile.json", |result| {
            result["audits"]
                .as_object_mut()
                .expect("Failed to read the audits")
                .remove("dom-size");
        });
        assert_eq!(
            value(
                &results,
                PAGE,
                &mobile_simulate().to_string(),
                DomElements::SLUG_STR,
                "value"
            ),
            None
        );
        assert_eq!(results.skipped, Skipped::default());
    }

    /// The resolved settings are parameters: the form factor, the throttling method and only the
    /// throttling values it applies, and the screen, or `disabled` when there is no emulation.
    ///
    /// Kills: a method's throttling values swapped for another's, values written under
    /// `provided`, which applies none, a screen size written with emulation disabled, and the
    /// form factor dropped.
    #[test]
    fn web_lighthouse_writes_the_resolved_settings() {
        let provided = json!({
            "@formFactor": "mobile",
            "@throttlingMethod": "provided",
            "@width": 412,
            "@height": 823,
            "@deviceScaleFactor": 1.75,
        });
        for (fixture, settings) in [
            (
                "desktop_preset.json",
                json!({
                    "@formFactor": "desktop",
                    "@throttlingMethod": "simulate",
                    "@rttMs": 40,
                    "@throughputKbps": 10240,
                    "@cpuSlowdownMultiplier": 1,
                    "@width": 1350,
                    "@height": 940,
                    "@deviceScaleFactor": 1,
                }),
            ),
            (
                "form_factor_desktop.json",
                json!({
                    "@formFactor": "desktop",
                    "@throttlingMethod": "simulate",
                    "@rttMs": 150,
                    "@throughputKbps": 1638.4,
                    "@cpuSlowdownMultiplier": 4,
                    "@screenEmulation": "disabled",
                }),
            ),
            ("devtools.json", mobile_devtools()),
            ("provided.json", provided),
        ] {
            let mut expected = vec![
                canonical(&settings.to_string()),
                canonical(&with_category(&settings, "performance")),
            ];
            expected.sort();
            assert_eq!(parameter_sets(&convert(fixture)), expected, "{fixture}");
        }
    }

    /// A page is named by its requested URL, the port dropped for a loopback host only.
    ///
    /// Kills: the port kept on a loopback host, the port dropped on any host, and a host matched
    /// by its prefix.
    #[test]
    fn web_lighthouse_drops_a_loopback_port() {
        for (url, name) in [
            ("http://localhost:8080/", "http://localhost/"),
            (
                "http://127.0.0.1:46263/about.html",
                "http://127.0.0.1/about.html",
            ),
            ("http://[::1]:8080/a?b=1#c", "http://[::1]/a?b=1#c"),
            (
                "http://user:pass@localhost:8080/",
                "http://user:pass@localhost/",
            ),
            ("http://localhost/", "http://localhost/"),
            ("https://example.com:8443/", "https://example.com:8443/"),
            (
                "http://localhost.example.com:8080/",
                "http://localhost.example.com:8080/",
            ),
            ("http://[::2]:8080/", "http://[::2]:8080/"),
        ] {
            assert_eq!(page_name(url), name, "{url}");
        }
        let results = convert_edited("perf_only.json", |result| {
            result["requestedUrl"] = "https://example.com:8443/".into();
        });
        assert!(
            results
                .inner
                .keys()
                .all(|name| name.to_string() == "https://example.com:8443/"),
            "{:?}",
            results.inner.keys()
        );
    }

    /// A result with a runtime error is refused, and the 400 names Lighthouse's error code and
    /// the page, under `web_lighthouse` and `magic` alike; another adapter keeps the generic
    /// error.
    ///
    /// Kills: the failed run read as an empty run with skips, the code dropped from the 400, and
    /// `magic` falling back to the generic error.
    #[test]
    fn web_lighthouse_names_a_runtime_error_in_the_400() {
        let input = read("not_found.json").to_string();
        let settings = Settings::new(None, BmfVersion::V1);
        for adapter in [Adapter::WebLighthouse, Adapter::Magic] {
            let error = AdapterResultsArray::new(&[&input], adapter, settings)
                .expect_err("expected a runtime error to be refused");
            let AdapterError::FailedRun {
                adapter: failed,
                reason,
            } = &error
            else {
                panic!("expected a failed run from {adapter}: {error}");
            };
            assert_eq!(*failed, Adapter::WebLighthouse);
            assert!(
                reason.contains("ERRORED_DOCUMENT_REQUEST")
                    && reason.contains("http://localhost:8080/missing.html")
                    && reason.contains("(Status code: 404)"),
                "{reason}"
            );
        }
        let error = AdapterResultsArray::new(&[&input], Adapter::RustBench, settings)
            .expect_err("expected rust_bench to refuse a Lighthouse result");
        assert!(matches!(error, AdapterError::Convert(_)), "{error}");
    }

    /// An audit in `error` is skipped and counted, and one that is `notApplicable` is skipped
    /// uncounted though it holds a value, each audit taken from a real result: the 404 run's
    /// errored `total-blocking-time`, and the timespan's `bootup-time`, `notApplicable` with a
    /// value of 0. No navigation capture has either among the metric audits.
    ///
    /// Kills: an errored audit skipped silently, a `notApplicable` audit's value written, and a
    /// `notApplicable` audit counted.
    #[test]
    fn web_lighthouse_counts_an_errored_audit_only() {
        let errored = read("not_found.json")["audits"]["total-blocking-time"].clone();
        let not_applicable = read("timespan.json")["audits"]["bootup-time"].clone();
        let results = convert_edited("perf_only.json", |result| {
            result["audits"]["total-blocking-time"] = errored;
            result["audits"]["bootup-time"] = not_applicable;
        });
        let settings = mobile_simulate().to_string();
        for measure in [TotalBlockingTime::SLUG_STR, ScriptExecutionTime::SLUG_STR] {
            assert_eq!(
                value(&results, PAGE, &settings, measure, "value"),
                None,
                "{measure}"
            );
        }
        assert_eq!(
            results.skipped,
            Skipped {
                measures: 1,
                ..Skipped::default()
            }
        );
    }

    /// An audit whose unit is not the one its measure takes is skipped and counted.
    ///
    /// Kills: a value read in a unit its measure does not take.
    #[test]
    fn web_lighthouse_counts_an_audit_in_another_unit() {
        let results = convert_edited("perf_only.json", |result| {
            result["audits"]["first-contentful-paint"]["numericUnit"] = "byte".into();
        });
        assert_eq!(
            value(
                &results,
                PAGE,
                &mobile_simulate().to_string(),
                FirstContentfulPaint::SLUG_STR,
                "value"
            ),
            None
        );
        assert_eq!(results.skipped.measures, 1);
    }

    /// A category whose score is null, as the 404 run's performance score is, is skipped and
    /// counted.
    ///
    /// Kills: a null score written as 0, or skipped silently.
    #[test]
    fn web_lighthouse_counts_a_null_category_score() {
        let null = read("not_found.json")["categories"]["performance"].clone();
        let results = convert_edited("perf_only.json", |result| {
            result["categories"]["performance"] = null;
        });
        assert_eq!(
            category_score(&results, &mobile_simulate(), "performance"),
            None
        );
        assert_eq!(variants(&results), 1);
        assert_eq!(results.skipped.measures, 1);
    }

    /// A runtime error refuses a result even where audits hold values, as they can beside one
    /// Lighthouse raises after the page loaded, such as `NO_FCP` (`core/lib/lh-error.js:277-281`);
    /// the 404 run's real runtime error is put on a measured result, since its own audits all
    /// errored.
    ///
    /// Kills: a runtime error ignored where some audits hold values.
    #[test]
    fn web_lighthouse_refuses_any_result_with_a_runtime_error() {
        let runtime_error = read("not_found.json")["runtimeError"].clone();
        let mut result = read("perf_only.json");
        result["runtimeError"] = runtime_error;
        let input = result.to_string();
        let settings = Settings::new(None, BmfVersion::V1);
        assert_eq!(Adapter::WebLighthouse.convert(&input, settings), None);
        let error = AdapterResultsArray::new(&[&input], Adapter::WebLighthouse, settings)
            .expect_err("expected a runtime error to be refused");
        assert!(matches!(error, AdapterError::FailedRun { .. }), "{error}");
    }

    /// A page whose URL is too long to be a benchmark name is skipped and counted.
    ///
    /// Kills: a page with no valid name dropped silently.
    #[test]
    fn web_lighthouse_counts_a_page_with_no_valid_name() {
        let results = convert_edited("perf_only.json", |result| {
            result["requestedUrl"] =
                format!("http://localhost:8080/?q={}", "a".repeat(1024)).into();
        });
        assert!(results.inner.is_empty(), "{:?}", results.inner.keys());
        assert_eq!(
            results.skipped,
            Skipped {
                benchmarks: 1,
                ..Skipped::default()
            }
        );
    }

    /// A category whose id is too long to be a parameter value, as a plugin's own id can be, is
    /// skipped and counted, and the page's other variants still land.
    ///
    /// Kills: a variant with invalid parameters dropped silently.
    #[test]
    fn web_lighthouse_counts_a_category_with_no_valid_id() {
        let id = "lighthouse-plugin-".repeat(4);
        let results = convert_edited("perf_only.json", |result| {
            let performance = result["categories"]["performance"].clone();
            result["categories"][id.as_str()] = performance;
        });
        assert_eq!(variants(&results), 2);
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 1,
                ..Skipped::default()
            }
        );
    }

    /// A timespan result is refused by its gather mode alone, though Lighthouse writes no
    /// `requestedUrl` there, so the real timespan is given one.
    ///
    /// Kills: a result of any gather mode read as a navigation.
    #[test]
    fn web_lighthouse_refuses_a_timespan_by_its_gather_mode() {
        let mut timespan = read("timespan.json");
        timespan["requestedUrl"] = "http://localhost:8080/".into();
        assert_eq!(
            Adapter::WebLighthouse
                .convert(&timespan.to_string(), Settings::new(None, BmfVersion::V1)),
            None
        );
        let mut navigation = timespan;
        navigation["gatherMode"] = "navigation".into();
        assert!(
            Adapter::WebLighthouse
                .convert(&navigation.to_string(), Settings::new(None, BmfVersion::V1))
                .is_some()
        );
    }
}
