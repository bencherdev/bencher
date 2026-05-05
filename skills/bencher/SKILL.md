---
name: bencher
description: >
  Guide for using Bencher continuous benchmarking. Use when the user wants to:
  benchmark code, track performance, detect regressions, set up CI performance
  checks, run bare metal benchmarks, configure thresholds, or use the bencher CLI.
  Also use when you see BENCHER_API_KEY, BENCHER_API_TOKEN, bencher CLI usage, bencher.dev
  URLs, or port 61016 (Bencher self-hosted default) in the project.
argument-hint: [task]
---

# Bencher

Bencher is a continuous benchmarking platform that tracks performance over time
and detects regressions before they reach production.

## Quick Start

Check if the CLI is installed:
```
`bencher --version 2>/dev/null || echo "bencher CLI not installed — see https://bencher.dev/docs/how-to/install-cli/"`
```

Choose your workflow:

| Goal | Workflow |
|------|---------|
| Run benchmarks locally | See [local-runs.md](./local-runs.md) |
| Run benchmarks on dedicated hardware (Bencher Cloud) | See [bare-metal.md](./bare-metal.md) |
| Set up CI regression detection | See [ci.md](./ci.md) |
| Configure statistical thresholds | See [thresholds.md](./thresholds.md) |
| Quick command reference | See [reference.md](./reference.md) |

Simplest possible run (generates mock data to verify setup):
```bash
bencher run "bencher mock"
```

## Authentication

**Preferred (agents and CI):** Project-scoped API key
```bash
export BENCHER_API_KEY=bencher_run_...
# or pass --key bencher_run_...
```

Project keys are scoped to a single project and can only submit benchmark runs
and read project data. This is the recommended authentication method.

**Alternative (users):** User-scoped API token
```bash
export BENCHER_API_TOKEN=eyJ...
# or pass --token eyJ...
```

User tokens grant full access to all projects and operations the user has permission for.

## Common Options

Every `bencher run` accepts these project-scoping options:

| Flag | Env Var | Purpose |
|------|---------|---------|
| `--project` | `BENCHER_PROJECT` | Project slug or UUID |
| `--branch` | `BENCHER_BRANCH` | Branch name/slug/UUID (auto-created if needed) |
| `--testbed` | `BENCHER_TESTBED` | Testbed name/slug/UUID (auto-created if needed) |
| `--host` | `BENCHER_HOST` | API host (default: `https://api.bencher.dev`) |

## Reading Project Data

With a project key, you can query existing data:

```bash
bencher project view <project>
bencher branch list --project <project>
bencher report list --project <project>
bencher alert list --project <project>
bencher threshold list --project <project>
```

Use `--format json` on any command for machine-readable output.
