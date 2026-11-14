use bencher_json::{
    BenchmarkName, MeasureNameId, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Latency, Throughput},
        },
        report::Adapter,
    },
};
use serde::{Deserialize, Deserializer};

use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::{
            finite::Finite,
            leaf::{LeafResults, Variant},
        },
        util::Units,
    },
    results::adapter_results::AdapterResults,
};

/// tinybench 6's tasks as an array of each task's name beside its result, the shape Vitest 5
/// writes a task in.
///
/// Every entry must carry tinybench 6's `state`; a task with no statistics is skipped and counted.
pub struct AdapterJsTinybench;

impl Adaptable for AdapterJsTinybench {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let entries: Vec<serde_json::Value> = serde_json::from_str(input).ok()?;
        let tasks = entries
            .iter()
            .map(|entry| {
                let task = Task::deserialize(entry).ok()?;
                Some((task, entry))
            })
            .collect::<Option<Vec<_>>>()?;
        let mut results = LeafResults::default();
        for (Task { name, state }, entry) in tasks {
            let statistics = matches!(state.as_str(), "completed" | "aborted-with-statistics")
                .then(|| TaskStatistics::deserialize(entry).ok())
                .flatten()
                .filter(|statistics| statistics.latency.mean().is_some());
            let (Some(statistics), Ok(benchmark_name)) =
                (statistics, name.parse::<BenchmarkName>())
            else {
                results.skip_benchmark();
                continue;
            };
            let Some(mut variant) = results.new_variant(benchmark_name, ParameterSet::default())
            else {
                continue;
            };
            statistics
                .latency
                .insert(&mut variant, &Latency::name_id(), Units::Milli.as_nanos());
            statistics
                .throughput
                .insert(&mut variant, &Throughput::name_id(), 1.0);
        }
        results.finish_with_skips(Adapter::JsTinybench)
    }
}

#[derive(Deserialize)]
struct Task {
    name: String,
    state: String,
}

#[derive(Deserialize)]
struct TaskStatistics {
    latency: Statistics,
    throughput: Statistics,
}

/// tinybench's statistics of a task's samples: an object per quantity in tinybench 6, as Vitest 5
/// writes it, and the latency's among the benchmark's own fields in tinybench 2, as Vitest 1 to 4
/// write it, with the median Vitest adds as `median`.
///
/// A field the object lacks is `None`, and one it holds as `null`, as `JSON.stringify` writes a
/// value that is not finite, is a [`Finite`] with no number.
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct Statistics {
    #[serde(default, deserialize_with = "present")]
    mean: Option<Finite>,
    #[serde(default, deserialize_with = "present", alias = "median")]
    p50: Option<Finite>,
    #[serde(default, deserialize_with = "present")]
    min: Option<Finite>,
    #[serde(default, deserialize_with = "present")]
    max: Option<Finite>,
    #[serde(default, deserialize_with = "present")]
    p75: Option<Finite>,
    #[serde(default, deserialize_with = "present")]
    p99: Option<Finite>,
    #[serde(default, deserialize_with = "present")]
    p995: Option<Finite>,
    #[serde(default, deserialize_with = "present")]
    p999: Option<Finite>,
    #[serde(default, deserialize_with = "present")]
    sd: Option<Finite>,
    #[serde(default, deserialize_with = "present")]
    sem: Option<Finite>,
    #[serde(default, deserialize_with = "present")]
    moe: Option<Finite>,
    #[serde(default, deserialize_with = "present")]
    mad: Option<Finite>,
    #[serde(default, deserialize_with = "present")]
    aad: Option<Finite>,
}

impl Statistics {
    pub(crate) fn mean(&self) -> Option<Finite> {
        self.mean
    }

