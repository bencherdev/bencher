# `bencher noise` — Environment Noise Detection Subcommand

## Overview

`bencher noise` is a CLI subcommand that measures how "noisy" a computing environment is for benchmarking purposes. It runs a suite of synthetic micro-benchmarks, collects system metrics, and produces a composite noise score. Output is available as human-readable terminal output and as BMF (Bencher Metric Format) for tracking noise over time in Bencher itself.

The primary marketing goal is to make invisible benchmark noise visible, creating a natural funnel toward Bencher's bare metal runners.

## CLI Interface

```
bencher noise [OPTIONS]
```

### Options

| Flag                   | Default | Description                                       |
| ---------------------- | ------- | ------------------------------------------------- |
| `--duration <SECONDS>` | `60`    | Total measurement duration in seconds             |
| `--format <FORMAT>`    | `human` | Output format: `human` or `json` (BMF)            |
| `--quiet`              | `false` | Suppress progress output, only print final result |

When `--format json` is used, output valid BMF JSON to stdout so it can be piped directly into `bencher run`.

### Example Usage

```bash
# Quick local check
bencher noise

# In CI, track noise over time
bencher run --adapter json "bencher noise --format json"

# Short measurement
bencher noise --duration 15
```

## Architecture

### Module Structure

```
src/
  cli/
    noise/
      mod.rs          # Subcommand definition, CLI arg parsing, orchestration
      report.rs       # Human-readable terminal output formatting
      bmf.rs          # BMF JSON output formatting
      score.rs        # Composite noise score calculation
      benchmarks/
        mod.rs        # Benchmark suite runner, timing infrastructure
        compute.rs    # Pure CPU compute jitter benchmark
        cache.rs      # Cache-sensitive memory traversal benchmark
        io.rs         # Small I/O jitter benchmark
      platform/
        mod.rs        # Platform trait + detection logic
        linux.rs      # Linux-specific system metrics
        macos.rs      # macOS-specific system metrics
        windows.rs    # Windows-specific system metrics
```

### Platform Trait

```rust
pub struct PlatformMetrics {
    pub cpu_steal_percent: Option<f64>,
    pub context_switch_rate: Option<f64>,
    pub cache_miss_rate: Option<f64>,       // hardware counters, best effort
    pub is_virtualized: Option<bool>,
    pub virtualization_type: Option<String>, // "KVM", "Hyper-V", "VMware", "Docker", etc.
    pub cache_sizes: CacheSizes,
}

pub struct CacheSizes {
    pub l1d: Option<usize>,  // bytes
    pub l2: Option<usize>,
    pub l3: Option<usize>,
}

pub trait Platform {
    /// Detect cache sizes for the current CPU
    fn detect_cache_sizes(&self) -> CacheSizes;

    /// Collect system-level metrics over the given duration
    fn collect_metrics(&self, duration: Duration) -> PlatformMetrics;

    /// Detect if running in a virtualized environment
    fn detect_virtualization(&self) -> (Option<bool>, Option<String>);
}
```

## Synthetic Benchmarks

All benchmarks use `std::time::Instant` for timing and are pure Rust (no platform-specific code). Each benchmark runs many short iterations within the measurement window and reports timing statistics.

### 1. Compute Jitter (`benchmarks/compute.rs`)

**Purpose:** Measure CPU scheduling noise and steal time effects.

**Method:**
- Run a fixed-work computation (e.g., SHA-256 hashing of a fixed buffer, or iterative integer arithmetic) in a tight loop
- Each iteration does identical work
- Record wall-clock time per iteration
- Report: mean, stddev, coefficient of variation (CoV), min, max, p99

**Why this works:** In a quiet environment, each iteration takes nearly identical time. VM steal time, noisy neighbors competing for CPU, and scheduler interference all show up as variance.

### 2. Cache Jitter (`benchmarks/cache.rs`)

**Purpose:** Detect cache contention from noisy neighbors.

**Method:**
- Auto-detect L2 and L3 cache sizes from `Platform::detect_cache_sizes()` (fall back to reasonable defaults: 256KB L2, 8MB L3 if detection fails)
- Allocate an array sized to ~75% of L3 cache
- Repeatedly traverse it sequentially (cache-friendly reads)
- Record wall-clock time per traversal
- Report: same statistics as compute benchmark

**Why this works:** If the array fits in cache and nothing else is evicting it, traversal times are fast and consistent. Noisy neighbors with their own working sets cause cache evictions, increasing both absolute time and variance. The 75% sizing ensures the array *should* fit if the cache isn't under pressure.

**Additional signal (optional, stretch goal):**
- Run a second pass with large-stride access (e.g., stride = cache line size * 64) to intentionally cause cache misses
- The ratio of sequential-to-strided performance indicates how much cache pressure exists vs. baseline

