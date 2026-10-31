use std::collections::HashMap;

use bencher_json::{
    BenchmarkName, ParameterSet,
    project::{
        measure::built_in::{BuiltInMeasure as _, default::Latency},
        report::Adapter,
    },
};

use crate::{
    Adaptable, Settings,
    adapters::{bmf_v1::leaf::LeafResults, util::Units},
    results::adapter_results::AdapterResults,
};

/// `console.time` at version 1: each timer's duration, a single measurement, as `value`, from the
/// lines Firefox, Node, Deno, and Bun print when a timer ends.
///
/// A label that ends more than once keeps its last line, since a timer's `timeLog` lines print
/// before its end and alike in Node, Deno, and Bun, and each earlier line is counted as skipped.
/// Those runtimes print a timer's end with no suffix, so any other line shaped like one
/// (`retry in: 5s`) reads as a timer, and so can the data a `timeLog` call prints after its label
/// (Bun's `[0.00ms] log extra 42`).
pub struct AdapterJsTimeV1;

impl Adaptable for AdapterJsTimeV1 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut timers = HashMap::new();
        let mut repeats = 0;
        for (label, nanos) in input.lines().filter_map(parse_line) {
            if timers.insert(label, nanos).is_some() {
                repeats += 1;
            }
        }
        let mut results = LeafResults::default();
        for _ in 0..repeats {
            results.skip_variant();
        }
        for (label, nanos) in timers {
            let Ok(benchmark_name) = label.parse::<BenchmarkName>() else {
                results.skip_benchmark();
                continue;
            };
            results
                .variant(benchmark_name, ParameterSet::default())
                .insert(Latency::name_id(), "value", nanos);
        }
        results.finish(Adapter::JsTime)
    }
}

/// A line's label and duration in nanoseconds: Bun's `[1.50s] label`, Firefox's
/// `label: 7617ms - timer ended`, and Node's and Deno's `label: 1.502s`, with Node's
/// `label: 1:01.609 (m:ss.mmm)` and `label: 1:00:08.681 (h:mm:ss.mmm)` past a minute.
fn parse_line(line: &str) -> Option<(&str, f64)> {
    if let Some((duration, label)) = line
        .strip_prefix('[')
        .and_then(|rest| rest.split_once("] "))
        && let Some(nanos) = duration_nanos(duration)
    {
        return Some((label, nanos));
    }
    let line = line.strip_suffix(" - timer ended").unwrap_or(line);
    let (label, duration) = line.rsplit_once(": ")?;
    Some((
        label,
        duration_nanos(duration).or_else(|| clock_nanos(duration))?,
    ))
}

/// A decimal number of milliseconds or seconds, such as `0.698ms` or `1.502s`.
fn duration_nanos(duration: &str) -> Option<f64> {
    let (number, units) = if let Some(number) = duration.strip_suffix("ms") {
        (number, Units::Milli)
    } else {
        (duration.strip_suffix('s')?, Units::Sec)
    };
    Some(decimal(number)? * units.as_nanos())
}

/// Node's clock past a minute, `m:ss.mmm (m:ss.mmm)` or `h:mm:ss.mmm (h:mm:ss.mmm)`.
fn clock_nanos(duration: &str) -> Option<f64> {
    let (clock, format) = duration.split_once(' ')?;
    let parts = clock.split(':').collect::<Vec<_>>();
    let (seconds, whole) = parts.split_last()?;
    let expected = match whole.len() {
        1 => "(m:ss.mmm)",
        2 => "(h:mm:ss.mmm)",
        _ => return None,
    };
    if format != expected {
        return None;
    }
    let mut minutes = 0.0;
    for part in whole {
        minutes = f64::mul_add(minutes, 60.0, decimal(part)?);
    }
    Some(minutes.mul_add(60.0, decimal(seconds)?) * Units::Sec.as_nanos())
}

