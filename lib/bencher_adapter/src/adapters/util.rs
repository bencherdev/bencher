use std::{fmt, str::FromStr};

use bencher_json::BenchmarkName;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::digit1,
    combinator::{map, map_res},
    error::ErrorKind as NomErrorKind,
    multi::fold_many1,
    IResult,
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

pub fn nom_error<T>(input: T) -> NomError
where
    T: Into<String>,
{
    nom::Err::Error(nom::error::make_error(input.into(), NomErrorKind::Tag))
}

pub fn latency_as_nanos<T>(time: T, units: Units) -> OrderedFloat<f64>
where
    T: Into<Time>,
{
    (time.into().as_f64() * units.as_nanos()).into()
}

pub fn throughput_as_secs<T>(time: T, units: Units) -> OrderedFloat<f64>
where
    T: Into<Time>,
{
    (time.into().as_f64() / units.as_secs()).into()
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

impl From<OrderedFloat<f64>> for Time {
    fn from(float: OrderedFloat<f64>) -> Self {
        Self::Float64(float.into_inner())
    }
}

impl From<Decimal> for Time {
    fn from(decimal: Decimal) -> Self {
        Self::Decimal(decimal)
    }
}

impl Time {
    #[allow(clippy::cast_precision_loss)]
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
    pub fn as_nanos(self) -> f64 {
        match self {
            Self::Pico => 1.0 / 1_000.0,
            Self::Nano => 1.0,
            Self::Micro => 1_000.0,
            Self::Milli => 1_000_000.0,
            Self::Sec => 1_000_000_000.0,
        }
    }

    pub fn as_secs(self) -> f64 {
        match self {
            Self::Pico => 1.0 / 1_000_000_000_000.0,
            Self::Nano => 1.0 / 1_000_000_000.0,
            Self::Micro => 1.0 / 1_000_000.0,
            Self::Milli => 1.0 / 1_000.0,
            Self::Sec => 1.0,
        }
    }
}

pub fn parse_units(input: &str) -> IResult<&str, Units> {
    alt((
        map(tag("ps"), |_| Units::Pico),
        map(tag("ns"), |_| Units::Nano),
        map(tag("μs"), |_| Units::Micro),
        map(tag("µs"), |_| Units::Micro),
        map(tag("us"), |_| Units::Micro),
        map(tag("ms"), |_| Units::Milli),
        map(tag("s"), |_| Units::Sec),
    ))(input)
}

impl FromStr for Units {
    type Err = AdapterError;

    fn from_str(units_str: &str) -> Result<Self, Self::Err> {
        #[allow(clippy::map_err_ignore)]
        let (remainder, units) =
            parse_units(units_str).map_err(|_| Self::Err::BenchmarkUnits(units_str.into()))?;
        if remainder.is_empty() {
            Ok(units)
        } else {
            Err(AdapterError::BenchmarkUnits(units_str.into()))
        }
    }
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

impl Visitor<'_> for UnitsVisitor {
    type Value = Units;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a standard unit abbreviation")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

pub fn parse_number_as_f64(input: &str) -> IResult<&str, f64> {
    // It is important to try to parse as a float first,
    // in order to avoid a false positive when parsing an integer.
    map_res(alt((parse_float, parse_int)), into_number)(input)
}

pub fn parse_u64(input: &str) -> IResult<&str, u64> {
    map_res(parse_int, into_number)(input)
}

pub fn parse_f64(input: &str) -> IResult<&str, f64> {
    map_res(parse_float, into_number)(input)
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
    T: FromStr,
{
    let mut number = String::new();
    for digit in input {
        number.push_str(digit);
    }

    T::from_str(&number)
        .map_err(|_e| nom::Err::Error(nom::error::make_error("\0", NomErrorKind::Tag)))
}

pub fn parse_benchmark_name_chars(name_chars: &[char]) -> Result<BenchmarkName, NomError> {
    let name: String = name_chars.iter().collect();
    parse_benchmark_name(&name)
}

pub fn parse_benchmark_name(name: &str) -> Result<BenchmarkName, NomError> {
    if let Ok(benchmark_name) = name.parse() {
        Ok(benchmark_name)
    } else {
        Err(nom_error(name))
    }
}
