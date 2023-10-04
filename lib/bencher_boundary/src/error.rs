#[derive(Debug, thiserror::Error)]
pub enum BoundaryError {
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
}
