use bencher_client::types::MeasureFilter;
use bencher_json::{
    MeasureNameId,
    project::report::{MeasureFilter as JsonMeasureFilter, MeasureFilterError},
};

#[derive(thiserror::Error, Debug)]
pub enum MeasuresError {
    #[error("Failed to keep the measures of `--measure`: {0}")]
    Filter(MeasureFilterError),
    #[error("Failed to send the measures of `--measure`: {0}")]
    Client(serde_json::Error),
}

/// The measures `--measure` keeps, or `None` to keep every measure.
pub fn measure_filter(
    measures: Vec<MeasureNameId>,
) -> Result<Option<MeasureFilter>, MeasuresError> {
    if measures.is_empty() {
        return Ok(None);
    }
    let measures = JsonMeasureFilter::try_from(measures).map_err(MeasuresError::Filter)?;
    // The client spells the list as plain JSON, so it travels through it.
    serde_json::to_value(measures)
        .and_then(serde_json::from_value)
        .map(Some)
        .map_err(MeasuresError::Client)
}

#[cfg(test)]
mod tests {
    use bencher_json::{MeasureNameId, project::report::MAX_MEASURE_FILTER_LEN};

    use super::{MeasuresError, measure_filter};

    fn measures(count: usize) -> Vec<MeasureNameId> {
        (0..count)
            .map(|n| format!("m{n}").parse().unwrap())
            .collect()
    }

    /// Kills: a `--measure` list past the limit that is refused only once the benchmarks have
    /// run, and a list that reaches the client other than as written.
    #[test]
    fn measure_filter_refuses_past_the_limit_before_sending() {
        let filter = measure_filter(measures(MAX_MEASURE_FILTER_LEN))
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::to_value(filter).unwrap(),
            serde_json::to_value(measures(MAX_MEASURE_FILTER_LEN)).unwrap()
        );
        assert!(matches!(
            measure_filter(measures(MAX_MEASURE_FILTER_LEN + 1)),
            Err(MeasuresError::Filter(_))
        ));
    }
}