### 3. I/O Jitter (`benchmarks/io.rs`)

**Purpose:** Measure scheduler and I/O subsystem noise.

**Method:**
- Repeatedly write and read a small temp file (or use `fsync` to ensure it hits the I/O path)
- Record wall-clock time per operation
- Report: same statistics

**Why this works:** I/O operations are sensitive to scheduler latency, VM I/O virtualization overhead, and contention on shared storage (common in CI).

### Timing Infrastructure (`benchmarks/mod.rs`)

Shared timing harness used by all benchmarks:

```rust
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: u64,
    pub mean_ns: f64,
    pub stddev_ns: f64,
    pub cov_percent: f64,  // coefficient of variation = stddev/mean * 100
    pub min_ns: f64,
    pub max_ns: f64,
    pub p99_ns: f64,
    pub samples: Vec<f64>, // raw timing samples in nanoseconds
}
```

Each benchmark runs in a loop for its share of the total duration (e.g., 20s each for a 60s total, or weighted differently). Warmup: first 10% of iterations are discarded.

## Platform Implementations

### Linux (`platform/linux.rs`) — Full Feature Set

| Metric                   | Source                                                                                                                                                                        |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Cache sizes              | `/sys/devices/system/cpu/cpu0/cache/index{0,1,2,3}/{size,type}`                                                                                                               |
| CPU steal time           | `/proc/stat` — parse `steal` field, sample at start and end of measurement                                                                                                    |
| Context switches         | `/proc/vmstat` — `ctxt` field, sample at start and end                                                                                                                        |
| Hardware cache misses    | `perf_event_open` syscall — `PERF_COUNT_HW_CACHE_MISSES` (best effort, requires `perf_event_paranoid <= 1` or `CAP_PERFMON`)                                                  |
| Virtualization detection | CPUID hypervisor bit + `/sys/class/dmi/id/product_name` + `/proc/cpuinfo` flags + check for `/.dockerenv` or `/run/.containerenv` + cgroup inspection for container detection |

### macOS (`platform/macos.rs`)

| Metric                   | Source                                                                                                                                   |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Cache sizes              | `sysctl` — `hw.l1dcachesize`, `hw.l2cachesize`, `hw.l3cachesize`                                                                         |
| CPU steal time           | Not available (report `None`)                                                                                                            |
| Context switches         | `host_statistics64` mach call — `context_switches` field                                                                                 |
| Hardware cache misses    | Not targeted for initial implementation (report `None`)                                                                                  |
| Virtualization detection | `sysctl kern.hv_vmm_present` (Hypervisor.framework) + `sysctl machdep.cpu.features` for VMM flag + check `ioreg` for VM-related hardware |

### Windows (`platform/windows.rs`)

| Metric                   | Source                                                                                                                                    |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Cache sizes              | `GetLogicalProcessorInformation` — `CACHE_DESCRIPTOR` entries                                                                             |
| CPU steal time           | Not available (report `None`)                                                                                                             |
| Context switches         | Performance Data Helper (PDH) — `\System\Context Switches/sec`                                                                            |
| Hardware cache misses    | Not targeted for initial implementation (report `None`)                                                                                   |
| Virtualization detection | CPUID hypervisor bit + WMI `Win32_ComputerSystem.Model` for "Virtual Machine" strings + check for Hyper-V, VMware, VirtualBox identifiers |

## Composite Noise Score

The composite score maps to a "decibel" scale from 0-100 for intuitive understanding.

### Inputs (all CoV percentages from the synthetic benchmarks)

| Component            | Weight | Rationale                                                                |
| -------------------- | ------ | ------------------------------------------------------------------------ |
| Compute jitter (CoV) | 0.30   | CPU scheduling is the most fundamental noise source                      |
| Cache jitter (CoV)   | 0.40   | Cache contention is the strongest "noisy neighbor" signal                |
| I/O jitter (CoV)     | 0.15   | I/O noise matters but is less universal                                  |
| CPU steal (%)        | 0.15   | Direct measure of VM overhead (0 when unavailable, weight redistributed) |

### Score Calculation (`score.rs`)

```
weighted_cov = sum(component_cov * weight)
noise_score = clamp(log_scale(weighted_cov), 0, 100)
```

Where `log_scale` maps CoV to a 0-100 range using a logarithmic curve (so that the difference between 0.1% and 1% CoV is perceptually similar to the difference between 1% and 10%).

Suggested thresholds for the human-readable report:

| Score  | Label      | Color  | Description                                     |
| ------ | ---------- | ------ | ----------------------------------------------- |
| 0-20   | Quiet      | Green  | Bare metal or well-isolated environment         |
| 21-50  | Moderate   | Yellow | Some noise, benchmarks may have minor variance  |
| 51-75  | Noisy      | Orange | Significant noise, benchmark results unreliable |
| 76-100 | Very Noisy | Red    | High contention, benchmarks are not meaningful  |

