use bencher_json::{
    BenchmarkName,
    project::measure::built_in::{
        BuiltInMeasure as _,
        default::{BytesThroughput, Throughput},
    },
};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        cpp::{nanobench_json::AdapterCppNanobenchJson, nanobench_text::AdapterCppNanobenchText},
        finite::Finite,
        leaf::{LeafResults, Variant, parameter_set},
    },
    results::adapter_results::AdapterResults,
};

/// nanobench at version 1: its `templates::json()` render, then its markdown table.
pub struct AdapterCppNanobench;

impl Adaptable for AdapterCppNanobench {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterCppNanobenchJson::parse(input, settings)
            .or_else(|| AdapterCppNanobenchText::parse(input, settings))
    }
}

/// The variant of a result named `name`, with its title unless it is nanobench's default and its
/// `complexityN` when the user set it, or `None`, counted as skipped, when it cannot be built, it
/// holds more keys than a variant may, or it repeats one already read.
pub(super) fn variant<'r>(
    results: &'r mut LeafResults,
    title: &str,
    name: &str,
    complexity_n: Option<f64>,
) -> Option<Variant<'r>> {
    variant_with_context(results, title, name, complexity_n, &[])
}

/// [`variant`], with each `contextColumn()` variable the row sets as the user's key and its
/// string value. A variable named for one of the adapter's keys is skipped and counted.
pub(super) fn variant_with_context<'r>(
    results: &'r mut LeafResults,
    title: &str,
    name: &str,
    complexity_n: Option<f64>,
    context: &[(&str, &str)],
) -> Option<Variant<'r>> {
    let Ok(benchmark_name) = name.parse::<BenchmarkName>() else {
        results.skip_benchmark();
        return None;
    };
    let mut parameters = serde_json::Map::new();
    if title != DEFAULT_TITLE {
        parameters.insert(TITLE.to_owned(), title.into());
    }
    if let Some(complexity_n) = complexity_n.filter(|n| *n > 0.0) {
        parameters.insert(COMPLEXITY_N.to_owned(), complexity_n.into());
    }
    let mut clashes = 0;
    for (key, value) in context {
        if [TITLE, COMPLEXITY_N].contains(key) {
            clashes += 1;
        } else {
            parameters.insert((*key).to_owned(), (*value).into());
        }
    }
    let Some(parameters) = parameter_set(parameters) else {
        results.skip_variant();
        return None;
    };
    for _ in 0..clashes {
        results.skip_variant();
    }
    results.new_variant(benchmark_name, parameters)
}

/// The title nanobench gives a `Bench` the user did not title.
const DEFAULT_TITLE: &str = "benchmark";
const TITLE: &str = "@title";
const COMPLEXITY_N: &str = "@complexityN";

/// The rate of `unit`s per second, as nanobench's `<unit>/s`: bytes in `bytes-throughput`, and
/// any other unit in `throughput`.
pub(super) fn insert_rate(variant: &mut Variant<'_>, unit: &str, rate: Finite) {
    let measure = if unit == "byte" {
        BytesThroughput::name_id()
    } else {
        Throughput::name_id()
    };
    rate.insert(variant, &measure, "median", 1.0);
}
