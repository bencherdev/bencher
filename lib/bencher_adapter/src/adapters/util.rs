use std::{str::FromStr, time::Duration};

use nom::{
    branch::alt,
    bytes::complete::{tag, take_until1},
    character::complete::{anychar, digit1, space1},
    combinator::{eof, map, map_res, peek, success},
    multi::{fold_many1, many1, many_till},
    sequence::{delimited, tuple},
    IResult,
};
use ordered_float::OrderedFloat;

pub fn time_nanos(time: u64, units: Units) -> OrderedFloat<f64> {
    (get_duration(time, units).as_nanos() as f64).into()
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]
pub fn get_duration(time: u64, units: Units) -> Duration {
    match units {
        Units::Pico => Duration::from_nanos((time as f64 * units.as_nanos()) as u64),
        Units::Nano => Duration::from_nanos(time),
        Units::Micro => Duration::from_micros(time),
        Units::Milli => Duration::from_millis(time),
        Units::Sec => Duration::from_secs(time),
    }
}

#[derive(Clone, Copy)]
pub enum Units {
    Pico,
    Nano,
    Micro,
    Milli,
    Sec,
}

impl Units {
    #[allow(clippy::float_arithmetic)]
    pub fn as_nanos(&self) -> f64 {
        match self {
            Self::Pico => 1.0 / 1_000.0,
            Self::Nano => 1.0,
            Self::Micro => 1_000.0,
            Self::Milli => 1_000_000.0,
            Self::Sec => 1_000_000_000.0,
        }
    }
}

pub fn parse_units(input: &str) -> IResult<&str, Units> {
    alt((
        map(tag("ps"), |_| Units::Pico),
        map(tag("ns"), |_| Units::Nano),
        map(tag("\u{3bc}s"), |_| Units::Micro),
        map(tag("\u{b5}s"), |_| Units::Micro),
        map(tag("ms"), |_| Units::Milli),
        map(tag("s"), |_| Units::Sec),
    ))(input)
}

pub fn parse_u64(input: &str) -> IResult<&str, u64> {
    let (remainder, int) = parse_int(input)?;
    Ok((remainder, into_number(int)?))
}

pub fn parse_f64(input: &str) -> IResult<&str, f64> {
    let (remainder, float) = parse_float(input)?;
    Ok((remainder, into_number(float)?))
}

pub fn parse_int(input: &str) -> IResult<&str, Vec<&str>> {
    fold_many1(
        alt((digit1, tag(","))),
        Vec::new,
        |mut int_chars, int_char| {
            if int_char == "," {
                int_chars
            } else {
                int_chars.push(int_char);
                int_chars
            }
        },
    )(input)
}

pub fn parse_float(input: &str) -> IResult<&str, Vec<&str>> {
    fold_many1(
        alt((digit1, tag("."), tag(","))),
        Vec::new,
        |mut float_chars, float_char| {
            if float_char == "," {
                float_chars
            } else {
                float_chars.push(float_char);
                float_chars
            }
        },
    )(input)
}

pub fn into_number<T>(input: Vec<&str>) -> Result<T, nom::Err<nom::error::Error<&str>>>
where
    T: std::str::FromStr,
{
    let mut number = String::new();
    for digit in input {
        number.push_str(digit);
    }

    T::from_str(&number)
        .map_err(|_e| nom::Err::Error(nom::error::make_error("\0", nom::error::ErrorKind::Tag)))
}
