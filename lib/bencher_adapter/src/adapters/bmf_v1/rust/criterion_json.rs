use bencher_json::{
    MeasureNameId, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{self, BytesThroughput, Cycles, Latency},
        },
        report::Adapter,
    },
};
use serde::Deserialize;

use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::leaf::{LeafResults, Variant},
        util::Units,
    },
    results::adapter_results::AdapterResults,
};

/// cargo-criterion's `--message-format=json`: each `benchmark-complete` message's
/// estimates with their intervals, and its throughput. The messages state no
/// confidence level, so the interval names carry none.
pub struct AdapterRustCriterionJson;

impl Adaptable for AdapterRustCriterionJson {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut results = LeafResults::default();
        for line in input.lines() {
            let Ok(Message::BenchmarkComplete(benchmark)) = serde_json::from_str(line.trim())
            else {
                continue;
            };
            let BenchmarkComplete {
                id,
                throughput,
                typical,
                mean,
                median,
                median_abs_dev,
                slope,
            } = *benchmark;
            let Ok(benchmark_name) = id.parse() else {
                results.skip_benchmark();
                continue;
            };
            let mut variant = results.variant(benchmark_name, ParameterSet::default());
            for (name, statistic) in [
                ("typical", Some(&typical)),
                ("mean", Some(&mean)),
                ("median", Some(&median)),
                ("median_abs_dev", Some(&median_abs_dev)),
                ("slope", slope.as_ref()),
            ] {
                if let Some(statistic) = statistic {
                    insert_estimate(&mut variant, name, statistic);
                }
            }
            for throughput in throughput {
                insert_throughput(&mut variant, &throughput, &typical);
            }
        }
        results.finish(Adapter::RustCriterionJson)
    }
}

fn insert_estimate(variant: &mut Variant<'_>, name: &str, statistic: &Statistic) {
    let Statistic {
        estimate,
        lower_bound,
        upper_bound,
        unit,
    } = statistic;
    let Some((measure, factor)) = measure_of(variant, unit, 1.0) else {
        return;
    };
    variant.insert(measure.clone(), name, estimate * factor);
    variant.insert(
        measure.clone(),
        &format!("{name}_ci_lower"),
        lower_bound * factor,
    );
    variant.insert(measure, &format!("{name}_ci_upper"), upper_bound * factor);
}

/// criterion's own rate: the bytes or elements of one iteration over its typical time,
/// which only a time can give.
fn insert_throughput(variant: &mut Variant<'_>, throughput: &Throughput, typical: &Statistic) {
    let measure = match throughput.unit.as_str() {
        "bytes" => BytesThroughput::name_id(),
        "elements" => default::Throughput::name_id(),
        _ => {
            variant.skip_measure();
            return;
        },
    };
    let Ok(units) = typical.unit.parse::<Units>() else {
        variant.skip_measure();
        return;
    };
    let per_second = throughput.per_iteration / units.as_secs();
    let rate = |time: f64| (time > 0.0).then(|| per_second / time);
    let (Some(estimate), Some(lower), Some(upper)) = (
        rate(typical.estimate),
        rate(typical.upper_bound),
        rate(typical.lower_bound),
    ) else {
        variant.skip_measure();
        return;
    };
    variant.insert(measure.clone(), "typical", estimate);
    variant.insert(measure.clone(), "typical_ci_lower", lower);
    variant.insert(measure, "typical_ci_upper", upper);
}

