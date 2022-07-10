use bencher_json::Metrics;

use crate::cli::benchmark::Output;
use crate::BencherError;

pub fn parse(output: &Output) -> Result<Metrics, BencherError> {
    serde_json::from_str(output.as_str()).map_err(BencherError::Serde)
}
