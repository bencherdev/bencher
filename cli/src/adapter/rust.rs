use std::collections::HashMap;

use nom::bytes::complete::tag;
use nom::character::complete::digit1;
use nom::character::complete::line_ending;
use nom::character::complete::space1;
use nom::combinator::map;
use nom::sequence::tuple;
use nom::IResult;

use crate::error::CliError;
use crate::output::Output;
use crate::report::Report;

pub fn parse(output: Output) -> Result<Report, CliError> {
    println!("{:?}", output);

    let report = parse_stdout(&output.stdout);

    Ok(Report::new(HashMap::new()))
}

// TODO if there is only a single test, it says `test` otherwise it says `tests`
fn parse_stdout(input: &str) -> IResult<&str, Report> {
    map(
        tuple((line_ending, tag("running"), space1, digit1)),
        |_| todo!(),
    )(input)
}

#[cfg(test)]
mod test {
    use super::parse_stdout;

    #[test]
    fn test_adapter_rust() {
        let input = "\nrunning 2 tests\ntest tests::ignored ... ignored\ntest tests::benchmark ... bench:       3,161 ns/iter (+/- 975)\n\ntest result: ok. 0 passed; 0 failed; 1 ignored; 1 measured; 0 filtered out; finished in 0.11s\n\n";
        let adapted = parse_stdout(input).unwrap();
        println!("{:?}", adapted);
    }
}