/// The measure a unit picks and its value in that measure's unit: a time is latency in
/// nanoseconds, a criterion rate is a bytes throughput, or for elements a throughput, per
/// second, `cycles` is cycles, and any other unit is a measure of its own name in that
/// unit, or `None`, counted as skipped, when the unit makes no measure name.
pub(super) fn measure_of(
    variant: &mut Variant<'_>,
    unit: &str,
    value: f64,
) -> Option<(MeasureNameId, f64)> {
    if let Ok(units) = unit.parse::<Units>() {
        return Some((Latency::name_id(), value * units.as_nanos()));
    }
    let (measure, factor) = match unit {
        "cycles" => (Cycles::name_id(), 1.0),
        "B/s" => (BytesThroughput::name_id(), 1.0),
        "KiB/s" => (BytesThroughput::name_id(), KIBI),
        "MiB/s" => (BytesThroughput::name_id(), KIBI * KIBI),
        "GiB/s" => (BytesThroughput::name_id(), KIBI * KIBI * KIBI),
        "KB/s" => (BytesThroughput::name_id(), KILO),
        "MB/s" => (BytesThroughput::name_id(), KILO * KILO),
        "GB/s" => (BytesThroughput::name_id(), KILO * KILO * KILO),
        "b/s" => (BytesThroughput::name_id(), 1.0 / BITS_PER_BYTE),
        "Kb/s" => (BytesThroughput::name_id(), KILO / BITS_PER_BYTE),
        "Mb/s" => (BytesThroughput::name_id(), KILO * KILO / BITS_PER_BYTE),
        "Gb/s" => (
            BytesThroughput::name_id(),
            KILO * KILO * KILO / BITS_PER_BYTE,
        ),
        "elem/s" => (default::Throughput::name_id(), 1.0),
        "Kelem/s" => (default::Throughput::name_id(), KILO),
        "Melem/s" => (default::Throughput::name_id(), KILO * KILO),
        "Gelem/s" => (default::Throughput::name_id(), KILO * KILO * KILO),
        _ => (variant.named(unit, unit)?, 1.0),
    };
    Some((measure, value * factor))
}

const KILO: f64 = 1_000.0;
const KIBI: f64 = 1_024.0;
const BITS_PER_BYTE: f64 = 8.0;

