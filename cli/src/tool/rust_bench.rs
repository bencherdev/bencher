use std::collections::HashMap;
use std::process::Output;

use crate::error::CliError;
use crate::report::Report;

pub fn parse(output: Output) -> Result<Report, CliError> {
    println!("{:?}", output);
    Ok(Report::new(HashMap::new()))
}
