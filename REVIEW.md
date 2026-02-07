# Code Review: Runner System (changes since `1dc6282b`)

## Overview

This is a massive changeset (~16,500 lines added, 97 files) implementing the **Bencher Runner System** — a bare-metal benchmark execution platform that:

1. Polls the Bencher API for jobs
2. Pulls OCI container images from a registry
3. Executes them in Firecracker microVMs with cgroup isolation
4. Reports results back via WebSocket

New crates: `bencher_runner`, `bencher_guest`, `bencher_init`, `bencher_oci`, `bencher_rootfs`, `bencher_runner_bin` (service), plus `test_runner` task and CI workflows.

## Critical Issues

### 1. COMPILE ERROR: Duplicate type imports in `bencher_json/src/lib.rs` (line 51 vs 84-88)

`JobStatus`, `JobUuid`, and `JsonJobSpec` are imported twice — once from `project::job` (line 51) and again from `runner` (lines 84-88). `RunnerUuid` is also imported twice (line 87 and 100). VS Code diagnostics confirm this produces compilation errors. The `project::job` and `runner::job` modules define entirely separate `JsonJobSpec` structs (one with `sql_type = Text`, the other with `sql_type = Integer` for `JobStatus`), which will conflict at the import site.

**Action required:** Resolve the duplicate definitions — either unify the types or use distinct names/aliases.

### 2. SECURITY: No path traversal validation in OCI layer extraction (`plus/bencher_oci/src/layer.rs:93`)

```rust
let target_path = target_dir.join(path.to_string_lossy().as_ref());
```

Tar entries from untrusted OCI images are extracted without validating that the resolved path stays within `target_dir`. A malicious image could include paths like `../../etc/passwd`. While the `tar` crate's `unpack()` has some protections, the explicit join+unpack pattern here bypasses them.

**Recommendation:** Add a canonicalization check:
```rust
if !target_path.starts_with(target_dir) {
    return Err(OciError::LayerExtraction("path traversal detected"));
}
```

### 3. SECURITY: `digest_to_blob_path` lacks validation (`plus/bencher_oci/src/image.rs:193-200`)

The `algorithm` and `hex` components from a digest string are used directly in path construction. A malicious digest like `sha256:../../../etc/passwd` would traverse out of the blobs directory.

**Recommendation:** Validate that both components only contain `[a-zA-Z0-9]` characters.

### 4. SECURITY: No SHA256 verification of downloaded binaries in `build.rs`

The build script downloads Firecracker and vmlinux kernel from GitHub/S3 without any checksum verification. A MITM or compromised mirror could inject malicious binaries.

**Recommendation:** Add known-good SHA256 hashes and verify after download.

## High Priority Issues

### 5. Inconsistent `JobStatus` storage types

- `project/job.rs:52`: `#[diesel(sql_type = diesel::sql_types::Text)]` — stores as text strings
- `runner/job.rs:123`: `#[diesel(sql_type = diesel::sql_types::Integer)]` — stores as integers

These are two different `JobStatus` enums with different DB representations. The migration at `up.sql` uses `INTEGER` for the `status` column. The `project/job.rs` version with Text storage will not work with the database schema.

### 6. Missing `#[serde(default)]` on optional fields in `runner/job.rs`

The `runner/job.rs:JsonJobSpec` uses `#[serde(skip_serializing_if = "Option::is_none")]` without `#[serde(default)]` on `entrypoint`, `cmd`, and `env`, while `project/job.rs` does include `#[serde(default, ...)]`. Without `default`, deserialization fails if the field is absent in the JSON (different from being `null`).

### 7. Install script downloads without checksum (`services/runner/templates/install-runner.sh.j2`)

The 408-line install script downloads the runner binary via curl/wget but doesn't verify checksums. Users piping this to `sh` have no integrity guarantee.

## Medium Priority Issues

### 8. File descriptor management in `bencher_init`

The init process uses raw `libc::socket`/`libc::close` without RAII wrappers. Multiple error paths require explicit `close(fd)` calls, making it easy to miss one. An fd leak in PID 1 is particularly impactful.

### 9. No input size limits on vsock data (`plus/bencher_guest/src/vsock.rs:92-99`)

`read_line()` has no size limit. A malicious host could send unlimited data, causing OOM in the guest VM.

### 10. Cgroup cleanup may silently fail (`plus/bencher_runner/src/jail/cgroup.rs:264-269`)

`remove_dir()` only works on empty cgroup directories. If processes remain, cleanup silently fails. While logged as a warning, stale cgroups can accumulate over time.

### 11. ImageDigest accepts uppercase hex (`lib/bencher_valid/src/image_digest.rs`)

OCI image digests should use lowercase hex per the spec. The current validation accepts both cases, which could cause inconsistencies when comparing digests.

### 12. OpenTelemetry meter not cached (`plus/bencher_otel/src/api_meter.rs`)

A new `Meter` is created on every call to `increment()`. This should be cached via `LazyLock` for efficiency.

## Style & Convention Issues

### 13. Redundant `test_` prefix on test functions

Multiple files use `test_` prefix on `#[test]` functions (clippy `redundant_test_prefix`):
- `plus/bencher_oci/src/registry.rs` — 7 instances
- `plus/bencher_oci/src/layer.rs:189`
- `plus/bencher_rootfs/src/squashfs.rs:179`

### 14. Lint suppression compliance — GOOD

All suppressions use `#[expect(...)]` per project convention. No `#[allow(...)]` found outside test modules. No `select!` macro usage detected.

## Positive Observations

- **Excellent RAII patterns** — `FirecrackerProcess`, `VsockListener`, `CgroupManager`, and `TuningGuard` all have proper `Drop` impls
- **Comprehensive test coverage** — 100+ unit tests across all crates, plus scenario-based integration tests
- **Strong signal handling** — Uses `AtomicBool` with `SeqCst` ordering, documented SAFETY comments
- **Good environment sanitization** — `BLOCKED_ENV_VARS` blocks dangerous `LD_*`, `MALLOC_*` vars from leaking into VMs
- **Proper digest verification** — OCI registry client validates SHA256 on blob downloads
- **Clean architecture** — Clear separation: thin CLI binary wraps the `bencher_runner` library
- **CI coverage** — Dedicated `runner.yml` workflow with KVM-enabled tests on both x86_64 and aarch64
- **Both Dockerfiles updated** for new crates per project convention
- **Well-factored test utilities** — Common helpers for test setup, database insertion, fixture generation
- **Proper feature gating** — Runner functionality correctly gated behind `#[cfg(feature = "plus")]` and `#[cfg(target_os = "linux")]`
- **Comprehensive system tuning** — ASLR, NMI watchdog, swappiness, CPU governor, perf_event_paranoid, SMT, turbo boost
- **Secure install script** — Proper quoting, `set -u`, `mktemp -d`, HTTPS enforcement, architecture validation

## Summary

| Category | Count | Severity |
|----------|-------|----------|
| Compile errors | 1 | Critical |
| Security (path traversal) | 2 | Critical |
| Security (supply chain) | 1 | Critical |
| Type inconsistencies | 2 | High |
| Resource safety | 3 | Medium |
| Performance | 1 | Medium |
| Style | 2 | Low |

The architecture and implementation quality is high — strong RAII, comprehensive tests, good concurrency patterns. The critical items are the duplicate type imports (which break compilation) and the path traversal vulnerabilities in OCI image handling.