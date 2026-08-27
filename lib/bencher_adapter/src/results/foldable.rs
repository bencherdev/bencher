use std::collections::HashMap;

use bencher_json::{
    BenchmarkNameId, JsonNewMetric, MeasureNameId,
    project::{metric::Mean, report::JsonFold},
};

use super::{CombinedKind, OrdKind, results_reducer::ResultsReducer};

/// A BMF v0 results payload: one grid point per benchmark, on the empty
/// parameter set, with every measure spelling out a metric triple.
///
/// Fold is defined here and nowhere else, so a BMF v1 payload can never be
/// folded: the mean of per iteration `p99` values is not the `p99` of the pooled
/// sample, and this product does not invent that statistic. The only way in is
/// [`super::AdapterResultsArray::foldable`], which refuses a v1 payload outright.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FoldableResults {
    pub inner: FoldableMap,
}

pub type FoldableMap = HashMap<BenchmarkNameId, FoldableMetrics>;

pub type FoldableMetrics = HashMap<MeasureNameId, JsonNewMetric>;

impl From<FoldableMap> for FoldableResults {
    fn from(inner: FoldableMap) -> Self {
        Self { inner }
    }
}

impl FoldableResults {
    fn combined(self, mut other: Self, kind: CombinedKind) -> Self {
        let mut fold_map = FoldableMap::new();
        for (benchmark_name, metrics) in self.inner {
            let other_metrics = other.inner.remove(&benchmark_name);
            let combined_metrics = if let Some(other_metrics) = other_metrics {
                combine_metrics(metrics, other_metrics, kind)
            } else {
                metrics
            };
            fold_map.insert(benchmark_name, combined_metrics);
        }
        fold_map.extend(other.inner);
        fold_map.into()
    }
}

fn combine_metrics(
    metrics: FoldableMetrics,
    mut other: FoldableMetrics,
    kind: CombinedKind,
) -> FoldableMetrics {
    let mut metric_map = FoldableMetrics::new();
    for (measure, metric) in metrics {
        let other_metric = other.remove(&measure);
        let combined_metric = if let Some(other_metric) = other_metric {
            match kind {
                CombinedKind::Ord(ord_kind) => match ord_kind {
                    OrdKind::Min => metric.min(other_metric),
                    OrdKind::Max => metric.max(other_metric),
                },
                CombinedKind::Add => metric + other_metric,
            }
        } else {
            metric
        };
        metric_map.insert(measure, combined_metric);
    }
    metric_map.extend(other);
    metric_map
}

impl std::ops::Add for FoldableResults {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        self.combined(rhs, CombinedKind::Add)
    }
}

impl std::iter::Sum for FoldableResults {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.into_iter()
            .fold(FoldableMap::new().into(), |results: Self, other| {
                results + other
            })
    }
}

impl std::ops::Div<usize> for FoldableResults {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        self.inner
            .into_iter()
            .map(|(benchmark_name, metrics)| {
                (
                    benchmark_name,
                    metrics
                        .into_iter()
                        .map(|(measure, metric)| (measure, metric / rhs))
                        .collect(),
                )
            })
            .collect::<FoldableMap>()
            .into()
    }
}

impl Mean for FoldableResults {}

/// Every results payload in a report, once each has been proven BMF v0.
#[derive(Debug, Clone)]
pub struct FoldableResultsArray {
    pub inner: Vec<FoldableResults>,
}

impl FoldableResultsArray {
    pub fn min(self) -> FoldableResults {
        self.ord(OrdKind::Min)
    }

    pub fn max(self) -> FoldableResults {
        self.ord(OrdKind::Max)
    }

    fn ord(self, ord_kind: OrdKind) -> FoldableResults {
        self.inner.into_iter().fold(
            FoldableMap::new().into(),
            |results: FoldableResults, other_results| {
                results.combined(other_results, CombinedKind::Ord(ord_kind))
            },
        )
    }

    pub fn mean(self) -> FoldableResults {
        FoldableResults::mean(self.inner).unwrap_or_default()
    }

    pub fn median(self) -> FoldableResults {
        ResultsReducer::from(self)
            .inner
            .into_iter()
            .map(|(benchmark, measures)| (benchmark, measures.median()))
            .collect::<FoldableMap>()
            .into()
    }

    pub fn fold(self, fold: JsonFold) -> FoldableResults {
        if self.inner.is_empty() {
            return FoldableResults::default();
        }

        match fold {
            JsonFold::Min => self.min(),
            JsonFold::Max => self.max(),
            JsonFold::Mean => self.mean(),
            JsonFold::Median => self.median(),
        }
    }
}

#[cfg(test)]
mod test_foldable {
    use bencher_json::{
        BmfVersion,
        project::report::{Adapter, JsonFold},
    };
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use super::FoldableResults;
    use crate::{AdapterResultsArray, Settings};

    const V0_TEN: &str = r#"{"bench": {"latency": {"value": 10.0, "lower_value": 9.0}}}"#;
    const V0_TWENTY: &str = r#"{"bench": {"latency": {"value": 20.0, "lower_value": 19.0}}}"#;
    const V1_P99: &str = r#"{"bench": [{"measures": {"latency": {"p99": 10.0}}}]}"#;
    const V1_TRIPLE: &str = r#"{"bench": [{"measures": {"latency": {"value": 20.0}}}]}"#;
    /// The one payload both JSON leaves claim, and so the one payload whose parsed
    /// version the declared version moves.
    const EMPTY: &str = "{}";

