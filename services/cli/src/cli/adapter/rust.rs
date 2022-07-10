use std::collections::BTreeMap;
use std::str::FromStr;
use std::time::Duration;

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take_until1;
use nom::character::complete::digit1;
use nom::character::complete::line_ending;
use nom::character::complete::space1;
use nom::combinator::map;
use nom::combinator::success;
use nom::multi::many0;
use nom::multi::many1;
use nom::sequence::tuple;
use nom::IResult;
use bencher_json::{Latency, Metrics};

use crate::cli::benchmark::Output;
use crate::BencherError;

pub fn parse(output: &Output) -> Result<Metrics, BencherError> {
    let (_, report) = parse_stdout(output.as_str()).unwrap();
    Ok(report)
}

enum Test {
    Ignored,
    Bench(Latency),
}

fn parse_stdout(input: &str) -> IResult<&str, Metrics> {
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
            let mut latencies = BTreeMap::new();
            for bench in benches {
                if let Some((key, latency)) = to_latency(bench) {
                    latencies.insert(key, latency);
                }
            }
            Metrics {
                latency: Some(latencies),
                ..Default::default()
            }
        },
    )(input)
}

fn to_latency(
    bench: (&str, &str, &str, &str, &str, &str, Test, &str),
) -> Option<(String, Latency)> {
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
            _ => panic!("Inexpected time abbreviation"),
        }
    }
}

fn parse_bench(input: &str) -> IResult<&str, Latency> {
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
            Latency {
                duration,
                upper_variance: variance.clone(),
                lower_variance: variance,
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
