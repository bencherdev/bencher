use std::collections::BTreeMap;

use crate::cli::adapter::Report;
use crate::cli::benchmark::Output;
use crate::BencherError;

pub fn parse(adapter: &str, output: Output) -> Result<Report, BencherError> {
    println!("{:?}", adapter);
    println!("{:?}", output);

    Ok(Report::new(BTreeMap::new()))
}
