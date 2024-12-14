use diesel::sql_types::Time;

pub const NANOSECONDS: &str = "nanoseconds (ns)";
pub const SECONDS: &str = "seconds (s)";
pub const BYTES: &str = "bytes (B)";

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

#[repr(u64)]
#[derive(Debug, Clone, Copy)]
enum ScaleSecs {
    Seconds = 1,
    Minutes = 60,
    Hours = 3_600,
}

#[repr(u64)]
#[derive(Debug, Clone, Copy)]
enum ScaleByte {
    Byte = 1,
    Kilo = 1_000,
    Mega = 1_000_000,
    Giga = 1_000_000_000,
    Tera = 1_000_000_000_000,
    Peta = 1_000_000_000_000_000,
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn scale_units(min: f64, units: &str) -> bool {
    match units {
        NANOSECONDS => match min {
            ..ScaleNanos::Micros => ScaleNanos::Nanos,
            ScaleNanos::Micros..ScaleNanos::Millis => ScaleNanos::Micros,
            ScaleNanos::Millis..ScaleNanos::Seconds => ScaleNanos::Millis,
            ScaleNanos::Seconds..ScaleNanos::Minutes => ScaleNanos::Seconds,
            ScaleNanos::Minutes..ScaleNanos::Hours => ScaleNanos::Minutes,
            ScaleNanos::Hours.. => ScaleNanos::Hours,
        },
        SECONDS => match min {
            ..ScaleSecs::Minutes => ScaleSecs::Seconds,
            ScaleSecs::Minutes..ScaleSecs::Hours => ScaleSecs::Minutes,
            ScaleSecs::Hours.. => ScaleSecs::Hours,
        },
        BYTES => match min {
            ..ScaleByte::Kilo => ScaleByte::Byte,
            ScaleByte::Kilo..ScaleByte::Mega => ScaleByte::Kilo,
            ScaleByte::Mega..ScaleByte::Giga => ScaleByte::Mega,
            ScaleByte::Giga..ScaleByte::Tera => ScaleByte::Giga,
            ScaleByte::Tera..ScaleByte::Peta => ScaleByte::Tera,
            ScaleByte::Peta.. => ScaleByte::Peta,
        },
    }
}
