use crate::{
    adapters::util::{latency_as_nanos, parse_benchmark_name_chars, parse_f64, NomError, Units},
    results::adapter_results::AdapterResults,
    Adapter, Settings,
};

pub struct AdapterElixirBenchee;

impl Adapter for AdapterElixirBenchee {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {}
}
