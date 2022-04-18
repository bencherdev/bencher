use std::collections::BTreeMap;

use reports::Report;

use crate::cli::benchmark::BenchmarkOutput;
use crate::BencherError;

pub fn parse(adapter: &str, output: BenchmarkOutput) -> Result<Report, BencherError> {
    println!("{:?}", adapter);
    println!("{:?}", output);

    Ok(Report::from(BTreeMap::new()))
}
