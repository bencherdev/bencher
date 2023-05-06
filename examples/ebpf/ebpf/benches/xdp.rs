use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use bencher_adapter::{AdapterResults, JsonMetric};
use ebpf::Process;
use tokio::runtime::Runtime;

const IFACE: &str = "ens160";

#[derive(Debug)]
pub struct EBpfBenchmark {
    pub name: &'static str,
    pub benchmark_fn: fn() -> f64,
}

inventory::collect!(EBpfBenchmark);
inventory::submit!(EBpfBenchmark {
    name: "fun_xdp",
    benchmark_fn: fun_xdp_benchmark
});

fn fun_xdp_benchmark() -> f64 {
    let rt = Runtime::new().unwrap();

    let shutdown = Arc::new(AtomicBool::new(false));
    let ebpf_shutdown = shutdown.clone();
    let process = rt.block_on(async { ebpf::run(IFACE, ebpf_shutdown).await.unwrap() });

    let _resp = rt.block_on(async { reqwest::get("https://bencher.dev").await.unwrap() });

    let bpf_stats = get_bpf_stats(&process);

    shutdown.store(true, Ordering::Relaxed);

    bpf_stats
}

fn get_bpf_stats(process: &Process) -> f64 {
    let fd_info = File::open(format!("/proc/{}/fdinfo/{}", process.pid, process.prog_fd)).unwrap();
    let reader = BufReader::new(fd_info);
    let (mut run_time_ns, mut run_ctn) = (None, None);
    for line in reader.lines().flatten() {
        if let Some(l) = line.strip_prefix("run_time_ns:") {
            run_time_ns = l.trim().parse::<u64>().ok();
        } else if let Some(l) = line.strip_prefix("run_cnt:") {
            run_ctn = l.trim().parse::<u64>().ok();
        }
    }

    match (run_time_ns, run_ctn) {
        (Some(run_time_ns), Some(run_ctn)) if run_ctn != 0 => run_time_ns as f64 / run_ctn as f64,
        _ => 0.0,
    }
}

// Enable stats
// `sudo sysctl -w kernel.bpf_stats_enabled=1`
// From the root of the repo:
// `cargo xtask build-ebpf --release`
// From within the `ebpf` directory:
// `cd ebpf`
// `sudo -E $(which cargo) bench`
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
