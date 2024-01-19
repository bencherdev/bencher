use bencher_json::{
    project::threshold::{JsonNewStatistic, StatisticKind},
    Boundary, SampleSize,
};

use crate::{BoundaryError, IqrBoundary, NormalBoundary, PercentageBoundary};

pub fn validate_statistic(statistic: JsonNewStatistic) -> Result<(), BoundaryError> {
    let JsonNewStatistic {
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
    } = statistic;
    match test {
        StatisticKind::Static => {
            if let Some(&min_sample_size) = min_sample_size.as_ref() {
                return Err(BoundaryError::StaticMinSampleSize(min_sample_size));
            } else if let Some(&max_sample_size) = max_sample_size.as_ref() {
                return Err(BoundaryError::StaticMaxSampleSize(max_sample_size));
            } else if let Some(&window) = window.as_ref() {
                return Err(BoundaryError::StaticWindow(window));
            }

            match (lower_boundary.as_ref(), upper_boundary.as_ref()) {
                (Some(&lower), Some(&upper)) => {
                    if f64::from(lower) > f64::from(upper) {
                        Err(BoundaryError::Boundaries { lower, upper })
                    } else {
                        Ok(())
                    }
                },
                (Some(_), None) | (None, Some(_)) => Ok(()),
                (None, None) => Err(BoundaryError::StaticNoBoundary),
            }
        },
        StatisticKind::Percentage => {
            validate_sample_size(min_sample_size, max_sample_size)?;
            validate_boundary::<PercentageBoundary>(lower_boundary, upper_boundary)
        },
        StatisticKind::ZScore | StatisticKind::TTest | StatisticKind::LogNormal => {
            validate_sample_size(min_sample_size, max_sample_size)?;
            validate_boundary::<NormalBoundary>(lower_boundary, upper_boundary)
        },
        StatisticKind::Iqr | StatisticKind::DeltaIqr => {
            validate_sample_size(min_sample_size, max_sample_size)?;
            validate_boundary::<IqrBoundary>(lower_boundary, upper_boundary)
        },
    }
}

fn validate_sample_size(
    min_sample_size: Option<SampleSize>,
    max_sample_size: Option<SampleSize>,
) -> Result<(), BoundaryError> {
    if let (Some(min), Some(max)) = (min_sample_size, max_sample_size) {
        if u32::from(min) > u32::from(max) {
            return Err(BoundaryError::SampleSizes { min, max });
        }
    }

    Ok(())
}

fn validate_boundary<B>(
    lower: Option<Boundary>,
    upper: Option<Boundary>,
) -> Result<(), BoundaryError>
where
    B: TryFrom<Boundary, Error = BoundaryError>,
    f64: From<B>,
{
    match (lower, upper) {
        (Some(lower), Some(upper)) => {
            let l = B::try_from(lower)?;
            let u = B::try_from(upper)?;
            if f64::from(l) > f64::from(u) {
                Err(BoundaryError::Boundaries { lower, upper })
            } else {
                Ok(())
            }
        },
        (Some(boundary), None) | (None, Some(boundary)) => B::try_from(boundary).map(|_| ()),
        (None, None) => Err(BoundaryError::NormalNoBoundary),
    }
}
