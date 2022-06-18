use reports::Metrics;

use crate::cli::benchmark::BenchmarkOutput;
use crate::BencherError;

pub fn parse(output: BenchmarkOutput) -> Result<Metrics, BencherError> {
    serde_json::from_str(&output.stdout).map_err(BencherError::Serde)
}
