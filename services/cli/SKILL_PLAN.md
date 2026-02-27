# Bencher Claude Code Skill Plan

Design plan for a Claude Code skill that teaches AI agents the Bencher workflow:
project setup, benchmark runs (local and bare metal), threshold configuration,
CI integration, and result interpretation.

The skill follows the [Agent Skills open standard](https://agentskills.io),
so it works across Claude Code, Cursor, GitHub Copilot, and other adopting platforms.

## File Structure

```
.claude/skills/bencher/
├── SKILL.md                  # Main skill: overview, workflow guidance, auto-trigger
├── bare-metal.md             # Bare metal workflow: build, push, bencher run --image
├── local-runs.md             # Local benchmark runs: adapters, iterations, fold
├── thresholds.md             # Threshold configuration: statistical models, boundaries
├── ci.md                     # CI integration: GitHub Actions, GitLab CI/CD, generic
└── reference.md              # Quick reference: all commands, adapters, models
```

For standalone plugin distribution (separate repo or subdirectory):
```
.claude-plugin/plugin.json    # Plugin metadata
skills/bencher/               # Same files as above
```

## File Contents

### 1. `SKILL.md` (~1,500 words)

The main entry point. YAML frontmatter:
```yaml
---
name: bencher
description: >
  Guide for using Bencher continuous benchmarking. Use when the user wants to:
  benchmark code, track performance, detect regressions, set up CI performance
  checks, run bare metal benchmarks, configure thresholds, or use the bencher CLI.
  Also use when you see BENCHER_API_TOKEN, bencher CLI usage, bencher.dev
  URLs, or port 61016 (Bencher self-hosted default) in the project.
argument-hint: [task]
---
```

Body covers:
- What Bencher is (1-2 sentences)
- Quick start: `bencher run <command>` for local, `bencher run --image` for bare metal
- Decision tree: which workflow to use
- Links to supporting files for each workflow
- Dynamic context: `` `bencher --version 2>/dev/null || echo "bencher CLI not installed"` `` to check CLI availability
- Authentication: `BENCHER_API_TOKEN` env var or `--token` flag
- Common project setup: `--project`, `--branch`, `--testbed`

### 2. `bare-metal.md`

Detailed bare metal workflow:
1. Writing a Dockerfile for benchmarks (what to include, entrypoint expectations)
2. Building the OCI image
3. Pushing to the registry
4. Submitting: `bencher run --image <ref> --spec <spec> --project <project> --branch <branch> --testbed <testbed> --adapter <adapter>`
5. Key flags: `--spec`, `--entrypoint`, `--env`, `--job-timeout`
6. How results flow back (job status, adapter parsing)
7. Example Dockerfiles for common benchmark harnesses (Rust criterion, Go bench, Python pytest-benchmark)

### 3. `local-runs.md`

Local benchmark workflow:
1. Basic: `bencher run "cargo bench"` with auto-detection (`--adapter magic`)
2. Explicit adapter selection: when to use which adapter (table of 17 adapters)
3. Multi-iteration: `--iter 5 --fold mean`
4. File-based: `--file results.json` for pre-existing output
5. Build time tracking: `--build-time`
6. File size tracking: `--file-size path/to/binary`
7. Shell vs exec mode
8. Output formats: `--format json` for programmatic use

### Adapter Table

| Adapter | Language | Tool |
|---|---|---|
| `magic` | Any | Auto-detect |
| `json` | Any | Bencher JSON format |
| `c_sharp_dot_net` | C# | DotNet BenchmarkDotNet |
| `cpp_catch2` | C++ | Catch2 |
| `cpp_google` | C++ | Google Benchmark |
| `go_bench` | Go | `go test -bench` |
| `java_jmh` | Java | JMH |
| `js_benchmark` | JavaScript | Benchmark.js |
| `js_time` | JavaScript | console.time |
| `python_asv` | Python | ASV |
| `python_pytest` | Python | pytest-benchmark |
| `ruby_benchmark` | Ruby | Benchmark module |
| `rust_bench` | Rust | `#[bench]` (nightly) |
| `rust_criterion` | Rust | Criterion.rs |
| `rust_iai` | Rust | Iai |
| `rust_gungraun` | Rust | Gungraun (Iai-Callgrind) |
| `shell_hyperfine` | Shell | Hyperfine |

### 4. `thresholds.md`

Threshold and alert configuration:
1. What thresholds do (detect regressions via statistical models)
2. Inline threshold creation with `bencher run`:
   - `--threshold-measure latency --threshold-test t --threshold-upper-boundary 0.99`
3. Statistical models: when to use each
4. Sample size and window configuration
5. `--err` flag to fail CI on alerts
6. Managing thresholds via `bencher threshold` subcommands

#### Threshold Model Table

| Model | Use Case | Key Params |
|---|---|---|
| `static` | Fixed value threshold | `lower_boundary`, `upper_boundary` |
| `percentage` | Percentage change from mean | `upper_boundary` (e.g. 0.10 = 10%) |
| `z_score` / `z` | Normal distribution, large samples | `upper_boundary` (cumulative percentage) |
| `t_test` / `t` | Normal distribution, small samples | `upper_boundary` (cumulative percentage) |
| `log_normal` | Log-normal distribution | `upper_boundary` (cumulative percentage) |
| `iqr` | Skewed data, outlier-robust | `upper_boundary` (IQR multiplier) |
| `delta_iqr` | Delta interquartile range | `upper_boundary` (ΔIQR multiplier) |

### 5. `ci.md`

CI integration guide covering three approaches:

**GitHub Actions (native integration):**
1. `--github-actions ${{ secrets.GITHUB_TOKEN }}` for automatic PR comments
2. PR comment options: `--ci-only-thresholds`, `--ci-only-on-alert`, `--ci-public-links`
3. Custom PR identifiers: `--ci-id`, `--ci-number`
4. Branch management for PRs: `--start-point main --start-point-clone-thresholds`
5. `--ci-on-the-fly` for auto-creating projects in CI

**GitLab CI/CD (manual integration via `--format html`):**
1. Setup: `BENCHER_API_TOKEN` as masked CI/CD variable
2. Target branch job: run on push to `main` with thresholds
3. Merge request job: run on `merge_request_event` using GitLab env vars:
   - `$CI_COMMIT_REF_NAME` for branch
   - `$CI_MERGE_REQUEST_TARGET_BRANCH_NAME` for start point
   - `$CI_MERGE_REQUEST_TARGET_BRANCH_SHA` for start point hash
4. Posting to MRs: `bencher run --format html` captures HTML output, then post via GitLab API:
   ```bash
   REPORT=$(bencher run --format html ...)
   curl --request POST --header "PRIVATE-TOKEN: $CI_JOB_TOKEN" \
     --data-urlencode "body=$REPORT" \
     "https://gitlab.com/api/v4/projects/$CI_PROJECT_ID/merge_requests/$CI_MERGE_REQUEST_IID/notes"
   ```
5. Branch cleanup: `bencher archive` when MR is closed

**Generic CI (any platform):**
1. Same `--format html` + manual posting pattern
2. Key: set `--branch`, `--start-point`, `--start-point-hash` from CI environment variables
3. Use `--start-point-clone-thresholds --start-point-reset` for MR/PR branches

### 6. `reference.md`

Quick-reference tables:
- All `bencher run` flags with one-line descriptions
- Adapter table (name, language, tool, format)
- Threshold model table (name, use case, key params)
- Environment variables (`BENCHER_API_TOKEN`, `BENCHER_HOST`, `BENCHER_PROJECT`, `BENCHER_BRANCH`, `BENCHER_CMD`)
- Common resource management commands (project/branch/testbed CRUD)
- Bare metal specific: spec listing, job status checking

## Key Design Decisions

1. **No `context: fork`** — the skill provides guidance, not isolated execution. The agent should run commands in the main conversation context so the user sees everything.

2. **No `disable-model-invocation`** — Claude should auto-trigger this skill when it detects benchmarking intent or Bencher usage in the project.

3. **No `allowed-tools` restriction** — the agent needs Bash (to run `bencher`, `docker`), Read/Write (for Dockerfiles, configs), etc.

4. **Dynamic context injection** — use `` `bencher --version 2>/dev/null` `` and similar to detect the current environment state when the skill loads.

5. **Progressive disclosure** — SKILL.md is the lightweight entry point (~1,500 words). Supporting files are only read when the agent needs deeper guidance on a specific workflow.

## Plugin Distribution

For external users, create a standalone plugin (can be a separate repo like `bencherdev/bencher-skill` or a published directory):

```json
// .claude-plugin/plugin.json
{
  "name": "bencher",
  "version": "0.1.0",
  "description": "Bencher continuous benchmarking skill for AI agents",
  "skills": "./skills/"
}
```

With `skills/bencher/` containing the same files. Users install via:
```
/plugin install github:bencherdev/bencher-skill
```

This can be done as a follow-up after the skill is proven in the main repo.

## Verification

1. Add the skill to `.claude/skills/bencher/SKILL.md` in the Bencher repo
2. Start a new Claude Code session in the repo
3. Test `/bencher` slash command invocation — verify the skill loads
4. Test auto-triggering by asking "help me benchmark this Rust project"
5. Verify the agent can guide through:
   - A local `bencher run "cargo bench"` workflow
   - A bare metal `bencher run --image` workflow
   - Threshold configuration
   - CI setup for GitHub Actions
   - CI setup for GitLab CI/CD with `--format html` + MR comment posting
6. Verify supporting files are referenced correctly when deeper guidance is needed