    fn results_array(results_array: &[&str]) -> AdapterResultsArray {
        results_array_at(results_array, BmfVersion::V0)
    }

    fn results_array_at(results_array: &[&str], bmf_version: BmfVersion) -> AdapterResultsArray {
        AdapterResultsArray::new(
            results_array,
            Adapter::Json,
            Settings::new(None, bmf_version),
        )
        .expect("Failed to convert results")
    }

    fn latency(results: &FoldableResults) -> OrderedFloat<f64> {
        results
            .inner
            .get(&"bench".parse().expect("Failed to parse benchmark name"))
            .expect("Missing benchmark")
            .get(&"latency".parse().expect("Failed to parse measure name"))
            .expect("Missing measure")
            .value
    }

    /// Fold over BMF v0 across iterations is exactly what it has always been.
    #[test]
    fn fold_v0_across_iterations() {
        for (fold, expected) in [
            (JsonFold::Min, 10.0),
            (JsonFold::Max, 20.0),
            (JsonFold::Mean, 15.0),
            (JsonFold::Median, 15.0),
        ] {
            let foldable = results_array(&[V0_TEN, V0_TWENTY])
                .foldable()
                .expect("BMF v0 results are foldable");
            assert_eq!(latency(&foldable.fold(fold)), OrderedFloat::from(expected));
        }
    }

    /// Fold is not supported for BMF v1: the mean of per iteration `p99` values
    /// is not the `p99` of the pooled sample. Refusal is by construction, since
    /// `fold` exists only on the foldable array a v1 payload cannot become.
    #[test]
    fn fold_refuses_v1() {
        drop(results_array(&[V1_P99]).foldable().unwrap_err());
    }

    /// Refusal keys on the payload version, not on which names a measure happens
    /// to carry: a v1 payload spelling only the conventional names is still v1.
    #[test]
    fn fold_refuses_v1_conventional_names() {
        drop(results_array(&[V1_TRIPLE]).foldable().unwrap_err());
    }

    /// One v1 payload poisons the whole array, since fold spans every iteration.
    #[test]
    fn fold_refuses_a_mixed_array() {
        drop(results_array(&[V0_TEN, V1_P99]).foldable().unwrap_err());
        drop(results_array(&[V1_P99, V0_TEN]).foldable().unwrap_err());
    }

    /// An empty payload is foldable at either declared version.
    ///
    /// At version 1 the empty payload parses through the v1 leaf, so without an
    /// explicit carve out it would read as a v1 payload and, since fold is all or
    /// nothing across the array, disable fold for every v0 iteration beside it.
    /// The refusal exists because a pooled statistic cannot be recomputed from per
    /// iteration values, and a payload that reported nothing has none, so it has
    /// nothing to refuse over.
    #[test]
    fn fold_accepts_an_empty_payload_at_either_version() {
        for fold in [
            JsonFold::Min,
            JsonFold::Max,
            JsonFold::Mean,
            JsonFold::Median,
        ] {
            let at_zero = results_array_at(&[EMPTY, V0_TEN, V0_TWENTY], BmfVersion::V0)
                .foldable()
                .expect("an empty payload beside v0 results is foldable at version 0")
                .fold(fold);
            let at_one = results_array_at(&[EMPTY, V0_TEN, V0_TWENTY], BmfVersion::V1)
                .foldable()
                .expect("an empty payload beside v0 results is foldable at version 1")
                .fold(fold);
            assert_eq!(at_one, at_zero, "{fold:?}");
        }
    }

    /// A pure v0 array folds to the same thing at either declared version.
    #[test]
    fn fold_v0_is_unmoved_by_the_declared_version() {
        for fold in [
            JsonFold::Min,
            JsonFold::Max,
            JsonFold::Mean,
            JsonFold::Median,
        ] {
            let at_zero = results_array_at(&[V0_TEN, V0_TWENTY], BmfVersion::V0)
                .foldable()
                .expect("BMF v0 results are foldable")
                .fold(fold);
            let at_one = results_array_at(&[V0_TEN, V0_TWENTY], BmfVersion::V1)
                .foldable()
                .expect("BMF v0 results are foldable")
                .fold(fold);
            assert_eq!(at_one, at_zero, "{fold:?}");
        }
    }

    /// A payload that actually reported something as v1 is still refused, and the
    /// declared version does not change that. Refusal keys on what was parsed.
    #[test]
    fn fold_refuses_v1_at_either_version() {
        for bmf_version in [BmfVersion::V0, BmfVersion::V1] {
            drop(
                results_array_at(&[V1_P99], bmf_version)
                    .foldable()
                    .unwrap_err(),
            );
            drop(
                results_array_at(&[V1_TRIPLE], bmf_version)
                    .foldable()
                    .unwrap_err(),
            );
            drop(
                results_array_at(&[EMPTY, V1_P99], bmf_version)
                    .foldable()
                    .unwrap_err(),
            );
        }
    }

    /// A refused array comes back untouched, so the caller can ingest unfolded.
    #[test]
    fn fold_hands_back_a_refused_array() {
        let results_array = results_array(&[V0_TEN, V1_P99]);
        let expected = results_array.inner.clone();
        let returned = results_array
            .foldable()
            .expect_err("BMF v1 is not foldable");
        assert_eq!(returned.inner, expected);
    }
}
