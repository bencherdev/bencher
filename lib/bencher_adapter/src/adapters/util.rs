use std::{fmt, str::FromStr};

use bencher_json::BenchmarkName;
use nom::{
    IResult, Parser as _,
    branch::alt,
    bytes::complete::tag,
    character::complete::digit1,
    combinator::{map, map_res},
    error::ErrorKind as NomErrorKind,
    multi::fold_many1,
};
use ordered_float::OrderedFloat;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive as _;
use serde::{
    Deserialize, Deserializer,
    de::{self, Visitor},
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
    #[expect(
        clippy::cast_precision_loss,
        reason = "u64 time value cast to f64 for conversion"
    )]
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
    ))
    .parse(input)
}

impl FromStr for Units {
    type Err = AdapterError;

    fn from_str(units_str: &str) -> Result<Self, Self::Err> {
        #[expect(
            clippy::map_err_ignore,
            reason = "nom error replaced with domain-specific error"
        )]
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

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.parse().map_err(E::custom)
    }
}

pub fn parse_number_as_f64(input: &str) -> IResult<&str, f64> {
    // It is important to try to parse as a float first,
    // in order to avoid a false positive when parsing an integer.
    map_res(alt((parse_float, parse_int)), into_number).parse(input)
}

pub fn parse_u64(input: &str) -> IResult<&str, u64> {
    map_res(parse_int, into_number).parse(input)
}

pub fn parse_f64(input: &str) -> IResult<&str, f64> {
    map_res(parse_float, into_number).parse(input)
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
    )
    .parse(input)
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
    )
    .parse(input)
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

#[cfg(test)]
mod tests {
    use bencher_json::BenchmarkName;
    use nom::error::{Error as NomInnerError, ErrorKind as NomErrorKind};
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;
    use rust_decimal::Decimal;

    use crate::AdapterError;

    use super::{
        NomError, Units, into_number, latency_as_nanos, nom_error, parse_benchmark_name,
        parse_benchmark_name_chars, parse_f64, parse_int, parse_number_as_f64, parse_u64,
        parse_units, throughput_as_secs,
    };

    const ALL_UNITS: [Units; 5] = [
        Units::Pico,
        Units::Nano,
        Units::Micro,
        Units::Milli,
        Units::Sec,
    ];

    fn assert_units(expected: Units, actual: Units, context: &str) {
        // `Units` does not implement `PartialEq`,
        // so compare the `Debug` representations instead.
        assert_eq!(format!("{expected:?}"), format!("{actual:?}"), "{context}");
    }

