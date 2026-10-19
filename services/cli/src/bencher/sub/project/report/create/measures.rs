use bencher_client::types::MeasureFilter;
use bencher_json::MeasureNameId;

/// The measures `--measure` keeps, or `None` to keep every measure.
pub fn measure_filter(measures: Vec<MeasureNameId>) -> Option<MeasureFilter> {
    (!measures.is_empty()).then(|| {
        measures
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>()
            .into()
    })
}
