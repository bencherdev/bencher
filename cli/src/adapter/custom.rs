use std::collections::HashMap;

use crate::cli::Output;
use crate::error::CliError;
use crate::report::Report;

pub fn parse(adapter: &str, output: Output) -> Result<Report, CliError> {
    println!("{:?}", adapter);
    println!("{:?}", output);

    Ok(Report::new(HashMap::new()))
}
