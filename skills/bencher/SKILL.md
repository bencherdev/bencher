---
name: bencher
description: >
  Guide for using Bencher continuous benchmarking. Use when the user wants to:
  benchmark code, track performance, detect regressions, set up CI performance
  checks, run bare metal benchmarks, configure thresholds, or use the bencher CLI.
  Also use when you see BENCHER_API_KEY, BENCHER_API_TOKEN, bencher_run_ or
  bencher_user_ API keys, bencher CLI usage, bencher.dev URLs, or port 6610
  (Bencher self-hosted default) in the project.
argument-hint: [task]
---

# Bencher

Bencher is a continuous benchmarking platform that tracks performance over time
and detects regressions before they reach production.

## Quick Start

Check if the CLI is installed:
```bash
bencher --version 2>/dev/null || echo "bencher CLI not installed. See https://bencher.dev/docs/how-to/install-cli/"
```

Choose your workflow:

| Goal | Workflow |
|------|---------|
| Run benchmarks locally | See [local-runs.md](./local-runs.md) |
| Run benchmarks on dedicated hardware (Bencher Cloud) | See [bare-metal.md](./bare-metal.md) |
| Set up CI regression detection | See [ci.md](./ci.md) |
| Configure statistical thresholds | See [thresholds.md](./thresholds.md) |
| Set up a self-hosted Bencher instance | See [self-hosted.md](./self-hosted.md) |
| Quick command reference | See [reference.md](./reference.md) |

Simplest possible run (generates mock data to verify setup):
```bash
bencher run "bencher mock"
```

## Authentication

Bencher authenticates with an **API key** via `--key` or `BENCHER_API_KEY`.
The prefix selects one of two kinds.

**Project key (`bencher_run_*`), preferred for CI:** scoped to a single existing
project. Submits runs and reads that project's data; cannot create or claim a project,
nor perform manage-level operations such as renaming resources or managing keys (least privilege).
```bash
export BENCHER_API_KEY=bencher_run_...
bencher run --project my-project "cargo bench"
```
The project must be identified by `--project` or derived from a `--image` whose
repository names the project. A project key cannot create a project on the fly.

**User key (`bencher_user_*`), good for local/interactive use:** authenticates as the
owning user across everything a session can do (except minting other keys), and can
create a project on the fly, so `--project` is optional.
```bash
export BENCHER_API_KEY=bencher_user_...
bencher run --project my-project "cargo bench"
```

**Deprecated, user API token (`--token` / `BENCHER_API_TOKEN`, a JWT):** existing
tokens still work, but new ones can no longer be created. Use an API key instead.

`--key` and `--token` are mutually exclusive; `--key` takes precedence.

## Common Options

Every `bencher run` accepts these project-scoping options:

| Flag | Env Var | Purpose |
|------|---------|---------|
| `--project` | `BENCHER_PROJECT` | Project slug or UUID |
| `--branch` | `BENCHER_BRANCH` | Branch name/slug/UUID (auto-created if needed) |
| `--testbed` | `BENCHER_TESTBED` | Testbed name/slug/UUID (auto-created if needed) |
| `--host` | `BENCHER_HOST` | API host (default: `https://api.bencher.dev`) |

## Reading Project Data

A project key reads its own project; a user key spans all the user's projects.
Query existing data:

```bash
bencher project view <project>
bencher branch list --project <project>
bencher report list --project <project>
bencher alert list --project <project>
bencher threshold list --project <project>
```

Use `--format json` on any command for machine-readable output.
