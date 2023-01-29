use std::fmt;

use bencher_json::BenchmarkName;
use nom::{
    branch::alt, bytes::complete::tag, character::complete::digit1, combinator::map,
    multi::fold_many1, IResult,
};
use ordered_float::OrderedFloat;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};

use crate::AdapterError;

pub type NomError = nom::Err<nom::error::Error<String>>;

pub fn nom_error(input: String) -> NomError {
    nom::Err::Error(nom::error::make_error(input, nom::error::ErrorKind::Tag))
}

pub fn time_as_nanos<T>(time: T, units: Units) -> OrderedFloat<f64>
where
    T: Into<Time>,
{
    (time.into().as_f64() * units.as_nanos()).into()
}

#[derive(Clone, Copy)]
pub enum Time {
    UInt64(u64),
    Float64(f64),
    Decimal(Decimal),
}

impl From<u64> for Time {
    fn from(int: u64) -> Self {
        Self::UInt64(int)
    }
}

impl From<f64> for Time {
    fn from(float: f64) -> Self {
        Self::Float64(float)
    }
}

impl From<Decimal> for Time {
    fn from(decimal: Decimal) -> Self {
        Self::Decimal(decimal)
    }
}

impl Time {
    fn as_f64(&self) -> f64 {
        match self {
            Self::UInt64(int) => *int as f64,
            Self::Float64(float) => *float,
            Self::Decimal(decimal) => decimal.to_f64().unwrap_or_default(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
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

impl<'de> Deserialize<'de> for Units {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(UnitsVisitor)
    }
}

struct UnitsVisitor;

impl<'de> Visitor<'de> for UnitsVisitor {
    type Value = Units;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a standard unit abbreviation")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let (remainder, units) = parse_units(value).map_err(E::custom)?;
        if remainder.is_empty() {
            Ok(units)
        } else {
            Err(E::custom(AdapterError::BenchmarkUnits))
        }
    }
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

pub fn parse_benchmark_name_chars(name_chars: &[char]) -> Result<BenchmarkName, NomError> {
    let name = name_chars.into_iter().collect();
    parse_benchmark_name(name)
}

pub fn parse_benchmark_name(name: String) -> Result<BenchmarkName, NomError> {
    if let Ok(benchmark_name) = name.parse() {
        Ok(benchmark_name)
    } else {
        Err(nom_error(name))
    }
}
