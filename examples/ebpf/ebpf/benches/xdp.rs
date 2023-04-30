#![feature(test)]

use std::{fs::File, io::Write};

use bencher_adapter::{AdapterResults, JsonMetric};

extern crate test;

#[derive(Debug)]
pub struct EBpfBenchmark {
    pub name: &'static str,
    pub benchmark_fn: fn() -> f64,
}

inventory::collect!(EBpfBenchmark);

fn basic_benchmark() -> f64 {
    use tokio::runtime::Runtime;

    // Create the runtime
    let rt = Runtime::new().unwrap();

    // Spawn a blocking function onto the runtime
    let process = rt.block_on(async { ebpf::run("ens160").await.unwrap() });

    0.0
}

inventory::submit!(EBpfBenchmark {
    name: "basic",
    benchmark_fn: basic_benchmark
});

fn main() {
    let mut results = Vec::new();

    for benchmark in inventory::iter::<EBpfBenchmark> {
        let benchmark_name = benchmark.name.parse().unwrap();
        let json_metric = JsonMetric::new((benchmark.benchmark_fn)(), None, None);
        results.push((benchmark_name, json_metric));
    }

    let adapter_results = AdapterResults::new_latency(results).unwrap();
    let adapter_results_str = serde_json::to_string_pretty(&adapter_results).unwrap();
    println!("{}", adapter_results_str);

    // write to file
    // use the --file flag for the bencher CLI command
    let mut file = File::create("../target/results.json").unwrap();
    file.write_all(adapter_results_str.as_bytes()).unwrap();
}
