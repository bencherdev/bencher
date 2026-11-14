use bencher_json::MeasureNameId;
use serde::{Deserialize, Deserializer};

use crate::adapters::bmf_v1::{finite::Finite, leaf::Variant};

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
    use bencher_json::{
        ParameterSet,
        project::{
            measure::built_in::{BuiltInMeasure as _, default::Latency},
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use super::Statistics;
    use crate::{
        adapters::bmf_v1::{leaf::LeafResults, test_util::assert_variant},
        results::adapter_results::AdapterResults,
    };

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
