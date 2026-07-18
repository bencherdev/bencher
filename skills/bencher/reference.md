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
| `--threshold-min-sample-size <n>` | Minimum historical sample size |
| `--threshold-max-sample-size <n>` | Maximum historical sample size |
| `--threshold-window <secs>` | Time window in seconds |
| `--threshold-upper-boundary <val>` | Upper boundary value |
| `--threshold-lower-boundary <val>` | Lower boundary value |
| `--thresholds-reset` | Reset all unspecified thresholds |
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
| `--spec-reset` | Reset testbed spec (requires `--testbed`, conflicts with `--image`) |
| `--entrypoint <cmd>` | Container entrypoint override |
| `--env KEY=VALUE` | Environment variable (repeatable) |
| `--job-timeout <secs>` | Maximum execution time |
| `--job-poll-interval <secs>` | Poll interval |
| `--detach` | Submit without waiting |

### Backend

| Flag | Env Var | Default | Purpose |
|------|---------|---------|---------|
| `--host <url>` | `BENCHER_HOST` | `https://api.bencher.dev` | API host |
| `--token <jwt>` | `BENCHER_API_TOKEN` | | User API token (JWT). Deprecated; use `--key` |
| `--key <key>` | `BENCHER_API_KEY` | | Bencher API key: user (`bencher_user_*`) or project (`bencher_run_*`); project keys need `--project` or `--image` |
| `--insecure-host` | | | Allow insecure HTTPS connections |
| `--native-tls` | | | Use platform native TLS certificates |
| `--timeout <secs>` | | 15 | Request timeout (1-900) |
| `--attempts <n>` | | 35 | Request attempts |
| `--retry-after <secs>` | | 1 | Initial backoff time (1-900) |
| `--max-retry-after <secs>` | | 30 | Max backoff time (1-900) |
| `--strict` | | | Strictly parse JSON responses |

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
| `js_vitest` | JavaScript | Vitest |
| `python_asv` | Python | ASV |
| `python_pytest` | Python | pytest-benchmark |
| `ruby_benchmark` | Ruby | Benchmark module |
| `rust_bench` | Rust | libtest bench (nightly) |
| `rust_criterion` | Rust | Criterion.rs |
| `rust_iai` | Rust | Iai |
| `rust_gungraun` | Rust | Gungraun (alias: `rust_iai_callgrind`) |
| `shell_hyperfine` | Shell | Hyperfine |

## Threshold Models

| Model | Slug | Use Case | Key Params |
|-------|------|----------|------------|
| Static | `static` | Fixed value | `lower_boundary`, `upper_boundary` |
| Percentage | `percentage` | % change from mean | `upper_boundary` (e.g., 0.10 = 10%) |
| z-score | `z_score` (alias: `z`) | Normal, large samples | `upper_boundary` (cumulative %) |
| Student's t | `t_test` (alias: `t`) | Normal, small samples | `upper_boundary` (cumulative %) |
| Log Normal | `log_normal` | Right-skewed (latency) | `upper_boundary` (cumulative %) |
| IQR | `iqr` | Outlier-robust | `upper_boundary` (IQR multiplier) |
| Delta IQR | `delta_iqr` | Change-based IQR | `upper_boundary` (delta multiplier) |

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `BENCHER_API_KEY` | Bencher API key: user (`bencher_user_*`) or project (`bencher_run_*`) |
| `BENCHER_API_TOKEN` | User API token (JWT). Deprecated; use `BENCHER_API_KEY` |
| `BENCHER_HOST` | API host URL |
| `BENCHER_PROJECT` | Default project |
| `BENCHER_BRANCH` | Default branch |
| `BENCHER_TESTBED` | Default testbed |
| `BENCHER_ADAPTER` | Default adapter |
| `BENCHER_CMD` | Default benchmark command |

## Key & Token Management

```bash
# User API keys (bencher_user_*)
bencher user key list <user>
bencher user key create <user> --name <name> [--ttl <seconds>]
bencher user key view <user> <uuid>
bencher user key update <user> <uuid> --name <name>
bencher user key revoke <user> <uuid>

# Project API keys (bencher_run_*) - preferred for `bencher run` in CI
bencher project key list <project>
bencher project key create <project> --name <name> [--ttl <seconds>]
bencher project key view <project> <uuid>
bencher project key update <project> <uuid> --name <name>
bencher project key revoke <project> <uuid>

# User API tokens (deprecated: existing tokens only, none can be created)
bencher token list <user>
bencher token view <user> <uuid>
bencher token update <user> <uuid> --name <name>
bencher token revoke <user> <uuid>
```

The plaintext key is returned only once, at creation time. Aliases: `add` (create),
`get` (view), `edit` (update), `rm` (revoke), `ls` (list).

## Utility Commands

```bash
# Generate mock benchmark data (verify setup)
bencher mock

# Measure environment noise
bencher noise
```

## Common Resource Commands

```bash
# Projects
bencher project list [organization]
bencher project view <project>
bencher project create <organization> --name <name>

# Branches
bencher branch list <project>
bencher branch view <project> <branch>
bencher branch create <project> --name <name>

# Testbeds
bencher testbed list <project>
bencher testbed view <project> <testbed>
bencher testbed create <project> --name <name>

# Benchmarks
bencher benchmark list <project>
bencher benchmark view <project> <benchmark>

# Measures
bencher measure list <project>
bencher measure view <project> <measure>

# Reports
bencher report list <project>
bencher report view <project> <report>

# Alerts
bencher alert list <project>
bencher alert view <project> <alert>

# Thresholds
bencher threshold list <project>
bencher threshold view <project> <threshold>

# Metrics (view only, no list)
bencher metric view <project> <metric>

# Plots
bencher plot list <project>
bencher plot view <project> <plot>

# Jobs (Bencher Plus)
bencher job list <project>
bencher job view <project> <job>

# Runners (Bencher Plus)
bencher runner list
bencher runner view <runner>

# Specs (Bencher Plus)
bencher spec list
bencher spec view <spec>

# Archive/Unarchive (branches, testbeds, benchmarks, measures)
bencher archive --project <project> --branch <branch>
bencher unarchive --project <project> --branch <branch>

# Performance query (all flags are repeatable and take UUIDs only)
bencher perf <project> --branches <branch-uuid> --testbeds <testbed-uuid> --benchmarks <benchmark-uuid> --measures <measure-uuid>
```
