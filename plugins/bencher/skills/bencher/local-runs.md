# Local Benchmark Runs

## Basic Usage

Run a benchmark command and submit results:
```bash
bencher run "cargo bench"
```

Without an API key, results go to a public `unclaimed` project that anyone can
view and post results to. With a user API key, the project goes under the user's
personal organization.

The default adapter (`magic`) auto-detects the output format.
To verify your setup without a real benchmark harness:
```bash
bencher run "bencher mock"
```

## Adapters

Adapters run on the server and read results at the project's BMF version
(see [BMF Version](#bmf-version)).
If auto-detection fails, specify the adapter explicitly with `--adapter`.
At version 1 each adapter writes every measure its harness reports, each with the
named metrics the harness prints. **Threshold first** is the metric to name with
`--threshold-metric` for the adapter's main measure, such as `latency`, or `throughput`
for a harness that reports only rates (see [thresholds.md](./thresholds.md)).

| Adapter | Language | Tool | Threshold first |
|---------|----------|------|-----------------|
| `magic` | Any | Auto-detect (default) | the detected adapter's |
| `json` | Any | Bencher Metric Format (BMF) JSON, at the project's version | a metric the payload writes |
| `json_v0` | Any | BMF version 0 JSON, on any project | none: plain threshold flags check `value` |
| `json_v1` | Any | BMF version 1 JSON, on any project | a metric the payload writes |
| `c_sharp_dot_net` | C# | BenchmarkDotNet | `median` |
| `cpp_catch2` | C++ | Catch2 (prefer `-r xml`) | `mean` |
| `cpp_google` | C++ | Google Benchmark | `median` with repetitions, else `mean` |
| `dart_benchmark_harness` | Dart | benchmark_harness | `mean` |
| `go_bench` | Go | `go test -bench` | `median` with `-count`, else `mean` |
| `java_jmh` | Java | JMH | `median` |
| `js_benchmark` | JavaScript | Benchmark.js | `mean` |
| `js_deno` | JavaScript | `deno bench --json` (version 1 only) | `mean` |
| `js_mitata` | JavaScript | mitata (version 1 only) | `median` |
| `js_time` | JavaScript | console.time / console.timeEnd | `value` |
| `js_tinybench` | JavaScript | tinybench (version 1 only) | `median` |
| `js_vitest` | JavaScript | Vitest | `median` |
| `python_asv` | Python | ASV | `median` |
| `python_pytest` | Python | pytest-benchmark | `median` |
| `ruby_benchmark` | Ruby | Benchmark module | `value` |
| `ruby_benchmark_ips` | Ruby | benchmark-ips (version 1 only) | `mean` |
| `rust_bench` | Rust | libtest bench (nightly) | `median` |
| `rust_criterion` | Rust | Criterion.rs | `typical` (text output), `median` (cargo-criterion JSON) |
| `rust_divan` | Rust | Divan (version 1 only) | `median` |
| `rust_iai` | Rust | Iai | `value` |
| `rust_gungraun` | Rust | Gungraun (alias: `rust_iai_callgrind`) | `value` |
| `shell_hyperfine` | Shell | Hyperfine | `median` |

Another measure of the same adapter, such as a rate, a memory measure, or a
custom unit, can write other metrics (often `mean` or `value`): the adapter's
section on the [adapters page](https://bencher.dev/docs/explanation/adapters/)
names the metric to threshold first for each of its measures.

An adapter marked version 1 only refuses a version 0 project with an error that
says how to move the project to version 1.

Example:
```bash
bencher run --adapter rust_criterion "cargo bench"
```

Harness notes:
- Run one harness per `bencher run`, and print anything of your own to stderr,
  so the adapter reads only the harness's output.
- Catch2: run with `-r xml`; its console output is read best effort.
- tinybench writes no output of its own: print
  `JSON.stringify(bench.tasks.map(t => ({ name: t.name, ...t.result })))`.
- mitata: `run({ format: { json: { samples: false, debug: false } } })`.
- Deno: `deno bench --json`, with no top-level print of your own.
- benchmark-ips: write `x.json!("results.json")` and read it with `--file results.json`.

Keep only some measures with `--measure` (repeatable, by name, slug, or UUID);
every other measure is dropped before it is stored:
```bash
bencher run --measure latency "cargo bench"
```

## BMF Version

Every project reads results at its Bencher Metric Format (BMF) version, and every
new project starts at version 1. With `--project` (or `BENCHER_PROJECT`),
`bencher run` reads the project's version; without it, the run is at version 1,
even with `--image`.
`--adapter json_v0` or `json_v1` sets the version itself, on any project. The run
builds its own results (`--build-time`, `--file-size`) and its threshold flags at
that version.
`bencher project view <project>` shows the version as `bmf_version`.

A BMF version 1 payload (`--adapter json`) maps each benchmark to a list of its
variants, each with optional `parameters` and its `measures`, and each measure maps
metric names to values:
```json
{
  "my_benchmark": [
    {
      "parameters": { "size": 64 },
      "measures": { "latency": { "value": 88.2 } }
    },
    {
      "parameters": { "size": 4096 },
      "measures": { "latency": { "value": 512.7 } }
    }
  ]
}
```

- `value` names a single number. A measure may hold up to 32 metric names,
  such as `median` and `p99`.
- `parameters` map up to 8 keys to a string, number, or boolean. Leave them out
  for a benchmark with a single variant.
- A version 0 payload maps each benchmark to its measures, each with a `value`
  (and optional `lower_value` and `upper_value`). On a version 1 project, send it
  with `--adapter json_v0`.
- Move an existing version 0 project to version 1 with
  `bencher project update <project> --bmf-version 1`. A user who can edit the
  project can do this; only a server admin can move it back. The project's
  existing thresholds check `value`, so they stop checking any measure whose
  adapter writes no `value` at version 1 (most harness adapters): recreate them
  with `--threshold-metric`.

## Mock Data

`bencher mock` writes BMF version 1 by default, with one `value` metric per measure:
```bash
# One variant of every benchmark per parameter set
bencher run "bencher mock --parameters '{\"size\": 64}' --parameters '{\"size\": 4096}'"

# BMF version 0, for a version 0 project (no --parameters)
bencher run "bencher mock --bmf-version 0"
```

## Multiple Iterations

Run the benchmark multiple times:
```bash
bencher run --iter 5 "cargo bench"
```

At version 1, every iteration is stored. `--fold` (`mean`, `median`, `min`, `max`)
folds the iterations into one result at BMF version 0 only; at version 1 it is
ignored with a warning on the report.

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

Track how long compilation takes (the `build-time` measure, metric `value`):
```bash
bencher run --build-time "cargo build --release"
```

Cannot be combined with `--file`.

## File Size Tracking

Track the size of a binary or artifact (the `file-size` measure, metric `value`):
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
