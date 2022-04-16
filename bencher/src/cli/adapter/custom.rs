use std::collections::BTreeMap;

use crate::cli::adapter::Report;
use crate::cli::benchmark::Output;
use crate::error::CliError;

pub fn parse(adapter: &str, output: Output) -> Result<Report, CliError> {
    println!("{:?}", adapter);
    println!("{:?}", output);

    Ok(Report::new(BTreeMap::new()))
}
