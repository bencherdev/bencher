use ordered_float::OrderedFloat;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use crate::ResourceName;

pub const NANOSECONDS: &str = "nanoseconds (ns)";
pub const SECONDS: &str = "seconds (s)";
pub const BYTES: &str = "bytes (B)";
pub const PERCENTAGE: &str = "percentage (%)";
pub const DECIBELS: &str = "decibels (dB)";

#[derive(Debug, Clone)]
pub struct Units {
    scale: Scale,
    units: ResourceName,
}

impl Units {
    pub fn new(min: f64, units: ResourceName) -> Self {
        let scale = Scale::new(min, units.as_ref());
        Self { scale, units }
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "scale factors are powers of 10, exact in f64"
    )]
    pub fn scale_factor(&self) -> OrderedFloat<f64> {
        OrderedFloat::from(self.scale.factor() as f64)
    }

    pub fn scale_units(&self) -> String {
        self.scale.units(self.units.as_ref())
    }

    pub fn scale_units_symbol(&self) -> String {
        self.scale.units_symbol(self.units.as_ref())
    }

    pub fn format_float(number: f64) -> String {
        Self::format_number(number, false)
    }

    pub fn format_number(number: f64, trim_decimal: bool) -> String {
        format_number(number, trim_decimal)
    }
}

#[derive(Debug, Clone, Copy)]
enum Scale {
    Nanos(ScaleNanos),
    Secs(ScaleSecs),
    Byte(ScaleBytes),
    OneE(ScaleOneE),
}

impl Scale {
    #[expect(
        clippy::cast_precision_loss,
        reason = "scale thresholds are powers of 10, exact in f64"
    )]
    fn new(min: f64, units: &str) -> Self {
        match units {
            NANOSECONDS => match min {
                n if n < ScaleNanos::Micros as u64 as f64 => ScaleNanos::Nanos,
                n if n < ScaleNanos::Millis as u64 as f64 => ScaleNanos::Micros,
                n if n < ScaleNanos::Seconds as u64 as f64 => ScaleNanos::Millis,
                n if n < ScaleNanos::Minutes as u64 as f64 => ScaleNanos::Seconds,
                n if n < ScaleNanos::Hours as u64 as f64 => ScaleNanos::Minutes,
                _n => ScaleNanos::Hours,
            }
            .into(),
            SECONDS => match min {
                n if n < ScaleSecs::Minutes as u64 as f64 => ScaleSecs::Seconds,
                n if n < ScaleSecs::Hours as u64 as f64 => ScaleSecs::Minutes,
                _n => ScaleSecs::Hours,
            }
            .into(),
            BYTES => match min {
                n if n < ScaleBytes::Kilo as u64 as f64 => ScaleBytes::Byte,
                n if n < ScaleBytes::Mega as u64 as f64 => ScaleBytes::Kilo,
                n if n < ScaleBytes::Giga as u64 as f64 => ScaleBytes::Mega,
                n if n < ScaleBytes::Tera as u64 as f64 => ScaleBytes::Giga,
                n if n < ScaleBytes::Peta as u64 as f64 => ScaleBytes::Tera,
                _n => ScaleBytes::Peta,
            }
            .into(),
            _ => match min {
                n if n < ScaleOneE::Three as u64 as f64 => ScaleOneE::One,
                n if n < ScaleOneE::Six as u64 as f64 => ScaleOneE::Three,
                n if n < ScaleOneE::Nine as u64 as f64 => ScaleOneE::Six,
                n if n < ScaleOneE::Twelve as u64 as f64 => ScaleOneE::Nine,
                n if n < ScaleOneE::Fifteen as u64 as f64 => ScaleOneE::Twelve,
                _n => ScaleOneE::Fifteen,
            }
            .into(),
        }
    }

    fn factor(&self) -> u64 {
        match self {
            Scale::Nanos(scale) => *scale as u64,
            Scale::Secs(scale) => *scale as u64,
            Scale::Byte(scale) => *scale as u64,
            Scale::OneE(scale) => *scale as u64,
        }
    }

    fn units(&self, units: &str) -> String {
        match self {
            Scale::Nanos(scale) => scale.units(),
            Scale::Secs(scale) => scale.units(),
            Scale::Byte(scale) => scale.units(),
            Scale::OneE(scale) => scale.units(units),
        }
    }

    fn units_symbol(&self, units: &str) -> String {
        match self {
            Scale::Nanos(scale) => scale.units_symbol(),
            Scale::Secs(scale) => scale.units_symbol(),
            Scale::Byte(scale) => scale.units_symbol(),
            Scale::OneE(scale) => scale.units_symbol(units),
        }
    }
}

