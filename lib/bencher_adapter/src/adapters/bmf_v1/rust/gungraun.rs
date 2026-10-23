use bencher_json::BenchmarkName;

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        leaf::{LeafResults, Variant},
        rust::{
            gungraun_json::AdapterRustGungraunJsonV1, gungraun_stdout::AdapterRustGungraunStdoutV1,
        },
    },
    results::adapter_results::{AdapterResults, GungraunTool},
};

/// gungraun at version 1: its JSON summaries, then its terminal output.
pub struct AdapterRustGungraunV1;

impl Adaptable for AdapterRustGungraunV1 {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterRustGungraunJsonV1::parse(input, settings)
            .or_else(|| AdapterRustGungraunStdoutV1::parse(input, settings))
    }
}

/// The variant of one tool's run of a benchmark, with the benchmark's `id` when it has one,
/// or `None`, counted as skipped, if the parameters are not valid.
pub(super) fn tool_variant<'r>(
    results: &'r mut LeafResults,
    benchmark_name: BenchmarkName,
    id: Option<&str>,
    tool: GungraunTool,
) -> Option<Variant<'r>> {
    let mut parameters = serde_json::Map::new();
    if let Some(id) = id {
        parameters.insert("id".to_owned(), id.into());
    }
    parameters.insert("tool".to_owned(), tool.as_str().into());
    let Ok(parameters) = serde_json::from_value(serde_json::Value::Object(parameters)) else {
        results.skip_variant();
        return None;
    };
    Some(results.variant(benchmark_name, parameters))
}
