# Runner CLAUDE.md

The goal of this file is to describe the common mistakes and confusion points
an agent might face as they work in this codebase.
If you ever encounter something in the project that surprises you,
please alert the developer working with you and indicate that this is the case by editing the `CLAUDE.md` file to help prevent future agents from having the same issue.

**Bare Metal `runner`** (`services/runner`) - Bare Metal benchmark runner:
- Rust
- Clap
- Firecracker

## Design

The runner is documented in the published docs:
- [Self-Hosted Runners](https://bencher.dev/docs/explanation/self-hosted-runners/) (explanation)
- [`runner` CLI](https://bencher.dev/docs/reference/runner/) (reference)
- [Runner Protocol](https://bencher.dev/docs/reference/runner-protocol/) (reference)

## Operations

See [`tasks/runner_ops/CLAUDE.md`](../../tasks/runner_ops/CLAUDE.md) for deploying and managing runner servers.

## Testing

See [services/runner/TEST.md](services/runner/TEST.md) for testing instructions and common failure patterns.
