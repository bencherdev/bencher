# Run a Benchmark

Now that you’ve installed the `bencher` CLI, let’s run your first benchmark with it.

## Select an Adapter

The `bencher` CLI has built in adapters for the following benchmarking tools:

- C++ Google `benchmark` (`cpp_google_benchmark`)
- Java OpenJDK `jmh` (`java_openjdk_jmh`)
- Python `pyperformance` (`python_pyperformance`)
- C# `BenchmarkDotNet` (`csharp_benchmarkdotnet`)
- JavaScript `benchmark.js` (`javascript_benchmarkjs`)
- JavaScript `jsperf.js` (`javascript_jsperfjs`)
- Go `testing` (`go_testing`)
- Swift `benchmark` (`swift_benchmark`)
- Rust `cargo bench` (`rust_cargo_bench`)
- Rust `criterion` (`rust_criterion`)

If your benchmarking tool is not on this list, you can still use the `bencher` CLI! See [creating a custom adapter](./custom_adapter.md) for how guidance on creating a custom adapter that outputs JSON in [Benchmark Metrics Format (BMF)](./benchmark_metrics_format.md).

## Run a Benchmark with `--local`

Now that we have our adapter selected, lets try to run our first benchmark. For example:

```
bencher run --local --adapter rust_cargo_bench "cargo bench"
```

Lets break this down:
- `bencher` - Invokes the `bencher` CLI.
- `run` - Invokes the `bencher run` sub-command.
- `--local` - Tells the CLI to run in local-only mode and not try to push your results to the Bencher REST API backend. We will get to that later in this tutorial.
- `--adapter rust_cargo_bench` - Tells the CLI to use the Rust `cargo bench` adapter.
- `"cargo bench"` - This is the command used to run the benchmarks.

Pluging in your adapter and benchmark command, you should see a Benchmark Metrics Format (BMF) output to the console. For example:

```
{"email":"","token":"","testbed":null,"date_time":"2022-06-20T15:52:15.249997Z","metrics":{"tests::benchmark_a":{"latency":{"duration":{"secs":0,"nanos":3425},"variance":{"secs":0,"nanos":3123}}},"tests::benchmark_b":{"latency":{"duration":{"secs":0,"nanos":9457},"variance":{"secs":0,"nanos":15420}}},"tests::benchmark_c":{"latency":{"duration":{"secs":0,"nanos":6381},"variance":{"secs":0,"nanos":20179}}}}}
```

Now that you have the `bencher` CLI running your benchmarks for you, lets [create a Bencher account](./04_bencher_account.md) to store your results.
