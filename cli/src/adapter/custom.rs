use std::collections::HashMap;

use crate::error::CliError;
use crate::output::Output;
use crate::report::Report;

pub fn parse(adapter: String, output: Output) -> Result<Report, CliError> {
    println!("{:?}", adapter);
    println!("{:?}", output);

    Ok(Report::new(HashMap::new()))
}