#[repr(u64)]
#[derive(Debug, Clone, Copy)]
enum ScaleNanos {
    Nanos = 1,
    Micros = 1_000,
    Millis = 1_000_000,
    Seconds = 1_000_000_000,
    Minutes = 60_000_000_000,
    Hours = 3_600_000_000_000,
}

impl From<ScaleNanos> for Scale {
    fn from(scale: ScaleNanos) -> Self {
        Scale::Nanos(scale)
    }
}

impl ScaleNanos {
    fn units(self) -> String {
        match self {
            ScaleNanos::Nanos => NANOSECONDS,
            ScaleNanos::Micros => "microseconds (µs)",
            ScaleNanos::Millis => "milliseconds (ms)",
            ScaleNanos::Seconds => SECONDS,
            ScaleNanos::Minutes => "minutes (m)",
            ScaleNanos::Hours => "hours (h)",
        }
        .to_owned()
    }

    fn units_symbol(self) -> String {
        match self {
            ScaleNanos::Nanos => "ns",
            ScaleNanos::Micros => "µs",
            ScaleNanos::Millis => "ms",
            ScaleNanos::Seconds => "s",
            ScaleNanos::Minutes => "m",
            ScaleNanos::Hours => "h",
        }
        .to_owned()
    }
}

#[repr(u64)]
#[derive(Debug, Clone, Copy)]
enum ScaleSecs {
    Seconds = 1,
    Minutes = 60,
    Hours = 3_600,
}

impl From<ScaleSecs> for Scale {
    fn from(scale: ScaleSecs) -> Self {
        Scale::Secs(scale)
    }
}

impl ScaleSecs {
    fn units(self) -> String {
        match self {
            ScaleSecs::Seconds => "seconds (s)",
            ScaleSecs::Minutes => "minutes (m)",
            ScaleSecs::Hours => "hours (h)",
        }
        .to_owned()
    }

    fn units_symbol(self) -> String {
        match self {
            ScaleSecs::Seconds => "s",
            ScaleSecs::Minutes => "m",
            ScaleSecs::Hours => "h",
        }
        .to_owned()
    }
}

#[repr(u64)]
#[derive(Debug, Clone, Copy)]
enum ScaleBytes {
    Byte = 1,
    Kilo = 1_000,
    Mega = 1_000_000,
    Giga = 1_000_000_000,
    Tera = 1_000_000_000_000,
    Peta = 1_000_000_000_000_000,
}

impl From<ScaleBytes> for Scale {
    fn from(scale: ScaleBytes) -> Self {
        Scale::Byte(scale)
    }
}

impl ScaleBytes {
    fn units(self) -> String {
        match self {
            ScaleBytes::Byte => BYTES,
            ScaleBytes::Kilo => "kilobytes (KB)",
            ScaleBytes::Mega => "megabytes (MB)",
            ScaleBytes::Giga => "gigabytes (GB)",
            ScaleBytes::Tera => "terabytes (TB)",
            ScaleBytes::Peta => "petabytes (PB)",
        }
        .to_owned()
    }

    fn units_symbol(self) -> String {
        match self {
            ScaleBytes::Byte => "B",
            ScaleBytes::Kilo => "KB",
            ScaleBytes::Mega => "MB",
            ScaleBytes::Giga => "GB",
            ScaleBytes::Tera => "TB",
            ScaleBytes::Peta => "PB",
        }
        .to_owned()
    }
}

#[repr(u64)]
#[derive(Debug, Clone, Copy)]
enum ScaleOneE {
    One = 1,
    Three = 1_000,
    Six = 1_000_000,
    Nine = 1_000_000_000,
    Twelve = 1_000_000_000_000,
    Fifteen = 1_000_000_000_000_000,
}

impl From<ScaleOneE> for Scale {
    fn from(scale: ScaleOneE) -> Self {
        Scale::OneE(scale)
    }
}

const X_THREE: &str = "x 1e3";
const X_SIX: &str = "x 1e6";
const X_NINE: &str = "x 1e9";
const X_TWELVE: &str = "x 1e12";
const X_FIFTEEN: &str = "x 1e15";