#[derive(Deserialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
enum Message {
    BenchmarkComplete(Box<BenchmarkComplete>),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct BenchmarkComplete {
    id: String,
    throughput: Vec<Throughput>,
    typical: Statistic,
    mean: Statistic,
    median: Statistic,
    median_abs_dev: Statistic,
    slope: Option<Statistic>,
}

#[derive(Deserialize)]
struct Throughput {
    per_iteration: f64,
    unit: String,
}

#[derive(Deserialize)]
struct Statistic {
    estimate: f64,
    lower_bound: f64,
    upper_bound: f64,
    unit: String,
}

#[cfg(test)]
#[expect(
    clippy::unreadable_literal,
    reason = "estimates as the harness wrote them"
)]
mod test_rust_criterion_json {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{BytesThroughput, Cycles, Latency, Throughput},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_units, assert_variant, convert_v1, typical,
        },
    };

    use super::AdapterRustCriterionJson;

    /// Each estimate with its interval, in the message's order: typical, mean,
    /// median, median absolute deviation, then slope when present.
    fn estimates(measure: &'static str, estimates: &[[f64; 3]]) -> Expected {
        let names = [
            ["typical", "typical_ci_lower", "typical_ci_upper"],
            ["mean", "mean_ci_lower", "mean_ci_upper"],
            ["median", "median_ci_lower", "median_ci_upper"],
            [
                "median_abs_dev",
                "median_abs_dev_ci_lower",
                "median_abs_dev_ci_upper",
            ],
            ["slope", "slope_ci_lower", "slope_ci_upper"],
        ];
        (
            measure,
            names
                .into_iter()
                .zip(estimates)
                .flat_map(|(names, values)| names.into_iter().zip(*values))
                .collect(),
        )
    }

    /// A rate from the typical time in nanoseconds, as criterion computes it.
    fn rate(
        measure: &'static str,
        per_iteration: f64,
        [estimate, lower, upper]: [f64; 3],
    ) -> Expected {
        typical(
            measure,
            per_iteration * 1e9 / upper,
            per_iteration * 1e9 / estimate,
            per_iteration * 1e9 / lower,
        )
    }

    /// Kills a leaf that drops an estimate or its interval, misnames one, reads
    /// the throughput list as one measure, derives a rate from anything but the
    /// typical time, or records no leaf.
    #[test]
    fn adapter_rust_criterion_json() {
        for adapter in [Adapter::RustCriterion, Adapter::RustCriterionJson] {
            let results = convert_v1(adapter, "rust/criterion/json.txt");
            assert_eq!(
                results.resolved_adapter,
                Some(Adapter::RustCriterionJson),
                "{adapter}"
            );
            assert_eq!(results.inner.len(), 5, "{adapter}");
            assert_variant(
                &results,
                "checksum/fold/1024",
                "{}",
                &[
                    estimates(Latency::SLUG_STR, &FOLD_1024),
                    rate(BytesThroughput::SLUG_STR, 1024.0, FOLD_1024[0]),
                ],
            );
            assert_variant(
                &results,
                "checksum/sort/elements",
                "{}",
                &[
                    estimates(Latency::SLUG_STR, &SORT_ELEMENTS),
                    rate(Throughput::SLUG_STR, 4096.0, SORT_ELEMENTS[0]),
                ],
            );
            assert_variant(
                &results,
                "fib 15",
                "{}",
                &[estimates(Latency::SLUG_STR, &FIB_15)],
            );
        }
    }

    /// Kills a leaf that requires the slope, which flat sampling does not estimate.
    #[test]
    fn adapter_rust_criterion_json_flat() {
        let results = convert_v1(Adapter::RustCriterionJson, "rust/criterion/json_flat.txt");
        assert_eq!(results.inner.len(), 1);
        assert_variant(
            &results,
            "slow/fib 30",
            "{}",
            &[estimates(Latency::SLUG_STR, &FLAT)],
        );
    }

    /// Kills a leaf that ingests a change against the baseline.
    #[test]
    fn adapter_rust_criterion_json_change() {
        let results = convert_v1(Adapter::RustCriterionJson, "rust/criterion/json_change.txt");
        assert_eq!(results.inner.len(), 5);
        assert_variant(
            &results,
            "fib 15",
            "{}",
            &[estimates(Latency::SLUG_STR, &FIB_15_CHANGE)],
        );
    }

    /// Kills a leaf that reads cycles as a time, or derives a rate from them.
    #[test]
    fn adapter_rust_criterion_json_cycles_per_byte() {
        let results = convert_v1(
            Adapter::RustCriterionJson,
            "rust/criterion/json_cycles_per_byte.txt",
        );
        assert_eq!(results.inner.len(), 2);
        assert_variant(
            &results,
            "checksum/fold",
            "{}",
            &[estimates(Cycles::SLUG_STR, &CYCLES_FOLD)],
        );
        assert_eq!(results.skipped.measures, 1);
    }

    /// criterion-perf-events reports its counts as `events`, a measure of that name
    /// in that unit. Kills a leaf that creates the measure in the generic unit.
    #[test]
    fn adapter_rust_criterion_json_perf_events() {
        let results = convert_v1(
            Adapter::RustCriterionJson,
            "rust/criterion/json_perf_events.txt",
        );
        assert_eq!(results.inner.len(), 2);
        assert_variant(
            &results,
            "fib 15 branches",
            "{}",
            &[estimates("events", &PERF_BRANCHES)],
        );
        assert_units(&results, &[("events", "events")]);
    }

    /// Kills a node that tries the text leaf before the JSON leaf.
    #[test]
    fn adapter_rust_criterion_v1_tries_json_first() {
        let input = ["json.txt", "throughput.txt"]
            .map(|fixture| {
                std::fs::read_to_string(format!("./tool_output/rust/criterion/{fixture}"))
                    .expect("Failed to read a fixture")
            })
            .join("\n");
        let results = Adapter::RustCriterion
            .convert(&input, Settings::new(None, BmfVersion::V1))
            .expect("Failed to parse");
        assert_eq!(results.resolved_adapter, Some(Adapter::RustCriterionJson));
    }

    /// Kills a leaf that skips an id that is not a valid benchmark name without counting it.
    #[test]
    fn adapter_rust_criterion_json_invalid_id() {
        let input = std::fs::read_to_string("./tool_output/rust/criterion/json_flat.txt")
            .expect("Failed to read a fixture");
        let complete = input
            .lines()
            .find(|line| line.contains("benchmark-complete"))
            .expect("a benchmark");
        let mut invalid = serde_json::from_str::<serde_json::Value>(complete).expect("a message");
        invalid["id"] = "".into();
        let results = AdapterRustCriterionJson::parse(
            &format!("{complete}\n{invalid}\n"),
            Settings::default(),
        )
        .expect("Failed to parse");
        assert_eq!(results.inner.len(), 1);
        assert_eq!(results.skipped.benchmarks, 1);
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_rust_criterion_json_ignores_average() {
        for suffix in [
            "rust/criterion/json.txt",
            "rust/criterion/json_flat.txt",
            "rust/criterion/json_perf_events.txt",
        ] {
            assert_ignores_average(Adapter::RustCriterionJson, suffix);
        }
    }

    const FIB_15: [[f64; 3]; 5] = [
        [975.8024689184164, 968.4851662363993, 984.0012633714073],
        [979.5300901810912, 973.3608287711527, 986.3351257972156],
        [973.414406365121, 967.6817288801572, 977.6359436394891],
        [14.990214558236488, 11.555368773570931, 23.305311579434765],
        [975.8024689184164, 968.4851662363993, 984.0012633714073],
    ];
    const FOLD_1024: [[f64; 3]; 5] = [
        [45.92530368204718, 45.05960516094139, 46.781858443644495],
        [44.49133944927969, 44.03449131342898, 44.9929288986498],
        [43.358806740073405, 42.943570863492525, 44.08206533592474],
        [1.2343317181411229, 0.6706732327578604, 2.203627215622756],
        [45.92530368204718, 45.05960516094139, 46.781858443644495],
    ];
    const SORT_ELEMENTS: [[f64; 3]; 5] = [
        [1100.6364336508277, 1084.4513543937353, 1116.7861622985947],
        [1078.870832773923, 1068.4534390441052, 1089.8859191409258],
        [1042.6483457019172, 1040.9804112554111, 1047.8077922077923],
        [5.946504630209997, 3.2822556924169484, 13.970092592582441],
        [1100.6364336508277, 1084.4513543937353, 1116.7861622985947],
    ];
    const FLAT: [[f64; 3]; 4] = [
        [1322824.7174863387, 1307320.4273224042, 1338762.0420765027],
        [1322824.7174863387, 1307320.4273224042, 1338762.0420765027],
        [1322111.8032786886, 1298284.4590163934, 1346007.6857923497],
        [31393.689868880105, 8701.431906174548, 44183.41955821047],
    ];
    const FIB_15_CHANGE: [[f64; 3]; 5] = [
        [972.0184030252394, 961.6834818785459, 985.2494424369906],
        [970.7291238702123, 963.4007212525374, 978.9908514019761],
        [966.2532833565579, 953.4917637917638, 972.9639776462852],
        [29.249438814048233, 22.202454392877385, 39.50174582516283],
        [972.0184030252394, 961.6834818785459, 985.2494424369906],
    ];
    const CYCLES_FOLD: [[f64; 3]; 5] = [
        [799.9555387353934, 791.9071087644801, 810.1246607738481],
        [799.8983551585542, 792.0929141282458, 809.0927784391312],
        [786.3673495850622, 785.4966983447107, 787.1719027860107],
        [2.7059052931806336, 1.8394153711368404, 3.492010098053696],
        [799.9555387353934, 791.9071087644801, 810.1246607738481],
    ];
    const PERF_BRANCHES: [[f64; 3]; 5] = [
        [3949.0514936509667, 3949.036849796875, 3949.0653634261193],
        [3949.0358082350936, 3949.024377695225, 3949.047795492926],
        [3949.0243207225885, 3949.013008786364, 3949.035438723131],
        [
            0.05247204159122057,
            0.03871476287770783,
            0.06907393449092906,
        ],
        [3949.0514936509667, 3949.036849796875, 3949.0653634261193],
    ];
}