/// Digits, with a fraction after a point.
fn decimal(number: &str) -> Option<f64> {
    let (whole, fraction) = number.split_once('.').unwrap_or((number, "0"));
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    if digits(whole) && digits(fraction) {
        number.parse().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod test_js_time_v1 {
    use bencher_json::project::{
        measure::built_in::{BuiltInMeasure as _, default::Latency},
        report::Adapter,
    };
    use pretty_assertions::assert_eq;

    use crate::adapters::bmf_v1::test_util::{assert_ignores_average, assert_variant, convert_v1};

    /// Kills a duration stored under any name but `value`, milliseconds or seconds misread,
    /// Node's clock past a minute or an hour refused or misread, a label cut at its first `: `,
    /// Bun's label read as a duration, a Node label in brackets read as Bun's, a `timeLog` line
    /// with data read as a timer, a repeated label that keeps its first line or goes uncounted,
    /// and a leaf that records no leaf.
    #[test]
    fn adapter_js_time_v1_runtimes() {
        for (suffix, repeats, expected) in [
            (
                "js/time/node.txt",
                2,
                vec![
                    ("default", 698_000.0),
                    ("sum loop", 2_682_000.0),
                    ("ratio: a/b", 1_502_000_000.0),
                    ("log", 68_000.0),
                    ("sleep 0", 1_137_000.0),
                    ("sleep 5", 5_115_000.0),
                    ("sleep 15", 15_106_000.0),
                    ("sleep 150", 150_282_000.0),
                    ("sleep 999", 1_000_000_000.0),
                    ("sleep 1000", 1_000_999_999.999_999_9),
                    ("sleep 9999", 10_009_000_000.0),
                    ("sleep 12345", 12_358_000_000.0),
                    ("sleep 61500", 61_609_000_000.0),
                    ("sleep 600500", 601_048_000_000.0),
                    ("sleep 3605000", 3_608_681_000_000.0),
                    ("loop", 354_000.0),
                    ("[cold] start", 871_000.0),
                ],
            ),
            (
                "js/time/deno.txt",
                2,
                vec![
                    ("default", 859_000.0),
                    ("sum loop", 4_139_999.999_999_999_5),
                    ("ratio: a/b", 1_504_000_000.0),
                    ("log", 386_000.0),
                    ("sleep 0", 4_850_000.0),
                    ("sleep 5", 6_130_000.0),
                    ("sleep 15", 16_100_000.000_000_002),
                    ("sleep 150", 151_000_000.0),
                    ("sleep 999", 1_000_000_000.0),
                    ("sleep 1000", 1_001_000_000.0),
                    ("sleep 9999", 9_999_000_000.0),
                    ("sleep 12345", 12_347_000_000.0),
                    ("sleep 61500", 62_185_000_000.0),
                    ("sleep 600500", 600_548_000_000.0),
                    ("sleep 3605000", 3_605_041_000_000.0),
                    ("loop", 776_000.0),
                ],
            ),
            (
                "js/time/bun.txt",
                2,
                vec![
                    ("default", 1_160_000.0),
                    ("sum loop", 1_910_000.0),
                    ("ratio: a/b", 1_500_000_000.0),
                    ("log extra", 0.0),
                    ("log", 210_000.0),
                    ("sleep 0", 1_240_000.0),
                    ("sleep 5", 5_070_000.0),
                    ("sleep 15", 15_070_000.0),
                    ("sleep 150", 150_230_000.0),
                    ("sleep 999", 1_000_330_000.0),
                    ("sleep 1000", 1_000_860_000.0),
                    ("sleep 9999", 10_000_000_000.0),
                    ("sleep 12345", 12_350_000_000.0),
                    ("sleep 61500", 172_930_000_000.0),
                    ("sleep 600500", 636_830_000_000.0),
                    ("sleep 3605000", 3_619_050_000_000.0),
                    ("loop", 170_000.0),
                ],
            ),
            (
                "js/time/four.txt",
                0,
                vec![
                    ("default", 7_617_000_000.0),
                    ("benchmark_1", 12_714_000_000.0),
                    ("benchmark 2", 9_034_000_000.0),
                    ("benchmark 3: The Third - timer/timerEnd", 8_827_000_000.0),
                ],
            ),
        ] {
            let results = convert_v1(Adapter::JsTime, suffix);
            assert_eq!(results.resolved_adapter, Some(Adapter::JsTime), "{suffix}");
            assert_eq!(results.skipped.variants, repeats, "{suffix}");
            assert_eq!(results.inner.len(), expected.len(), "{suffix}");
            for (label, nanos) in expected {
                assert_variant(
                    &results,
                    label,
                    "{}",
                    &[(Latency::SLUG_STR, vec![("value", nanos)])],
                );
            }
        }
    }

    /// Kills a program's own print of a negative duration, or of one with no digit before its
    /// point, read as a timer, though no runtime ends a timer with either.
    #[test]
    fn adapter_js_time_v1_prints() {
        let results = convert_v1(Adapter::JsTime, "js/time/prints.txt");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(results.inner.len(), 2);
        for (label, nanos) in [("sum loop", 6_468_000.0), ("sort", 1_114_000.0)] {
            assert_variant(
                &results,
                label,
                "{}",
                &[(Latency::SLUG_STR, vec![("value", nanos)])],
            );
        }
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_js_time_v1_ignores_average() {
        for suffix in [
            "js/time/bun.txt",
            "js/time/deno.txt",
            "js/time/four.txt",
            "js/time/node.txt",
        ] {
            assert_ignores_average(Adapter::JsTime, suffix);
        }
    }
}
