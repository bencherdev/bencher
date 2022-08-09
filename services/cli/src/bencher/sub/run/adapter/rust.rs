use std::{
    str::FromStr,
    time::Duration,
};

use bencher_json::report::{
    JsonLatency,
    JsonNewBenchmarks,
    JsonNewPerf,
};
use nom::{
    branch::alt,
    bytes::complete::{
        tag,
        take_until1,
    },
    character::complete::{
        digit1,
        line_ending,
        space1,
    },
    combinator::{
        map,
        success,
    },
    multi::{
        many0,
        many1,
    },
    sequence::tuple,
    IResult,
};

use crate::{
    bencher::sub::run::benchmark::Output,
    BencherError,
};

pub fn parse(output: &Output) -> Result<JsonNewBenchmarks, BencherError> {
    let (_, report) = parse_stdout(output.as_str()).unwrap();
    Ok(report)
}

enum Test {
    Ignored,
    Bench(JsonLatency),
}

fn parse_stdout(input: &str) -> IResult<&str, JsonNewBenchmarks> {
    map(
        tuple((
            line_ending,
            // running X test(s)
            tag("running"),
            space1,
            digit1,
            space1,
            alt((tag("tests"), tag("test"))),
            line_ending,
            // test rust::mod::path::to_test ... ignored/Y ns/iter (+/- Z)
            many0(tuple((
                tag("test"),
                space1,
                take_until1(" "),
                space1,
                tag("..."),
                space1,
                alt((
                    map(tag("ignored"), |_| Test::Ignored),
                    map(parse_bench, Test::Bench),
                )),
                line_ending,
            ))),
            line_ending,
        )),
        |(_, _, _, _, _, _, _, benches, _)| {
            let mut benchmarks = JsonNewBenchmarks::new();
            for bench in benches {
                if let Some((benchmark, latency)) = to_latency(bench) {
                    let perf = JsonNewPerf {
                        latency: Some(latency),
                        ..Default::default()
                    };
                    benchmarks.insert(benchmark, perf);
                }
            }
            benchmarks
        },
    )(input)
}

fn to_latency(
    bench: (&str, &str, &str, &str, &str, &str, Test, &str),
) -> Option<(String, JsonLatency)> {
    let (_, _, key, _, _, _, test, _) = bench;
    match test {
        Test::Ignored => None,
        Test::Bench(latency) => Some((key.into(), latency)),
    }
}

pub enum Units {
    Nano,
    Micro,
    Milli,
    Sec,
}

impl From<&str> for Units {
    fn from(time: &str) -> Self {
        match time {
            "ns" => Self::Nano,
            "μs" => Self::Micro,
            "ms" => Self::Milli,
            "s" => Self::Sec,
            _ => panic!("Unexpected time abbreviation"),
        }
    }
}

fn parse_bench(input: &str) -> IResult<&str, JsonLatency> {
    map(
        tuple((
            tag("bench:"),
            space1,
            parse_number,
            space1,
            take_until1("/"),
            tag("/iter"),
            space1,
            tag("(+/-"),
            space1,
            parse_number,
            tag(")"),
        )),
        |(_, _, duration, _, units, _, _, _, _, variance, _)| {
            let units = Units::from(units);
            let duration = to_duration(to_u64(duration), &units);
            let variance = to_duration(to_u64(variance), &units);
            JsonLatency {
                duration:       duration.as_nanos(),
                lower_variance: variance.as_nanos(),
                upper_variance: variance.as_nanos(),
            }
        },
    )(input)
}

fn parse_number(input: &str) -> IResult<&str, Vec<(&str, &str)>> {
    many1(tuple((digit1, alt((tag(","), success(" "))))))(input)
}

fn to_u64(input: Vec<(&str, &str)>) -> u64 {
    let mut number = String::new();
    for (digit, _) in input {
        number.push_str(digit);
    }
    u64::from_str(&number).unwrap()
}

fn to_duration(time: u64, units: &Units) -> Duration {
    match units {
        Units::Nano => Duration::from_nanos(time),
        Units::Micro => Duration::from_micros(time),
        Units::Milli => Duration::from_millis(time),
        Units::Sec => Duration::from_secs(time),
    }
}

#[cfg(test)]
mod test {
    use super::parse_stdout;

    #[test]
    fn test_adapter_rust_zero() {
        let input = "\nrunning 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n\n";
        let adapted = parse_stdout(input).unwrap();
        println!("{:?}", adapted);
    }

    #[test]
    fn test_adapter_rust_one() {
        let input = "\nrunning 1 test\ntest tests::benchmark ... bench:       3,161 ns/iter (+/- 975)\n\ntest result: ok. 0 passed; 0 failed; 1 ignored; 1 measured; 0 filtered out; finished in 0.11s\n\n";
        let adapted = parse_stdout(input).unwrap();
        println!("{:?}", adapted);
    }

    #[test]
    fn test_adapter_rust_two() {
        let input = "\nrunning 2 tests\ntest tests::ignored ... ignored\ntest tests::benchmark ... bench:       3,161 ns/iter (+/- 975)\n\ntest result: ok. 0 passed; 0 failed; 1 ignored; 1 measured; 0 filtered out; finished in 0.11s\n\n";
        let adapted = parse_stdout(input).unwrap();
        println!("{:?}", adapted);
    }
}