    /// Every statistic in `measure`, times `factor`, the mean's interval at the 95% level that
    /// tinybench fixes with its two-sided Student's t critical value.
    pub(crate) fn insert(&self, variant: &mut Variant<'_>, measure: &MeasureNameId, factor: f64) {
        let Self {
            mean,
            p50,
            min,
            max,
            p75,
            p99,
            p995,
            p999,
            sd,
            sem,
            moe,
            mad,
            aad,
        } = *self;
        for (name, value) in [
            ("mean", mean),
            ("median", p50),
            ("min", min),
            ("max", max),
            ("p75", p75),
            ("p99", p99),
            ("p99_5", p995),
            ("p99_9", p999),
        ] {
            if let Some(value) = value {
                value.insert(variant, measure, name, factor);
            }
        }
        let mean = mean.unwrap_or_default();
        let interval = moe.map(|moe| {
            let (lower, upper) = match (mean.get(), moe.get()) {
                (Some(mean), Some(moe)) => (Some(mean - moe), Some(mean + moe)),
                _ => (None, None),
            };
            [
                ("mean_ci95_lower", Finite::from(lower)),
                ("mean_ci95_upper", Finite::from(upper)),
                ("mean_ci95_margin", moe),
            ]
        });
        for (name, spread) in [
            ("std_dev", sd),
            ("std_err", sem),
            ("median_abs_dev", mad),
            ("mean_abs_dev", aad),
        ]
        .into_iter()
        .chain(
            interval
                .into_iter()
                .flatten()
                .map(|(name, spread)| (name, Some(spread))),
        ) {
            if let Some(spread) = spread {
                spread.insert_spread(mean, variant, measure, name, factor);
            }
        }
    }
}

