use std::collections::BTreeMap;

use reports::Metrics;

use crate::cli::benchmark::BenchmarkOutput;
use crate::BencherError;

pub fn parse(adapter: &str, output: BenchmarkOutput) -> Result<Metrics, BencherError> {
    println!("{:?}", adapter);
    println!("{:?}", output);

    Ok(BTreeMap::new())
}
