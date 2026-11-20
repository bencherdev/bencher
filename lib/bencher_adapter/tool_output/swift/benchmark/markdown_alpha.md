[0/8] Compiling CSystem shims.c
[1/8] Write swift-version-6A52383ED409C6A7.txt
[2/8] Wrapping AST for ArgumentParserToolInfo for debugging
[3/8] Wrapping AST for SystemPackage for debugging
[4/8] Wrapping AST for ArgumentParser for debugging
[6/10] Emitting module BenchmarkBoilerplateGenerator
[7/10] Compiling BenchmarkBoilerplateGenerator BenchmarkBoilerplateGenerator.swift
[8/11] Wrapping AST for BenchmarkBoilerplateGenerator for debugging
[9/11] Write Objects.LinkFileList
[10/11] Linking BenchmarkBoilerplateGenerator-tool
Building for debugging...
[0/2] Write swift-version-6A52383ED409C6A7.txt
[1/3] Wrapping AST for ArgumentParser for debugging
[2/10] Wrapping AST for ArgumentParserToolInfo for debugging
[3/10] Compiling CSystem shims.c
[5/12] Emitting module BenchmarkShared
[6/12] Compiling BenchmarkShared Command+Helpers.swift
[7/13] Wrapping AST for SystemPackage for debugging
[9/47] Compiling Benchmark BenchmarkThresholds+Defaults.swift
[10/47] Compiling Benchmark BenchmarkThresholds.swift
[11/49] Compiling Benchmark ARCStats.swift
[12/49] Compiling Benchmark ARCStatsProducer.swift
[13/49] Compiling Benchmark AllocationStackRecorder.swift
[14/49] Compiling Benchmark AllocationStackReport+Folded.swift
[15/49] Compiling Benchmark AllocationStackReport+Formatting.swift
[16/49] Compiling Benchmark AllocationStackReport.swift
[17/49] Emitting module Benchmark
[18/49] Compiling Benchmark Blackhole.swift
[19/49] Compiling Benchmark Diagnostics.swift
[20/49] Compiling Benchmark AllocationStackSymbolicator.swift
[21/49] Compiling Benchmark Benchmark+ConvenienceInitializers.swift
[22/49] Compiling Benchmark Benchmark.swift
[23/49] Compiling Benchmark Utilities.swift
[24/49] Compiling Benchmark Statistics.swift
[25/49] Compiling Benchmark BenchmarkRunner+ReadWrite.swift
[26/49] Compiling Benchmark BenchmarkRunner.swift
[27/49] Compiling Benchmark OperatingSystemStats.swift
[28/49] Compiling Benchmark OperatingSystemStatsProducer+Darwin.swift
[29/49] Compiling Benchmark BenchmarkMetric.swift
[30/49] Compiling Benchmark BenchmarkResult.swift
[31/49] Compiling Benchmark OperatingSystemStatsProducer+Linux.swift
[32/49] Compiling Benchmark OutputSuppressor.swift
[33/49] Compiling Benchmark Int+Extensions.swift
[34/49] Compiling Benchmark MallocStats+jemalloc-support.swift
[35/49] Compiling Benchmark MallocStats.swift
[36/49] Compiling Benchmark MallocStatsProducer+jemalloc.swift
[37/49] Compiling Benchmark Progress.swift
[38/49] Compiling Benchmark ProgressElements.swift
[39/49] Compiling Benchmark BenchmarkInternals.swift
[40/49] Compiling Benchmark BenchmarkMetric+Defaults.swift
[41/49] Compiling Benchmark NIOLock.swift
[42/49] Compiling Benchmark lock.swift
[43/49] Compiling Benchmark BenchmarkClock.swift
[44/49] Compiling Benchmark BenchmarkExecutor+Extensions.swift
[45/49] Compiling Benchmark BenchmarkExecutor.swift
[47/67] Compiling BenchmarkTool BenchmarkTool+Thresholds.swift
[48/68] Compiling BenchmarkTool BenchmarkTool+Machine.swift
[49/68] Compiling BenchmarkTool BenchmarkTool+Export+JMHFormatter.swift
[50/68] Compiling BenchmarkTool BenchmarkTool+Export+JMHElement.swift
[51/68] Compiling BenchmarkTool String+Additions.swift
[52/68] Compiling BenchmarkTool BenchmarkTool+CreateBenchmark.swift
[53/68] Compiling BenchmarkTool BenchmarkTool+Export+InfluxCSVFormatter.swift
[54/68] Compiling BenchmarkTool BenchmarkTool+JSON.swift
[55/68] Compiling BenchmarkTool BenchmarkTool+Export+JSON.swift
[56/68] Compiling BenchmarkTool BenchmarkTool+Export.swift
[57/68] Compiling BenchmarkTool BenchmarkTool+Operations.swift
[58/68] Compiling BenchmarkTool BenchmarkTool+PrettyPrinting.swift
[59/68] Emitting module BenchmarkTool
[60/68] Compiling BenchmarkTool BenchmarkTool+AllocationStacks.swift
[61/68] Compiling BenchmarkTool BenchmarkTool+Baselines.swift
[62/68] Compiling BenchmarkTool FilePath+Additions.swift
[63/68] Compiling BenchmarkTool FilePath+DirectoryView.swift
[64/68] Compiling BenchmarkTool BenchmarkTool+ReadP90AbsoluteThresholds.swift
[65/68] Compiling BenchmarkTool BenchmarkTool.swift
[66/68] Write Objects.LinkFileList
[67/68] Linking BenchmarkTool-tool
Build of product 'BenchmarkTool' complete! (2.51s)
[33mWarning: running with the Swift runtime interposer on Linux to avoid the Swift 6.3 runtime hook crash. See https://github.com/ordo-one/benchmark/issues/349[0m
intentional failure for the fixture


## Baseline 'Current_run'

```
Host 'localhost' with 16 'x86_64' processors with 15 GB memory, running:
#100-Ubuntu SMP PREEMPT_DYNAMIC Thu Jan  1 00:00:00 UTC 2026
```
## Alpha

### Custom metrics

| Metric                         |        p0 |       p25 |       p50 |       p75 |       p90 |       p99 |      p100 |   Samples |
|:-------------------------------|----------:|----------:|----------:|----------:|----------:|----------:|----------:|----------:|
| CacheHits                      |       999 |       980 |       955 |       926 |       908 |       900 |       900 |       100 |
| QueueDepth *                   |         1 |         4 |         7 |         9 |        11 |        12 |        12 |       100 |
| Time (wall clock) (ns) *       |       570 |       581 |       581 |       592 |       811 |       962 |      2294 |       100 |

### Fixed ms

| Metric                         |        p0 |       p25 |       p50 |       p75 |       p90 |       p99 |      p100 |   Samples |
|:-------------------------------|----------:|----------:|----------:|----------:|----------:|----------:|----------:|----------:|
| Throughput (# / s) (#)         |       644 |       643 |       643 |       640 |       638 |       632 |       632 |        50 |
| Time (total CPU) (ms) *        |         0 |         0 |         0 |         0 |         0 |         0 |         0 |        50 |
| Time (wall clock) (ms) *       |         2 |         2 |         2 |         2 |         2 |         2 |         2 |        50 |

### Insert (count: 1, kind: array)

| Metric                         |        p0 |       p25 |       p50 |       p75 |       p90 |       p99 |      p100 |   Samples |
|:-------------------------------|----------:|----------:|----------:|----------:|----------:|----------:|----------:|----------:|
| Malloc (total) *               |         1 |         1 |         1 |         1 |         1 |         1 |         1 |     10000 |
| Throughput (# / s) (M)         |        17 |        14 |        14 |        13 |        12 |         7 |         0 |     10000 |
| Time (wall clock) (ns) *       |        60 |        70 |        71 |        80 |        81 |       150 |      8233 |     10000 |

### Insert (count: 1, kind: set)

| Metric                         |        p0 |       p25 |       p50 |       p75 |       p90 |       p99 |      p100 |   Samples |
|:-------------------------------|----------:|----------:|----------:|----------:|----------:|----------:|----------:|----------:|
| Malloc (total) *               |         1 |         1 |         1 |         1 |         1 |         1 |         1 |     10000 |
| Throughput (# / s) (M)         |        11 |        11 |        11 |        10 |        10 |         6 |         0 |     10000 |
| Time (wall clock) (ns) *       |        89 |        90 |        91 |       100 |       100 |       170 |      7723 |     10000 |

### Insert (count: 4096, kind: array)

| Metric                         |        p0 |       p25 |       p50 |       p75 |       p90 |       p99 |      p100 |   Samples |
|:-------------------------------|----------:|----------:|----------:|----------:|----------:|----------:|----------:|----------:|
| Malloc (total) *               |        13 |        13 |        13 |        13 |        13 |        13 |        13 |     10000 |
| Throughput (# / s) (K)         |       380 |       374 |       371 |       370 |       333 |       233 |        28 |     10000 |
| Time (wall clock) (ns) *       |      2633 |      2677 |      2693 |      2705 |      3005 |      4287 |     35426 |     10000 |

### Insert (count: 4096, kind: set)

| Metric                         |        p0 |       p25 |       p50 |       p75 |       p90 |       p99 |      p100 |   Samples |
|:-------------------------------|----------:|----------:|----------:|----------:|----------:|----------:|----------:|----------:|
| Malloc (total) *               |        13 |        13 |        13 |        13 |        13 |        13 |        13 |     10000 |
| Throughput (# / s) (K)         |        12 |        11 |        11 |        11 |        10 |         8 |         5 |     10000 |
| Time (wall clock) (μs) *       |        85 |        88 |        90 |        93 |        96 |       120 |       200 |     10000 |

### Insert (count: 64, kind: array)

| Metric                         |        p0 |       p25 |       p50 |       p75 |       p90 |       p99 |      p100 |   Samples |
|:-------------------------------|----------:|----------:|----------:|----------:|----------:|----------:|----------:|----------:|
| Malloc (total) *               |         7 |         7 |         7 |         7 |         7 |         7 |         7 |     10000 |
| Throughput (# / s) (K)         |      3717 |      3572 |      3559 |      3449 |      3228 |      1752 |        80 |     10000 |
| Time (wall clock) (ns) *       |       269 |       280 |       281 |       290 |       310 |       571 |     12460 |     10000 |

### Insert (count: 64, kind: set)

| Metric                         |        p0 |       p25 |       p50 |       p75 |       p90 |       p99 |      p100 |   Samples |
|:-------------------------------|----------:|----------:|----------:|----------:|----------:|----------:|----------:|----------:|
| Malloc (total) *               |         7 |         7 |         7 |         7 |         7 |         7 |         7 |     10000 |
| Throughput (# / s) (K)         |       671 |       666 |       666 |       662 |       661 |       454 |        40 |     10000 |
| Time (wall clock) (ns) *       |      1491 |      1502 |      1503 |      1512 |      1513 |      2203 |     25280 |     10000 |

### Name, with "quotes"

| Metric                         |        p0 |       p25 |       p50 |       p75 |       p90 |       p99 |      p100 |   Samples |
|:-------------------------------|----------:|----------:|----------:|----------:|----------:|----------:|----------:|----------:|
| Throughput (# / s) (M)         |        17 |        14 |        14 |        14 |        14 |         3 |         3 |       100 |
| Time (wall clock) (ns) *       |        60 |        69 |        70 |        70 |        71 |        80 |       290 |       100 |

### Scaled kilo

| Metric                         |        p0 |       p25 |       p50 |       p75 |       p90 |       p99 |      p100 |   Samples |
|:-------------------------------|----------:|----------:|----------:|----------:|----------:|----------:|----------:|----------:|
| Instructions *                 |       271 |       271 |       271 |       271 |       271 |       271 |       271 |     10000 |
| Throughput (# / s) (M)         |        84 |        83 |        82 |        78 |        62 |        46 |        10 |     10000 |
| Time (wall clock) (ns) *       |        12 |        12 |        12 |        13 |        16 |        22 |        99 |     10000 |

### Sum array

| Metric                         |        p0 |       p25 |       p50 |       p75 |       p90 |       p99 |      p100 |   Samples |
|:-------------------------------|----------:|----------:|----------:|----------:|----------:|----------:|----------:|----------:|
| Free (total) *                 |        17 |        17 |        17 |        17 |        17 |        17 |        21 |     10000 |
| Instructions (K) *             |        17 |        17 |        17 |        17 |        17 |        17 |        20 |     10000 |
| Malloc (bytes total) (K) *     |        43 |        43 |        43 |        43 |        43 |        43 |        52 |     10000 |
| Malloc (total) *               |        17 |        17 |        17 |        17 |        17 |        17 |        21 |     10000 |
| Malloc / free Δ *              |         0 |         0 |         0 |         0 |         0 |         0 |         2 |     10000 |
| Malloc / free Δ (bytes) *      |         0 |         0 |         0 |         0 |         0 |         0 |      8272 |     10000 |
| Memory (resident peak) (M)     |        25 |        25 |        25 |        25 |        25 |        25 |        25 |     10000 |
| Throughput (# / s) (K)         |       198 |       197 |       197 |       196 |       195 |       118 |        35 |     10000 |
| Time (total CPU) (μs) *        |         0 |         0 |         0 |         0 |         0 |         0 |     10000 |     10000 |
| Time (wall clock) (ns) *       |      5038 |      5079 |      5091 |      5107 |      5119 |      8487 |     28184 |     10000 |

