#![allow(clippy::unit_arg)]

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use custom::play_game;

#[derive(Debug)]
struct CustomBenchmark {
    name: &'static str,
    benchmark_fn: fn() -> dhat::HeapStats,
}
inventory::collect!(CustomBenchmark);

impl CustomBenchmark {
    fn run(&self) -> serde_json::Value {
        let heap_stats = (self.benchmark_fn)();
        let measures = serde_json::json!({
            "Final Blocks": {
                "value": heap_stats.curr_blocks,
            },
            "Final Bytes": {
                "value": heap_stats.curr_bytes,
            },
            "Max Blocks": {
                "value": heap_stats.max_blocks,
            },
            "Max Bytes": {
                "value": heap_stats.max_bytes,
            },
            "Total Blocks": {
                "value": heap_stats.total_blocks,
            },
            "Total Bytes": {
                "value": heap_stats.total_bytes,
            },
        });
        let mut benchmark_map = serde_json::Map::new();
        benchmark_map.insert(self.name.to_string(), measures);
        benchmark_map.into()
    }
}

fn bench_play_game() -> dhat::HeapStats {
    let _profiler = dhat::Profiler::builder().testing().build();

    std::hint::black_box(for i in 1..=100 {
        play_game(i, false);
    });

    dhat::HeapStats::get()
}
inventory::submit!(CustomBenchmark {
    name: "bench_play_game",
    benchmark_fn: bench_play_game
});

fn main() {
    let mut bmf = serde_json::Map::new();

    for benchmark in inventory::iter::<CustomBenchmark> {
        let mut results = benchmark.run();
        bmf.append(results.as_object_mut().unwrap());
    }

    let bmf_str = serde_json::to_string_pretty(&bmf).unwrap();
    std::fs::write("results.json", &bmf_str).unwrap();
    println!("{bmf_str}");
}