    #[test]
    fn units_as_nanos() {
        for (index, (units, expected)) in [
            (Units::Pico, 1e-3),
            (Units::Nano, 1.0),
            (Units::Micro, 1e3),
            (Units::Milli, 1e6),
            (Units::Sec, 1e9),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                OrderedFloat(expected),
                OrderedFloat(units.as_nanos()),
                "#{index}: {units:?}"
            );
        }
    }

    #[test]
    fn units_as_secs() {
        for (index, (units, expected)) in [
            (Units::Pico, 1e-12),
            (Units::Nano, 1e-9),
            (Units::Micro, 1e-6),
            (Units::Milli, 1e-3),
            (Units::Sec, 1.0),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                OrderedFloat(expected),
                OrderedFloat(units.as_secs()),
                "#{index}: {units:?}"
            );
        }
    }

    #[test]
    fn units_nanos_secs_relationship() {
        // For every unit there are exactly 1e9 nanoseconds per second,
        // modulo floating point rounding error.
        for units in ALL_UNITS {
            let ratio = units.as_nanos() / units.as_secs();
            assert!(
                (ratio - 1e9).abs() < 1.0,
                "{units:?}: {ratio} should be within 1.0 of 1e9"
            );
        }
    }

    #[test]
    fn latency_as_nanos_units() {
        for (index, (units, expected)) in [
            (Units::Pico, 1e-3),
            (Units::Nano, 1.0),
            (Units::Micro, 1e3),
            (Units::Milli, 1e6),
            (Units::Sec, 1e9),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                OrderedFloat(expected),
                latency_as_nanos(1.0, units),
                "#{index}: {units:?}"
            );
        }
    }

    #[test]
    fn latency_as_nanos_time_variants() {
        // `u64`, `f64`, `OrderedFloat<f64>`, and `Decimal` times
        // all convert to the same value.
        let expected = OrderedFloat(5_000.0);
        assert_eq!(expected, latency_as_nanos(5u64, Units::Micro), "u64");
        assert_eq!(expected, latency_as_nanos(5.0, Units::Micro), "f64");
        assert_eq!(
            expected,
            latency_as_nanos(OrderedFloat(5.0), Units::Micro),
            "OrderedFloat<f64>"
        );
        assert_eq!(
            expected,
            latency_as_nanos(Decimal::from(5u64), Units::Micro),
            "Decimal"
        );

        // Fractional values
        assert_eq!(OrderedFloat(2_500.0), latency_as_nanos(2.5, Units::Micro));
        assert_eq!(
            OrderedFloat(2_500.0),
            latency_as_nanos(Decimal::new(25, 1), Units::Micro)
        );
        assert_eq!(OrderedFloat(0.0), latency_as_nanos(0u64, Units::Sec));
    }

    #[test]
    fn latency_as_nanos_u64_max_precision_loss() {
        // u64::MAX is 18_446_744_073_709_551_615 but f64 only has 53 bits of
        // mantissa, so the converted value is rounded to the nearest f64.
        assert_eq!(
            OrderedFloat(1.844_674_407_370_955_2e19),
            latency_as_nanos(u64::MAX, Units::Nano)
        );
    }

    #[test]
    fn latency_as_nanos_decimal_edges() {
        // Decimal::MAX is 79_228_162_514_264_337_593_543_950_335 (2^96 - 1)
        assert_eq!(
            OrderedFloat(7.922_816_251_426_434e28),
            latency_as_nanos(Decimal::MAX, Units::Nano),
            "Decimal::MAX"
        );
        assert_eq!(
            OrderedFloat(-7.922_816_251_426_434e28),
            latency_as_nanos(Decimal::MIN, Units::Nano),
            "Decimal::MIN"
        );
        assert_eq!(
            OrderedFloat(0.0),
            latency_as_nanos(Decimal::ZERO, Units::Sec),
            "Decimal::ZERO"
        );
        // Negative latencies are passed through as-is
        assert_eq!(
            OrderedFloat(-1_000.0),
            latency_as_nanos(Decimal::NEGATIVE_ONE, Units::Micro),
            "Decimal::NEGATIVE_ONE"
        );
        // Maximum scale: 1e-28
        // `Decimal::to_f64` is not correctly rounded for the maximum scale,
        // so the result is one ULP above 1e-19
        assert_eq!(
            OrderedFloat(1.000_000_000_000_000_1e-19),
            latency_as_nanos(Decimal::new(1, 28), Units::Sec),
            "Decimal 1e-28"
        );
    }

    #[test]
    fn throughput_as_secs_units() {
        for (index, (units, expected)) in [
            (Units::Pico, 1e12),
            // Note: 1.0 / (1.0 / 1e9) rounds to just under 1e9
            (Units::Nano, 999_999_999.999_999_9),
            (Units::Micro, 1e6),
            (Units::Milli, 1e3),
            (Units::Sec, 1.0),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                OrderedFloat(expected),
                throughput_as_secs(1.0, units),
                "#{index}: {units:?}"
            );
        }
    }

    #[test]
    fn throughput_as_secs_time_variants() {
        let expected = OrderedFloat(7_000.0);
        assert_eq!(expected, throughput_as_secs(7u64, Units::Milli), "u64");
        assert_eq!(expected, throughput_as_secs(7.0, Units::Milli), "f64");
        assert_eq!(
            expected,
            throughput_as_secs(OrderedFloat(7.0), Units::Milli),
            "OrderedFloat<f64>"
        );
        assert_eq!(
            expected,
            throughput_as_secs(Decimal::from(7u64), Units::Milli),
            "Decimal"
        );
    }

    #[test]
    fn latency_throughput_round_trip() {
        // One operation per `unit` of time means
        // `as_nanos()` nanoseconds of latency and
        // 1 / `as_secs()` operations per second.
        // Latency (ns) * throughput (ops/s) should always be ~1e9 ns/s.
        for units in ALL_UNITS {
            let latency = latency_as_nanos(1.0, units);
            let throughput = throughput_as_secs(1.0, units);
            let ns_per_sec = latency.into_inner() * throughput.into_inner();
            assert!(
                (ns_per_sec - 1e9).abs() < 1.0,
                "{units:?}: {ns_per_sec} should be within 1.0 of 1e9"
            );
        }
    }

    #[test]
    fn parse_units_valid() {
        for (index, (input, expected)) in [
            ("ps", Units::Pico),
            ("ns", Units::Nano),
            ("μs", Units::Micro), // Greek small letter mu (U+03BC)
            ("µs", Units::Micro), // Micro sign (U+00B5)
            ("us", Units::Micro),
            ("ms", Units::Milli),
            ("s", Units::Sec),
        ]
        .into_iter()
        .enumerate()
        {
            let (remainder, units) = parse_units(input).unwrap();
            assert_eq!("", remainder, "#{index}: {input}");
            assert_units(expected, units, input);
        }
    }

    #[test]
    fn parse_units_remainder() {
        for (index, (input, expected, expected_remainder)) in [
            ("ns/iter", Units::Nano, "/iter"),
            ("s/op", Units::Sec, "/op"),
            ("ss", Units::Sec, "s"),
            ("ms ", Units::Milli, " "),
        ]
        .into_iter()
        .enumerate()
        {
            let (remainder, units) = parse_units(input).unwrap();
            assert_eq!(expected_remainder, remainder, "#{index}: {input}");
            assert_units(expected, units, input);
        }
    }

    #[test]
    fn parse_units_invalid() {
        for (index, input) in ["", "x", "p", "n", "NS", " ns", "S", "/s"]
            .into_iter()
            .enumerate()
        {
            assert_eq!(true, parse_units(input).is_err(), "#{index}: {input}");
        }
    }

    #[test]
    fn units_from_str_valid() {
        for (index, (input, expected)) in [
            ("ps", Units::Pico),
            ("ns", Units::Nano),
            ("μs", Units::Micro),
            ("µs", Units::Micro),
            ("us", Units::Micro),
            ("ms", Units::Milli),
            ("s", Units::Sec),
        ]
        .into_iter()
        .enumerate()
        {
            let units: Units = input.parse().unwrap_or_else(|e| {
                panic!("#{index}: {input} failed to parse: {e}");
            });
            assert_units(expected, units, input);
        }
    }

    #[test]
    fn units_from_str_trailing_garbage() {
        // A valid unit followed by junk must be rejected
        for (index, input) in ["ns/iter", "nsx", "ss", "ms ", "s/op", "us\n"]
            .into_iter()
            .enumerate()
        {
            let err = input.parse::<Units>().unwrap_err();
            let AdapterError::BenchmarkUnits(units_str) = err else {
                panic!("#{index}: {input} returned unexpected error variant");
            };
            assert_eq!(input, units_str, "#{index}");
        }
    }

    #[test]
    fn units_from_str_invalid() {
        for (index, input) in ["", "x", "NS", " ns", "1ns"].into_iter().enumerate() {
            let err = input.parse::<Units>().unwrap_err();
            let AdapterError::BenchmarkUnits(units_str) = err else {
                panic!("#{index}: {input} returned unexpected error variant");
            };
            assert_eq!(input, units_str, "#{index}");
        }
    }

    #[test]
    fn units_deserialize_valid() {
        for (index, (input, expected)) in [
            ("\"ps\"", Units::Pico),
            ("\"ns\"", Units::Nano),
            ("\"us\"", Units::Micro),
            ("\"ms\"", Units::Milli),
            ("\"s\"", Units::Sec),
        ]
        .into_iter()
        .enumerate()
        {
            let units: Units = serde_json::from_str(input).unwrap_or_else(|e| {
                panic!("#{index}: {input} failed to deserialize: {e}");
            });
            assert_units(expected, units, input);
        }
    }

    #[test]
    fn units_deserialize_invalid() {
        for (index, input) in [
            "\"\"",
            "\"abc\"",
            "\"ns/iter\"",
            "5",
            "null",
            "[\"ns\"]",
            "{}",
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                true,
                serde_json::from_str::<Units>(input).is_err(),
                "#{index}: {input}"
            );
        }
    }

    #[test]
    fn parse_number_as_f64_valid() {
        for (index, (expected, input)) in [
            (Ok(("", 1_000.5)), "1,000.50"),
            (Ok(("", 1_000.0)), "1,000"),
            (Ok(("", 123.0)), "123"),
            (Ok(("", 0.001)), "0.001"),
            (Ok(("", 0.5)), ".5"),
            (Ok(("", 5.0)), "5."),
            (Ok(("", 7.0)), "007"),
            (Ok(("", 1_234_567.89)), "1,234,567.89"),
            // Commas are stripped without validating their placement
            (Ok(("", 1_000.0)), "1,,000"),
            (Ok(("", 5.0)), ",5"),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_number_as_f64(input), "#{index}: {input}");
        }
    }

    #[test]
    fn parse_number_as_f64_remainder() {
        // The parser stops at the first non-numeric character
        // and returns the rest as the remainder.
        for (index, (expected, input)) in [
            (Ok((" ns", 4.2)), "4.2 ns"),
            (Ok(("abc", 12.0)), "12abc"),
            // No support for scientific notation
            (Ok(("e3", 1.0)), "1e3"),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_number_as_f64(input), "#{index}: {input}");
        }
    }

    #[test]
    fn parse_number_as_f64_invalid() {
        // Note: multi-dot input like "1.2.3" is an error rather than a
        // partial parse: the float branch greedily consumes all dots and
        // digits, then fails conversion, and `map_res` does not backtrack
        // to the integer branch.
        for (index, input) in ["", "abc", "-1", "+1", ".", ",", " 1", "NaN", "1.2.3x"]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                true,
                parse_number_as_f64(input).is_err(),
                "#{index}: {input}"
            );
        }
    }

    #[test]
    fn parse_u64_valid() {
        for (index, (expected, input)) in [
            (Ok(("", 0u64)), "0"),
            (Ok(("", 123)), "123"),
            (Ok(("", 1_000)), "1,000"),
            (Ok(("", 1_234_567)), "1,234,567"),
            (Ok(("", u64::MAX)), "18446744073709551615"),
            // Stops at the decimal point
            (Ok((".5", 1)), "1.5"),
            (Ok((" ns", 42)), "42 ns"),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_u64(input), "#{index}: {input}");
        }
    }

    #[test]
    fn parse_u64_invalid() {
        for (index, input) in [
            "",
            "abc",
            "-1",
            ".5",
            ",",
            // u64::MAX + 1 overflows
            "18446744073709551616",
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(true, parse_u64(input).is_err(), "#{index}: {input}");
        }
    }

    #[test]
    fn parse_f64_valid() {
        for (index, (expected, input)) in [
            (Ok(("", 1.5)), "1.5"),
            (Ok(("", 0.001)), "0.001"),
            // A decimal point is not actually required
            (Ok(("", 123.0)), "123"),
            (Ok(("", 1_234.5)), "1,234.5"),
            (Ok((" s", 0.25)), "0.25 s"),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_f64(input), "#{index}: {input}");
        }
    }

    #[test]
    fn parse_f64_invalid() {
        for (index, input) in [
            "", "abc", "-1.5", ".",
            // Multiple decimal points are all consumed and then fail conversion
            "1.2.3", "1..2",
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(true, parse_f64(input).is_err(), "#{index}: {input}");
        }
    }

    #[test]
    fn parse_int_strips_commas() {
        for (index, (expected, input)) in [
            (Ok(("", vec!["123"])), "123"),
            (Ok(("", vec!["1", "000"])), "1,000"),
            // A lone comma parses as an empty digit list
            (Ok(("", vec![])), ","),
            (Ok((".5", vec!["1"])), "1.5"),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_int(input), "#{index}: {input}");
        }
    }

    #[test]
    fn into_number_valid() {
        assert_eq!(Ok(1_000u64), into_number(vec!["1", "000"]));
        assert_eq!(Ok(0.5f64), into_number(vec![".", "5"]));
        assert_eq!(Ok(123u64), into_number(vec!["123"]));
    }

    #[test]
    fn into_number_invalid() {
        // An empty digit list produces an empty string, which fails conversion
        assert_eq!(true, into_number::<u64>(vec![]).is_err(), "empty");
        assert_eq!(
            true,
            into_number::<u64>(vec![".", "5"]).is_err(),
            "float as u64"
        );
    }

    #[test]
    fn nom_error_tag_kind() {
        let expected: NomError = nom::Err::Error(NomInnerError::new(
            "some input".to_owned(),
            NomErrorKind::Tag,
        ));
        assert_eq!(expected, nom_error("some input"));
        assert_eq!(expected, nom_error(String::from("some input")));
    }

    #[test]
    fn benchmark_name_valid() {
        for (index, input) in [
            "a",
            "tests::benchmark",
            "BenchmarkFib/my_tabled_benchmark_-_10-8",
            "with spaces ~!",
            "0123456789",
        ]
        .into_iter()
        .enumerate()
        {
            let name = parse_benchmark_name(input).unwrap_or_else(|e| {
                panic!("#{index}: {input} failed to parse: {e}");
            });
            assert_eq!(input, name.as_ref(), "#{index}");
        }
    }

    #[test]
    fn benchmark_name_invalid() {
        let err = parse_benchmark_name("").unwrap_err();
        assert_eq!(nom_error(""), err);
    }

    #[test]
    fn benchmark_name_max_len() {
        let max_len_name = "a".repeat(BenchmarkName::MAX_LEN);
        let name = parse_benchmark_name(&max_len_name).unwrap();
        assert_eq!(max_len_name, name.as_ref());

        let too_long_name = "a".repeat(BenchmarkName::MAX_LEN + 1);
        let err = parse_benchmark_name(&too_long_name).unwrap_err();
        assert_eq!(nom_error(too_long_name.as_str()), err);
    }

    #[test]
    fn benchmark_name_chars_valid() {
        let name = parse_benchmark_name_chars(&['f', 'o', 'o']).unwrap();
        assert_eq!("foo", name.as_ref());

        let name = parse_benchmark_name_chars(&['μ', 's', ' ', 'b', 'e', 'n', 'c', 'h']).unwrap();
        assert_eq!("μs bench", name.as_ref());
    }

    #[test]
    fn benchmark_name_chars_invalid() {
        let err = parse_benchmark_name_chars(&[]).unwrap_err();
        assert_eq!(nom_error(""), err);
    }

    #[test]
    fn benchmark_name_chars_byte_len() {
        // The max length check is on bytes, not chars:
        // 'é' is 2 bytes in UTF-8, so 512 of them hit the 1024 byte limit
        let half_max_len = BenchmarkName::MAX_LEN.div_euclid(2);
        let max_len_chars = vec!['é'; half_max_len];
        let name = parse_benchmark_name_chars(&max_len_chars).unwrap();
        assert_eq!(half_max_len, name.as_ref().chars().count());

        let too_long_chars = vec!['é'; half_max_len + 1];
        assert_eq!(
            true,
            parse_benchmark_name_chars(&too_long_chars).is_err(),
            "513 two-byte chars exceeds the 1024 byte limit"
        );
    }
}
