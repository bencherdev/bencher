#![allow(clippy::unit_arg)]

use bencher_adapter::{AdapterResults, JsonMetric};
use custom::play_game;

#[derive(Debug)]
pub struct CustomBenchmark {
    pub name: &'static str,
    pub benchmark_fn: fn(),
}

inventory::collect!(CustomBenchmark);
inventory::submit!(CustomBenchmark {
    name: "bench_play_game",
    benchmark_fn: bench_play_game
});

fn bench_play_game() {
    std::hint::black_box(for i in 1..=100 {
        play_game(i, false);
    });
}

fn main() {
    let mut results = Vec::new();

    for benchmark in inventory::iter::<CustomBenchmark> {
        let benchmark_name = benchmark.name.parse().unwrap();
        (benchmark.benchmark_fn)();
        let json_metric = JsonMetric::new(0.0, None, None);
        println!("{benchmark_name}: {json_metric}");
        results.push((benchmark_name, json_metric));
    }

    let adapter_results = AdapterResults::new_latency(results).unwrap();
    let adapter_results_str = serde_json::to_string_pretty(&adapter_results).unwrap();
    println!("{adapter_results_str}");
}
