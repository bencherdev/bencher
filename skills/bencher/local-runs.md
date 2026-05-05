# Local Benchmark Runs

## Basic Usage

Run a benchmark command and submit results:
```bash
bencher run "cargo bench"
```

The default adapter (`magic`) auto-detects the output format.
To verify your setup without a real benchmark harness:
```bash
bencher run "bencher mock"
```

## Adapters

If auto-detection fails, specify the adapter explicitly with `--adapter`:

| Adapter | Language | Tool |
|---------|----------|------|
| `magic` | Any | Auto-detect (default) |
| `json` | Any | Bencher Metric Format (BMF) JSON |
| `c_sharp_dot_net` | C# | BenchmarkDotNet |
| `cpp_catch2` | C++ | Catch2 |
| `cpp_google` | C++ | Google Benchmark |
| `dart_benchmark_harness` | Dart | benchmark_harness |
| `go_bench` | Go | `go test -bench` |
| `java_jmh` | Java | JMH |
| `js_benchmark` | JavaScript | Benchmark.js |
| `js_time` | JavaScript | console.time / console.timeEnd |
| `python_asv` | Python | ASV |
| `python_pytest` | Python | pytest-benchmark |
| `ruby_benchmark` | Ruby | Benchmark module |
| `rust_bench` | Rust | libtest bench (nightly) |
| `rust_criterion` | Rust | Criterion.rs |
| `rust_iai` | Rust | Iai |
| `rust_iai_callgrind` | Rust | Iai-Callgrind |
| `shell_hyperfine` | Shell | Hyperfine |

Example:
```bash
bencher run --adapter rust_criterion "cargo bench"
```

## Multiple Iterations

Run the benchmark multiple times and aggregate results:
```bash
bencher run --iter 5 --fold mean "cargo bench"
```

`--fold` options: `mean`, `median`, `min`, `max`

## File-Based Input

If your benchmark writes output to a file instead of stdout:
```bash
bencher run --file results.json "cargo bench -- --output results.json"
```

Multiple files:
```bash
bencher run --file results1.json --file results2.json "run-benchmarks.sh"
```

## Build Time Tracking

Track how long compilation takes:
```bash
bencher run --build-time "cargo build --release"
```

Cannot be combined with `--file`.

## File Size Tracking

Track the size of a binary or artifact:
```bash
bencher run --file-size target/release/my-binary
```

Multiple paths:
```bash
bencher run --file-size target/release/binary1 --file-size target/release/binary2
```

Cannot be combined with `--file`.

## Shell vs Exec Mode

By default, commands run through the shell (`/bin/sh -c "..."`).

To run as a direct executable (no shell interpretation):
```bash
bencher run --exec cargo bench -- --output-format json
```

Customize the shell:
```bash
bencher run --shell /bin/bash --flag -c "cargo bench"
```

## Output Formats

| Format | Flag | Use Case |
|--------|------|----------|
| `human` | `--format human` | Terminal display (default) |
| `json` | `--format json` | Programmatic consumption |
| `html` | `--format html` | CI comment posting (GitLab, etc.) |

Suppress progress output and only print the final report:
```bash
bencher run --quiet --format json "cargo bench"
```

## Dry Run

Validate everything without saving data:
```bash
bencher run --dry-run "cargo bench"
```
