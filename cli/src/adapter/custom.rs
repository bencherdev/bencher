use std::collections::BTreeMap;

use crate::adapter::Report;
use crate::command::Output;
use crate::error::CliError;

pub fn parse(adapter: &str, output: Output) -> Result<Report, CliError> {
    println!("{:?}", adapter);
    println!("{:?}", output);

    Ok(Report::new(BTreeMap::new()))
}