## Output Formats

### Human-Readable (default)

```
🔊 Bencher Noise Report
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Platform:        VM (KVM on AWS EC2)
  Duration:        60s
  CPU Caches:      L1d: 32KB | L2: 256KB | L3: 30MB

  Compute Jitter:  3.2% CoV   ██████░░░░░░░░░░░░░░  Moderate
  Cache Jitter:    12.8% CoV  █████████████░░░░░░░░  Noisy
  I/O Jitter:      8.1% CoV   ████████░░░░░░░░░░░░░  Noisy
  CPU Steal:       4.2%        ████░░░░░░░░░░░░░░░░░  Moderate
  Context Switches: 12,450/s

  Noise Score:     68 dB 🟠 Noisy
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  💡 Your environment has significant benchmark noise.
     Benchmarks here may vary ±12.8% between runs.
     Try Bencher bare metal runners for <1% jitter.
     https://bencher.dev/bare-metal
```

### BMF JSON (`--format json`)

```json
{
  "noise/compute_jitter": {
    "latency": {
      "value": 3.2,
      "lower_value": 1.1,
      "upper_value": 8.7
    }
  },
  "noise/cache_jitter": {
    "latency": {
      "value": 12.8,
      "lower_value": 5.2,
      "upper_value": 24.3
    }
  },
  "noise/io_jitter": {
    "latency": {
      "value": 8.1,
      "lower_value": 2.4,
      "upper_value": 18.9
    }
  },
  "noise/cpu_steal": {
    "latency": {
      "value": 4.2
    }
  },
  "noise/composite": {
    "latency": {
      "value": 68.0
    }
  }
}
```

The `value` is the CoV percentage for jitter metrics, the steal percentage for CPU steal, and the 0-100 score for composite. `lower_value` and `upper_value` represent the p5 and p95 of individual sample CoVs computed over rolling windows (provides a confidence interval for the noise level).

## Implementation Order

### Phase 1 — Core (Linux only, MVP)

1. CLI arg parsing and subcommand registration
2. Timing infrastructure (`BenchmarkResult`, warmup, statistics)
3. Compute jitter benchmark
4. Cache jitter benchmark (with auto-detection from `/sys/`)
5. I/O jitter benchmark
6. Linux platform metrics: steal time, context switches, virtualization detection
7. Composite score calculation
8. Human-readable output
9. BMF JSON output
10. Integration test: verify BMF output parses correctly and can be consumed by `bencher run`

### Phase 2 — Cross-Platform

11. macOS platform implementation (cache sizes via `sysctl`, context switches via mach calls, VM detection)
12. Windows platform implementation (cache sizes via `GetLogicalProcessorInformation`, context switches via PDH, VM detection)
13. Graceful degradation: when a metric isn't available, redistribute its weight and note it in the report

### Phase 3 — Polish

14. Linux hardware performance counters (best effort `perf_event_open`)
15. Cache stride benchmark (optional secondary signal)
16. Progress bar / live updating during measurement
17. `--quiet` flag for CI usage
18. Documentation and `bencher noise --help` text
19. Blog post / marketing page copy

## Testing Strategy

- **Unit tests:** Score calculation with known inputs, statistics functions, BMF serialization
- **Integration tests:** Run `bencher noise --duration 5 --format json` and validate BMF schema
- **Platform tests:** Mock `/proc/stat`, `sysctl` outputs, etc. for platform metric parsing
- **CI validation:** Run in GitHub Actions to verify it works in a real CI environment (and produces a high noise score, which is the point)

## Dependencies

Prefer minimal dependencies to keep the CLI lightweight:

- `std::time::Instant` for timing (no external crate needed)
- `serde` / `serde_json` for BMF output (already a Bencher dependency)
- `libc` for `perf_event_open` on Linux (best effort)
- `mach2` crate for macOS host_statistics (or raw FFI)
- `windows` crate for Windows API calls (or raw FFI)
- No dependency on `criterion` or other benchmark frameworks — this is intentionally a standalone measurement tool

## Open Questions

1. **BMF measure type:** Should jitter metrics use `latency` or a custom measure? Using `latency` works with existing Bencher infrastructure but the semantics are slightly different (it's a CoV, not an absolute latency).
2. **Score stability:** The log scale mapping needs empirical tuning. We should run `bencher noise` across a variety of environments (bare metal, EC2, GitHub Actions, Docker) to calibrate the thresholds.
3. **Warm-up strategy:** 10% of iterations may be too much or too little. Needs testing.
4. **Thread pinning:** Should the benchmarks attempt to pin to a single core for more consistent measurement? This helps isolate scheduling noise but may not reflect real benchmark conditions.
