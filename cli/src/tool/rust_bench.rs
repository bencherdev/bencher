use std::collections::HashMap;

use nom::IResult;

use crate::error::CliError;
use crate::output::Output;
use crate::report::Report;

pub fn parse(output: Output) -> Result<Report, CliError> {
    println!("{:?}", output);

    let report = parse_stdout(&output.stdout);

    Ok(Report::new(HashMap::new()))
}

fn parse_stdout(input: &str) -> IResult<&str, Report> {
    todo!()
}