/// A field that is present, even as `null`.
fn present<'de, D>(deserializer: D) -> Result<Option<Finite>, D::Error>
where
    D: Deserializer<'de>,
{
    Finite::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod test_tinybench {
    use std::collections::BTreeSet;

    use bencher_json::{
        BmfVersion, ParameterSet,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Latency, Throughput},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use super::Statistics;
    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::{
            leaf::LeafResults,
            test_util::{
                assert_ignores_average, assert_value, assert_variant, convert_edited_v1, convert_v1,
            },
        },
        results::adapter_results::AdapterResults,
    };

    fn names(results: &AdapterResults) -> BTreeSet<String> {
        results.inner.keys().map(ToString::to_string).collect()
    }

    /// Kills an errored or aborted task kept, or left uncounted, a task name changed, and one
    /// errored task that refuses the whole array.
    #[test]
    fn adapter_js_tinybench_states() {
        let results = convert_v1(Adapter::JsTinybench, "js/tinybench/shapes.json");
        assert_eq!(results.resolved_adapter, Some(Adapter::JsTinybench));
        assert_eq!(results.skipped.benchmarks, 3);
        assert_eq!(results.skipped.variants, 0);
        assert_eq!(
            names(&results),
            ["noop", "retained samples", "sleep 20ms", "sort 1000"]
                .map(ToOwned::to_owned)
                .into()
        );
    }

    /// Kills latency left in milliseconds, a statistic dropped or misnamed, an interval not
    /// centred on the mean, and throughput read from latency.
    #[test]
    fn adapter_js_tinybench_statistics() {
        let results = convert_v1(Adapter::JsTinybench, "js/tinybench/shapes.json");
        assert_variant(
            &results,
            "sort 1000",
            "{}",
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("mean", 84_283.872_053_874_54),
                        ("median", 81_991.000_000_016_36),
                        ("min", 80_333.999_999_766_09),
                        ("max", 161_497.999_999_937_75),
                        ("p75", 83_891.499_999_822_38),
                        ("p99", 119_620.040_000_059_29),
                        ("p99_5", 129_927.440_000_287_7),
                        ("p99_9", 154_912.141_999_981_45),
                        ("std_dev", 7_551.320_260_058_81),
                        ("std_err", 309.834_423_224_360_5),
                        ("median_abs_dev", 706.000_000_036_510_8),
                        ("mean_abs_dev", 3_718.796_823_452_833),
                        ("mean_ci95_lower", 83_675.365_773_100_16),
                        ("mean_ci95_upper", 84_892.378_334_648_92),
                        ("mean_ci95_margin", 608.506_280_774_385_6),
                    ],
                ),
                (
                    Throughput::SLUG_STR,
                    vec![
                        ("mean", 11_931.158_530_683_306),
                        ("median", 12_196.460_632_491_879),
                        ("min", 6_192.027_145_849_394),
                        ("max", 12_448.029_476_970_047),
                        ("p75", 12_278.186_880_507_368),
                        ("p99", 12_388.534_609_754_644),
                        ("p99_5", 12_408.349_065_863_367),
                        ("p99_9", 12_438.943_822_215_722),
                        ("std_dev", 757.876_098_222_718_4),
                        ("std_err", 31.096_032_969_277_875),
                        ("median_abs_dev", 105.002_248_135_562_7),
                        ("mean_abs_dev", 440.011_648_866_515_26),
                        ("mean_ci95_lower", 11_870.086_777_673_89),
                        ("mean_ci95_upper", 11_992.230_283_692_721),
                        ("mean_ci95_margin", 61.071_753_009_415_83),
                    ],
                ),
            ],
        );
    }

    /// Kills a task aborted after it took samples skipped, a repeated name that overwrites the
    /// first task or goes uncounted, and a task of zero duration, whose `rme` is `null`, counted
    /// as skipped.
    #[test]
    fn adapter_js_tinybench_benches() {
        let results = convert_v1(Adapter::JsTinybench, "js/tinybench/benches.json");
        assert_eq!(results.skipped.benchmarks, 3);
        assert_eq!(results.skipped.variants, 1);
        assert_eq!(results.skipped.metrics, 0);
        assert_value(
            &results,
            ("noop", "{}"),
            (Latency::SLUG_STR, "mean"),
            31.021_510_232_908_8,
        );
        assert_value(
            &results,
            ("aborted with statistics", "{}"),
            (Throughput::SLUG_STR, "mean"),
            476.310_367_281_865_75,
        );
        assert_value(
            &results,
            ("zero duration", "{}"),
            (Latency::SLUG_STR, "mean_ci95_margin"),
            0.0,
        );
    }

    /// Kills a leaf that claims an array whose entries lack tinybench 6's `state`, such as a
    /// tinybench 2 or a tinybench 3 to 5 result list.
    #[test]
    fn adapter_js_tinybench_requires_state() {
        let input = std::fs::read_to_string("./tool_output/js/tinybench/shapes.json")
            .expect("Failed to read a fixture");
        let mut json: serde_json::Value = serde_json::from_str(&input).expect("Failed to parse");
        json[1]
            .as_object_mut()
            .expect("Failed to edit")
            .remove("state");
        assert_eq!(
            Adapter::JsTinybench.convert(&json.to_string(), Settings::new(None, BmfVersion::V1)),
            None
        );
    }

    /// Kills a completed task with no latency mean read as a benchmark with no metrics.
    #[test]
    fn adapter_js_tinybench_requires_mean() {
        let results = convert_edited_v1(Adapter::JsTinybench, "js/tinybench/shapes.json", |json| {
            json[0]["latency"]
                .as_object_mut()
                .expect("Failed to edit")
                .remove("mean");
        });
        assert_eq!(results.skipped.benchmarks, 4);
        assert!(results.get("noop").is_none());
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_js_tinybench_ignores_average() {
        for suffix in ["js/tinybench/shapes.json", "js/tinybench/benches.json"] {
            assert_ignores_average(Adapter::JsTinybench, suffix);
        }
    }

    fn convert(statistics: &str) -> AdapterResults {
        let statistics: Statistics = serde_json::from_str(statistics).expect("Failed to parse");
        let mut results = LeafResults::default();
        let mut variant = results.variant(
            "task".parse().expect("Failed to parse"),
            ParameterSet::default(),
        );
        statistics.insert(&mut variant, &Latency::name_id(), 1.0);
        results.finish(Adapter::JsVitestV5).expect("no results")
    }

    /// Kills a spread written as `null` beside a finite mean counted as skipped, and a mean
    /// written as `null`, or a spread beside it, left uncounted. tinybench writes a value that is
    /// not finite as `null`, as its `rme` when the mean is 0.
    #[test]
    fn tinybench_statistics_not_finite() {
        let results = convert(
            r#"{"mean":0.5,"p50":0.4,"sd":null,"sem":null,"moe":null,"mad":null,"aad":null,"rme":null}"#,
        );
        assert_eq!(results.skipped.metrics, 0);
        assert_variant(
            &results,
            "task",
            "{}",
            &[(Latency::SLUG_STR, vec![("mean", 0.5), ("median", 0.4)])],
        );
        let results = convert(r#"{"mean":null,"p50":0.4,"sd":null}"#);
        assert_eq!(results.skipped.metrics, 2);
        assert_variant(
            &results,
            "task",
            "{}",
            &[(Latency::SLUG_STR, vec![("median", 0.4)])],
        );
    }
}
