use bencher_json::{
    BenchmarkName, BenchmarkNameId, BmfVersion, MeasureNameId, MetricName, ParameterSet,
    project::report::Adapter,
};

use crate::results::{
    adapter_metrics::AdapterMetrics,
    adapter_results::{AdapterResults, MeasureUnits, ResultsMap, Skipped},
};

/// The results one version 1 leaf builds, counting what it skips.
#[derive(Debug, Default)]
pub(crate) struct LeafResults {
    inner: ResultsMap,
    skipped: Skipped,
    units: MeasureUnits,
}

impl LeafResults {
    /// A benchmark's variant, recorded even if it never reports a metric.
    pub(crate) fn variant(
        &mut self,
        benchmark: BenchmarkName,
        parameters: ParameterSet,
    ) -> Variant<'_> {
        let metrics = self
            .inner
            .entry(BenchmarkNameId::new_name(benchmark))
            .or_default()
            .entry(parameters)
            .or_default();
        Variant {
            metrics,
            skipped: &mut self.skipped,
            units: &mut self.units,
        }
    }

    /// A benchmark's variant the leaf has not built yet, or `None`, counted as skipped, if it has.
    pub(crate) fn new_variant(
        &mut self,
        benchmark: BenchmarkName,
        parameters: ParameterSet,
    ) -> Option<Variant<'_>> {
        let variants = self
            .inner
            .entry(BenchmarkNameId::new_name(benchmark))
            .or_default();
        if variants.contains_key(&parameters) {
            self.skipped.variants += 1;
            return None;
        }
        Some(Variant {
            metrics: variants.entry(parameters).or_default(),
            skipped: &mut self.skipped,
            units: &mut self.units,
        })
    }

    pub(crate) fn skip_benchmark(&mut self) {
        self.skipped.benchmarks += 1;
    }

    pub(crate) fn skip_variant(&mut self) {
        self.skipped.variants += 1;
    }

    /// The results, recording `adapter` as the leaf that parsed them, or `None`
    /// when the input held no benchmark this leaf reads.
    pub(crate) fn finish(self, adapter: Adapter) -> Option<AdapterResults> {
        (!self.inner.is_empty()).then(|| self.into_results(adapter))
    }

    /// [`Self::finish`], also when the input held benchmarks or variants of this leaf's harness
    /// that it could only skip, so that their count still reaches the report.
    pub(crate) fn finish_with_skips(self, adapter: Adapter) -> Option<AdapterResults> {
        (!self.inner.is_empty() || self.skipped.benchmarks > 0 || self.skipped.variants > 0)
            .then(|| self.into_results(adapter))
    }

    fn into_results(self, adapter: Adapter) -> AdapterResults {
        let Self {
            inner,
            skipped,
            units,
        } = self;
        AdapterResults {
            inner,
            version: BmfVersion::V1,
            resolved_adapter: Some(adapter),
            skipped,
            units,
        }
    }
}

/// The metrics of one benchmark's variant.
pub(crate) struct Variant<'a> {
    metrics: &'a mut AdapterMetrics,
    skipped: &'a mut Skipped,
    units: &'a mut MeasureUnits,
}

impl Variant<'_> {
    pub(crate) fn insert(&mut self, measure: MeasureNameId, metric: &str, value: f64) {
        let Ok(metric) = metric.parse::<MetricName>() else {
            self.skipped.metrics += 1;
            return;
        };
        self.metrics
            .inner
            .entry(measure)
            .or_default()
            .inner
            .insert(metric, value.into());
    }

    /// A measure the harness names, in `unit`, or `None`, counted as skipped, if the
    /// name or the unit is not valid.
    pub(crate) fn named(&mut self, measure: &str, unit: &str) -> Option<MeasureNameId> {
        let (Ok(measure), Ok(unit)) = (measure.parse::<MeasureNameId>(), unit.parse()) else {
            self.skip_measure();
            return None;
        };
        self.units.insert(measure.clone(), unit);
        Some(measure)
    }

    pub(crate) fn skip_measure(&mut self) {
        self.skipped.measures += 1;
    }

    pub(crate) fn skip_metric(&mut self) {
        self.skipped.metrics += 1;
    }
}
