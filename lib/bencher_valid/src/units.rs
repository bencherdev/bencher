use ordered_float::OrderedFloat;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use crate::ResourceName;

pub const NANOSECONDS: &str = "nanoseconds (ns)";
pub const SECONDS: &str = "seconds (s)";
pub const BYTES: &str = "bytes (B)";

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

    #[expect(clippy::cast_precision_loss)]
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
    #[expect(clippy::cast_precision_loss)]
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

#[expect(dead_code)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn scale_factor(min: f64, units: &str) -> u64 {
    Scale::new(min, units).factor()
}

#[expect(dead_code)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn scale_units(min: f64, units: &str) -> String {
    Scale::new(min, units).units(units)
}

#[expect(dead_code)]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
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
