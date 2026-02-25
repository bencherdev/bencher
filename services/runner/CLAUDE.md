# Runner CLAUDE.md

The goal of this file is to describe the common mistakes and confusion points
an agent might face as they work in this codebase.
If you ever encounter something in the project that surprises you,
please alert the developer working with you and indicate that this is the case by editing the `CLAUDE.md` file to help prevent future agents from having the same issue.

**Bencher Console** (`services/runner`) - Bare Metal benchmark runner:
- Rust
- Clap
- Firecracker

## Design

See [services/runner/DESIGN.md](services/runner/DESIGN.md) for design documentation.

## Testing

See [services/runner/TEST.md](services/runner/TEST.md) for testing instructions and common failure patterns.