impl ScaleOneE {
    fn units(self, units: &str) -> String {
        match self {
            ScaleOneE::One => units.to_owned(),
            ScaleOneE::Three => format!("{units} {X_THREE}"),
            ScaleOneE::Six => format!("{units} {X_SIX}"),
            ScaleOneE::Nine => format!("{units} {X_NINE}"),
            ScaleOneE::Twelve => format!("{units} {X_TWELVE}"),
            ScaleOneE::Fifteen => format!("{units} {X_FIFTEEN}"),
        }
    }

    fn units_symbol(self, units: &str) -> String {
        #[inline]
        fn units_symbol(units: &str) -> Option<String> {
            units
                .split_once('(')
                .and_then(|(_, delimited)| delimited.split_once(')'))
                .map(|(symbol, _)| symbol.to_owned())
        }

        if let Some(symbol) = units_symbol(units) {
            match self {
                ScaleOneE::One => symbol,
                ScaleOneE::Three => format!("{symbol} {X_THREE}"),
                ScaleOneE::Six => format!("{symbol} {X_SIX}"),
                ScaleOneE::Nine => format!("{symbol} {X_NINE}"),
                ScaleOneE::Twelve => format!("{symbol} {X_TWELVE}"),
                ScaleOneE::Fifteen => format!("{symbol} {X_FIFTEEN}"),
            }
        } else {
            match self {
                ScaleOneE::One => "",
                ScaleOneE::Three => X_THREE,
                ScaleOneE::Six => X_SIX,
                ScaleOneE::Nine => X_NINE,
                ScaleOneE::Twelve => X_TWELVE,
                ScaleOneE::Fifteen => X_FIFTEEN,
            }
            .to_owned()
        }
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[cfg_attr(
    not(feature = "wasm"),
    expect(dead_code, reason = "exported only for wasm")
)]
pub fn scale_factor(min: f64, units: &str) -> u64 {
    Scale::new(min, units).factor()
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[cfg_attr(
    not(feature = "wasm"),
    expect(dead_code, reason = "exported only for wasm")
)]
pub fn scale_units(min: f64, units: &str) -> String {
    Scale::new(min, units).units(units)
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[cfg_attr(
    not(feature = "wasm"),
    expect(dead_code, reason = "exported only for wasm")
)]
pub fn scale_units_symbol(min: f64, units: &str) -> String {
    Scale::new(min, units).units_symbol(units)
}

enum Position {
    Whole(usize),
    Point,
    Decimal,
}

fn format_number(number: f64, trim_decimal: bool) -> String {
    let mut number_str = String::new();
    let mut position = Position::Decimal;
    for c in format!("{:.2}", number.abs()).chars().rev() {
        match position {
            Position::Whole(place) => {
                if place % 3 == 0 {
                    number_str.push(',');
                }
                position = Position::Whole(place + 1);
            },
            Position::Point => {
                position = Position::Whole(1);
            },
            Position::Decimal => {
                if c == '.' {
                    position = Position::Point;
                }
            },
        }
        number_str.push(c);
    }
    if number < 0.0 {
        number_str.push('-');
    }
    if trim_decimal && number_str.starts_with("00.") {
        number_str
            .chars()
            .collect::<Vec<_>>()
            .into_iter()
            .skip(3)
            .rev()
            .collect()
    } else {
        number_str.chars().rev().collect()
    }
}

#[cfg(test)]
mod tests {
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use crate::ResourceName;

    use super::{BYTES, DECIBELS, NANOSECONDS, PERCENTAGE, SECONDS, Units};

    const ONE: f64 = 1.0;
    const THOUSAND: f64 = 1_000.0;
    const MILLION: f64 = 1_000_000.0;
    const BILLION: f64 = 1_000_000_000.0;
    const TRILLION: f64 = 1_000_000_000_000.0;
    const QUADRILLION: f64 = 1_000_000_000_000_000.0;
    const MINUTE_NANOS: f64 = 60_000_000_000.0;
    const HOUR_NANOS: f64 = 3_600_000_000_000.0;
    const MINUTE_SECS: f64 = 60.0;
    const HOUR_SECS: f64 = 3_600.0;

    const GENERIC_UNITS: &str = "widgets (w)";
    const GENERIC_UNITS_NO_SYMBOL: &str = "widgets";
    const GENERIC_UNITS_UNCLOSED_SYMBOL: &str = "broken (oops";

    fn units(min: f64, units: &str) -> Units {
        Units::new(
            min,
            units.parse::<ResourceName>().expect("valid resource name"),
        )
    }

    fn assert_scale(
        min: f64,
        units_str: &str,
        expected_factor: f64,
        expected_units: &str,
        expected_symbol: &str,
    ) {
        let units = units(min, units_str);
        assert_eq!(
            OrderedFloat::from(expected_factor),
            units.scale_factor(),
            "scale factor for `{units_str}` at min {min}"
        );
        assert_eq!(
            expected_units,
            units.scale_units(),
            "scale units for `{units_str}` at min {min}"
        );
        assert_eq!(
            expected_symbol,
            units.scale_units_symbol(),
            "scale units symbol for `{units_str}` at min {min}"
        );
    }

    #[test]
    fn nanoseconds_scale_boundaries() {
        let cases = [
            (-1.0, ONE, "nanoseconds (ns)", "ns"),
            (0.0, ONE, "nanoseconds (ns)", "ns"),
            (1.0, ONE, "nanoseconds (ns)", "ns"),
            (999.999, ONE, "nanoseconds (ns)", "ns"),
            (THOUSAND, THOUSAND, "microseconds (µs)", "µs"),
            (999_999.0, THOUSAND, "microseconds (µs)", "µs"),
            (MILLION, MILLION, "milliseconds (ms)", "ms"),
            (999_999_999.0, MILLION, "milliseconds (ms)", "ms"),
            (BILLION, BILLION, "seconds (s)", "s"),
            (59_999_999_999.0, BILLION, "seconds (s)", "s"),
            (MINUTE_NANOS, MINUTE_NANOS, "minutes (m)", "m"),
            (3_599_999_999_999.0, MINUTE_NANOS, "minutes (m)", "m"),
            (HOUR_NANOS, HOUR_NANOS, "hours (h)", "h"),
            (f64::MAX, HOUR_NANOS, "hours (h)", "h"),
        ];
        for (min, factor, scale_units, symbol) in cases {
            assert_scale(min, NANOSECONDS, factor, scale_units, symbol);
        }
    }

    #[test]
    fn seconds_scale_boundaries() {
        let cases = [
            (0.0, ONE, "seconds (s)", "s"),
            (59.999, ONE, "seconds (s)", "s"),
            (MINUTE_SECS, MINUTE_SECS, "minutes (m)", "m"),
            (3_599.999, MINUTE_SECS, "minutes (m)", "m"),
            (HOUR_SECS, HOUR_SECS, "hours (h)", "h"),
            (f64::MAX, HOUR_SECS, "hours (h)", "h"),
        ];
        for (min, factor, scale_units, symbol) in cases {
            assert_scale(min, SECONDS, factor, scale_units, symbol);
        }
    }

    #[test]
    fn bytes_scale_boundaries() {
        let cases = [
            (0.0, ONE, "bytes (B)", "B"),
            (999.0, ONE, "bytes (B)", "B"),
            (THOUSAND, THOUSAND, "kilobytes (KB)", "KB"),
            (999_999.0, THOUSAND, "kilobytes (KB)", "KB"),
            (MILLION, MILLION, "megabytes (MB)", "MB"),
            (999_999_999.0, MILLION, "megabytes (MB)", "MB"),
            (BILLION, BILLION, "gigabytes (GB)", "GB"),
            (999_999_999_999.0, BILLION, "gigabytes (GB)", "GB"),
            (TRILLION, TRILLION, "terabytes (TB)", "TB"),
            (999_999_999_999_999.0, TRILLION, "terabytes (TB)", "TB"),
            (QUADRILLION, QUADRILLION, "petabytes (PB)", "PB"),
            (f64::MAX, QUADRILLION, "petabytes (PB)", "PB"),
        ];
        for (min, factor, scale_units, symbol) in cases {
            assert_scale(min, BYTES, factor, scale_units, symbol);
        }
    }

    #[test]
    fn generic_scale_boundaries() {
        let cases = [
            (0.0, ONE, "widgets (w)", "w"),
            (999.0, ONE, "widgets (w)", "w"),
            (THOUSAND, THOUSAND, "widgets (w) x 1e3", "w x 1e3"),
            (999_999.0, THOUSAND, "widgets (w) x 1e3", "w x 1e3"),
            (MILLION, MILLION, "widgets (w) x 1e6", "w x 1e6"),
            (BILLION, BILLION, "widgets (w) x 1e9", "w x 1e9"),
            (TRILLION, TRILLION, "widgets (w) x 1e12", "w x 1e12"),
            (QUADRILLION, QUADRILLION, "widgets (w) x 1e15", "w x 1e15"),
            (f64::MAX, QUADRILLION, "widgets (w) x 1e15", "w x 1e15"),
        ];
        for (min, factor, scale_units, symbol) in cases {
            assert_scale(min, GENERIC_UNITS, factor, scale_units, symbol);
        }
    }

    #[test]
    fn generic_scale_without_symbol() {
        let cases = [
            (0.0, ONE, "widgets", ""),
            (THOUSAND, THOUSAND, "widgets x 1e3", "x 1e3"),
            (QUADRILLION, QUADRILLION, "widgets x 1e15", "x 1e15"),
        ];
        for (min, factor, scale_units, symbol) in cases {
            assert_scale(min, GENERIC_UNITS_NO_SYMBOL, factor, scale_units, symbol);
        }
    }

    #[test]
    fn generic_scale_unclosed_symbol() {
        // An opening parenthesis without a closing one yields no symbol.
        let cases = [
            (0.0, ONE, "broken (oops", ""),
            (THOUSAND, THOUSAND, "broken (oops x 1e3", "x 1e3"),
        ];
        for (min, factor, scale_units, symbol) in cases {
            assert_scale(
                min,
                GENERIC_UNITS_UNCLOSED_SYMBOL,
                factor,
                scale_units,
                symbol,
            );
        }
    }

    #[test]
    fn percentage_and_decibels_use_generic_scaling() {
        assert_scale(0.0, PERCENTAGE, ONE, "percentage (%)", "%");
        assert_scale(
            THOUSAND,
            PERCENTAGE,
            THOUSAND,
            "percentage (%) x 1e3",
            "% x 1e3",
        );
        assert_scale(0.0, DECIBELS, ONE, "decibels (dB)", "dB");
        assert_scale(
            MILLION,
            DECIBELS,
            MILLION,
            "decibels (dB) x 1e6",
            "dB x 1e6",
        );
    }

    #[test]
    fn nan_min_selects_largest_scale() {
        // NaN fails every `<` comparison, so it falls through to the largest scale.
        assert_scale(f64::NAN, NANOSECONDS, HOUR_NANOS, "hours (h)", "h");
        assert_scale(f64::NAN, SECONDS, HOUR_SECS, "hours (h)", "h");
        assert_scale(f64::NAN, BYTES, QUADRILLION, "petabytes (PB)", "PB");
        assert_scale(
            f64::NAN,
            GENERIC_UNITS,
            QUADRILLION,
            "widgets (w) x 1e15",
            "w x 1e15",
        );
    }

    #[test]
    fn format_float_pads_two_decimals() {
        assert_eq!("0.00", Units::format_float(0.0));
        assert_eq!("1.00", Units::format_float(1.0));
        assert_eq!("999.00", Units::format_float(999.0));
        assert_eq!("0.50", Units::format_float(0.5));
    }

    #[test]
    fn format_float_inserts_commas() {
        assert_eq!("1,000.00", Units::format_float(1_000.0));
        assert_eq!("123,456.00", Units::format_float(123_456.0));
        assert_eq!("1,234,567.00", Units::format_float(1_234_567.0));
        assert_eq!("1,000,000,000.00", Units::format_float(1_000_000_000.0));
    }

    #[test]
    fn format_float_rounds_to_two_decimals() {
        assert_eq!("12,345.68", Units::format_float(12_345.678));
        // Rounding happens before comma insertion.
        assert_eq!("1,000.00", Units::format_float(999.999));
    }

    #[test]
    fn format_float_negative() {
        assert_eq!("-1.00", Units::format_float(-1.0));
        assert_eq!("-0.50", Units::format_float(-0.5));
        assert_eq!("-1,234.50", Units::format_float(-1_234.5));
    }

    #[test]
    fn format_number_trims_whole_decimal() {
        assert_eq!("0", Units::format_number(0.0, true));
        assert_eq!("5", Units::format_number(5.0, true));
        assert_eq!("1,000", Units::format_number(1_000.0, true));
        assert_eq!("-5", Units::format_number(-5.0, true));
        assert_eq!("-1,000", Units::format_number(-1_000.0, true));
    }

    #[test]
    fn format_number_keeps_nonzero_decimal() {
        assert_eq!("5.25", Units::format_number(5.25, true));
        // Only an all-zero decimal (".00") is trimmed; trailing zeros remain.
        assert_eq!("5.10", Units::format_number(5.1, true));
        assert_eq!("0.50", Units::format_number(0.5, true));
    }

    #[test]
    fn format_number_no_trim_keeps_decimal() {
        assert_eq!("5.00", Units::format_number(5.0, false));
        assert_eq!("1,000.00", Units::format_number(1_000.0, false));
    }
}
