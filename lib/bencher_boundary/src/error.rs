use bencher_json::{Boundary, SampleSize, Window};

use crate::{IqrBoundary, NormalBoundary, PercentageBoundary};

#[derive(Debug, thiserror::Error)]
pub enum BoundaryError {
    #[error("Invalid statistic, minimum sample size ({min}) is greater than maximum sample size ({max})")]
    SampleSizes { min: SampleSize, max: SampleSize },
    #[error(
        "Invalid statistic, lower boundary ({lower}) is greater than upper boundary ({upper})"
    )]
    Boundaries { lower: Boundary, upper: Boundary },

    #[error("Invalid static statistic, includes a minimum sample size: {0}")]
    StaticMinSampleSize(SampleSize),
    #[error("Invalid static statistic, includes a maximum sample size: {0}")]
    StaticMaxSampleSize(SampleSize),
    #[error("Invalid static statistic, includes a sampling window: {0}")]
    StaticWindow(Window),
    #[error("Invalid static statistic, no boundary provided")]
    StaticNoBoundary,

    #[error("Invalid percentage boundary: {0}")]
    PercentageBoundary(f64),
    #[error("Invalid percentage statistic, lower boundary ({lower:?}) is greater than upper boundary ({upper:?})")]
    PercentageBoundaries {
        lower: PercentageBoundary,
        upper: PercentageBoundary,
    },
    #[error("Invalid percentage statistic, no boundary provided")]
    PercentageNoBoundary,

    #[error("Invalid statistical boundary: {0}")]
    NormalBoundary(f64),
    #[error("Invalid normal distribution statistic, lower boundary ({lower:?}) is greater than upper boundary ({upper:?})")]
    NormalBoundaries {
        lower: NormalBoundary,
        upper: NormalBoundary,
    },
    #[error("Invalid normal distribution statistic, no boundary provided")]
    NormalNoBoundary,
    #[error("Invalid inter-quartile range boundary: {0}")]
    IqrBoundary(f64),
    #[error("Invalid Normal Distribution (mean: {mean} | std dev: {std_dev}): {error}")]
    Normal {
        mean: f64,
        std_dev: f64,
        error: statrs::StatsError,
    },
    #[error("Invalid Student T Distribution (mean: {mean} | scale: {std_dev} | freedom: {freedom}): {error}")]
    StudentsT {
        mean: f64,
        std_dev: f64,
        freedom: f64,
        error: statrs::StatsError,
    },
    #[error("Invalid Log Normal Distribution (mean: {mean} | std dev: {std_dev}): {error}")]
    LogNormal {
        mean: f64,
        std_dev: f64,
        error: statrs::StatsError,
    },

    #[error("Invalid inter-quartile range statistic, lower boundary ({lower:?}) is greater than upper boundary ({upper:?})")]
    IqrBoundaries {
        lower: IqrBoundary,
        upper: IqrBoundary,
    },
    #[error("Invalid inter-quartile range statistic, no boundary provided")]
    IqrNoBoundary,
}
