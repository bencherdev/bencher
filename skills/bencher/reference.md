# Bencher CLI Reference

## `bencher run` Flags

### Project and Branch

| Flag | Env Var | Default | Purpose |
|------|---------|---------|---------|
| `--project <slug>` | `BENCHER_PROJECT` | | Project slug or UUID |
| `--ci-on-the-fly` | | | Auto-create project in CI |
| `--branch <name>` | `BENCHER_BRANCH` | | Branch name/slug/UUID (auto-created) |
| `--hash <hash>` | | HEAD | Git commit hash |
| `--start-point <branch>` | | | Base branch for new branch |
| `--start-point-hash <hash>` | | | Full git hash for start point |
| `--start-point-max-versions <n>` | | 255 | Historical versions to include |
| `--start-point-clone-thresholds` | | | Clone thresholds from start point |
| `--start-point-reset` | | | Reset branch head to empty |

### Benchmark Configuration

| Flag | Env Var | Default | Purpose |
|------|---------|---------|---------|
| `--testbed <name>` | `BENCHER_TESTBED` | | Testbed name/slug/UUID (auto-created) |
| `--adapter <name>` | `BENCHER_ADAPTER` | `magic` | Benchmark harness adapter |
| `--average <type>` | | | Suggested central tendency |
| `--iter <n>` | | 1 | Number of run iterations |
| `--fold <fn>` | | | Aggregate function (requires `--iter`) |
| `--backdate <epoch>` | | | Backdate report (seconds since epoch) |
| `--allow-failure` | | | Allow benchmark command failure |

### Thresholds

| Flag | Purpose |
|------|---------|
| `--threshold-measure <measure>` | Measure for threshold |
| `--threshold-test <model>` | Statistical test model |
| `--threshold-upper-boundary <val>` | Upper boundary value |
| `--threshold-lower-boundary <val>` | Lower boundary value |
| `--error-on-alert` / `--err` | Exit non-zero on alert |

### Command Execution

| Flag | Env Var | Purpose |
|------|---------|---------|
| `--build-time` | | Track build time |
| `--file <path>` | | Read output from file (repeatable) |
| `--file-size <path>` | | Track file size (repeatable) |
| `--shell <path>` | | Shell command path |
| `--flag <flag>` | | Shell command flag |
| `--exec` | | Run as executable, not shell command |
| (trailing args) | `BENCHER_CMD` | Benchmark command |

### Output

| Flag | Default | Purpose |
|------|---------|---------|
| `--format <fmt>` | `human` | Output format: human, json, html |
| `--quiet` / `-q` | | Only output final report |
| `--dry-run` | | Validate without saving |

### CI Integration

| Flag | Purpose |
|------|---------|
| `--github-actions <token>` | GitHub Actions PR comments |
| `--ci-only-thresholds` | Post only if threshold exists |
| `--ci-only-on-alert` | Post only on alert |
| `--ci-public-links` | Use public URLs |
| `--ci-id <id>` | Custom CI comment identifier |
| `--ci-number <n>` | Issue/PR number |

### Bare Metal (Bencher Plus)

| Flag | Purpose |
|------|---------|
| `--image <ref>` | OCI image reference |
| `--spec <slug>` | Hardware spec |
| `--entrypoint <cmd>` | Container entrypoint override |
| `--env KEY=VALUE` | Environment variable (repeatable) |
| `--job-timeout <secs>` | Maximum execution time |
| `--job-poll-interval <secs>` | Poll interval |
| `--detach` | Submit without waiting |

### Backend

| Flag | Env Var | Default | Purpose |
|------|---------|---------|---------|
| `--host <url>` | `BENCHER_HOST` | `https://api.bencher.dev` | API host |
| `--token <jwt>` | `BENCHER_API_TOKEN` | | User API token |
| `--key <key>` | `BENCHER_API_KEY` | | Project API key (preferred) |
| `--timeout <secs>` | | 15 | Request timeout |
| `--attempts <n>` | | 10 | Request attempts |
| `--retry-after <secs>` | | 1 | Initial backoff time |

## Adapters

| Adapter | Language | Tool |
|---------|----------|------|
| `magic` | Any | Auto-detect |
| `json` | Any | Bencher Metric Format (BMF) |
| `c_sharp_dot_net` | C# | BenchmarkDotNet |
| `cpp_catch2` | C++ | Catch2 |
| `cpp_google` | C++ | Google Benchmark |
| `dart_benchmark_harness` | Dart | benchmark_harness |
| `go_bench` | Go | `go test -bench` |
| `java_jmh` | Java | JMH |
| `js_benchmark` | JavaScript | Benchmark.js |
| `js_time` | JavaScript | console.time/timeEnd |
| `python_asv` | Python | ASV |
| `python_pytest` | Python | pytest-benchmark |
| `ruby_benchmark` | Ruby | Benchmark module |
| `rust_bench` | Rust | libtest bench (nightly) |
| `rust_criterion` | Rust | Criterion.rs |
| `rust_iai` | Rust | Iai |
| `rust_iai_callgrind` | Rust | Iai-Callgrind |
| `shell_hyperfine` | Shell | Hyperfine |

## Threshold Models

| Model | Slug | Use Case | Key Params |
|-------|------|----------|------------|
| Static | `static` | Fixed value | `lower_boundary`, `upper_boundary` |
| Percentage | `percentage` | % change from mean | `upper_boundary` (e.g., 0.10 = 10%) |
| z-score | `z` | Normal, large samples | `upper_boundary` (cumulative %) |
| Student's t | `t` | Normal, small samples | `upper_boundary` (cumulative %) |
| Log Normal | `log_normal` | Right-skewed (latency) | `upper_boundary` (cumulative %) |
| IQR | `iqr` | Outlier-robust | `upper_boundary` (IQR multiplier) |
| Delta IQR | `delta_iqr` | Change-based IQR | `upper_boundary` (delta multiplier) |

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `BENCHER_API_KEY` | Project-scoped API key (preferred for agents/CI) |
| `BENCHER_API_TOKEN` | User-scoped API token |
| `BENCHER_HOST` | API host URL |
| `BENCHER_PROJECT` | Default project |
| `BENCHER_BRANCH` | Default branch |
| `BENCHER_TESTBED` | Default testbed |
| `BENCHER_ADAPTER` | Default adapter |
| `BENCHER_CMD` | Default benchmark command |

## Common Resource Commands

```bash
# Projects
bencher project list
bencher project view <project>
bencher project create --org <org> --name <name>

# Branches
bencher branch list --project <project>
bencher branch view --project <project> <branch>
bencher branch create --project <project> --name <name>

# Testbeds
bencher testbed list --project <project>
bencher testbed view --project <project> <testbed>
bencher testbed create --project <project> --name <name>

# Reports
bencher report list --project <project>
bencher report view --project <project> <report>

# Alerts
bencher alert list --project <project>
bencher alert view --project <project> <alert>

# Thresholds
bencher threshold list --project <project>
bencher threshold view --project <project> <threshold>

# Archive/Unarchive (branches, testbeds, benchmarks, measures)
bencher archive --project <project> --branch <branch>
bencher unarchive --project <project> --branch <branch>

# Performance query
bencher perf --project <project> --branch <branch> --testbed <testbed> --benchmark <benchmark> --measure <measure>
```
