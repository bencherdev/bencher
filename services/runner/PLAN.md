# Runner Binary Implementation Plan

This document tracks the implementation plan for aligning the `runner` binary with the updated `RUNNER_DESIGN.md`.

## Overview

The runner binary (`services/runner`) is a thin CLI wrapper around the `bencher_runner` library (`plus/bencher_runner`). The binary's role is:
1. Parse CLI arguments via `clap`
2. Convert parser types to library config types
3. Delegate execution to the library

Most of the work will be in the library, not the binary itself.

## Current State (Updated)

The runner has two subcommands:
- `daemon` - Polls API for jobs, executes them in Firecracker VMs
- `run` - One-shot execution of an OCI image

### Job Spec (OCI-based) - IMPLEMENTED

```rust
// Local struct in api_client.rs matching JsonJobSpec from bencher_json
pub struct JobSpec {
    pub registry: Url,
    pub project: Uuid,
    pub digest: String,
    pub entrypoint: Option<Vec<String>>,
    pub cmd: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub vcpu: u32,
    pub memory: u64,  // bytes
    pub disk: u64,    // bytes
    pub timeout: u32,
    pub network: bool,
}
```

**Key principle:** The runner has no default values for job resources. All job configuration (vcpu, memory, disk, timeout, network) comes from the job spec provided by the API.

## Completed Changes

### Phase 1: New Types in bencher_valid and bencher_json

- [x] Created `ImageDigest` validation type (`lib/bencher_valid/src/image_digest.rs`)
- [x] Added `ValidError::ImageDigest` variant
- [x] Created `JobUuid` and `JsonJobSpec` in `lib/bencher_json/src/project/job.rs`
- [x] Created `JobStatus` enum for job lifecycle tracking
- [x] Created `RunnerUuid` in `lib/bencher_json/src/system/runner.rs`
- [x] Exported all new types from respective lib.rs files

### Phase 2: Unit Conversion Helpers

- [x] Created `plus/bencher_runner/src/units.rs` with:
  - `bytes_to_mib()` - converts API bytes to Firecracker MiB (rounds up)
  - `mib_to_bytes()` - converts MiB back to bytes

### Phase 3: Extended Config

- [x] Added fields to `Config` struct (`plus/bencher_runner/src/config.rs`):
  - `disk_mib: u32`
  - `network: bool`
  - `entrypoint: Option<Vec<String>>`
  - `cmd: Option<Vec<String>>`
  - `env: Option<HashMap<String, String>>`
- [x] Added builder methods: `with_disk_mib()`, `with_network()`, `with_entrypoint()`, `with_entrypoint_opt()`, `with_cmd()`, `with_cmd_opt()`, `with_env()`, `with_env_opt()`

### Phase 4: Simplified DaemonConfig

- [x] Removed `default_vcpus` and `default_memory_mib` from `DaemonConfig`
- [x] All job resources now come entirely from the job spec

### Phase 5: Updated API Client

- [x] Replaced git-based `JobSpec` with OCI-based spec in `api_client.rs`
- [x] Fixed WebSocket URL path: `/ws` → `/channel`
- [x] Updated all tests for new JSON structure

### Phase 6: Updated Job Execution

- [x] Updated `build_config_from_job()` in `job.rs`:
  - No longer takes `DaemonConfig` parameter
  - All values come from job spec (no defaults)
  - Converts bytes → MiB for memory/disk
  - Builds OCI image URL: `{registry}/{project}/images@{digest}`
- [x] Added `Cancelled` message sending on job cancellation

### Phase 7: Updated WebSocket Protocol

- [x] Added `Cancelled` variant to `RunnerMessage` enum

### Phase 8: Simplified CLI Parser

- [x] Removed `--vcpus` and `--memory` flags from daemon parser
- [x] Updated `DaemonRunner` to match simplified `DaemonConfig`

## Remaining Work

### OCI Registry Authentication

**Status:** Design decision pending

The design lists three options:
1. Runner token directly authenticates to registry
2. Runner exchanges token for short-lived OCI token via API
3. Job claim response includes OCI token

**Action:** Wait for decision, then implement selected approach.

### Network Isolation Implementation

- [ ] Implement network isolation in Firecracker setup based on `network: bool`
- [ ] Default behavior when `network: false` (fully isolated VM)
- [ ] Network setup when `network: true`

### Disk Resource Implementation

- [ ] Pass `disk_mib` to Firecracker VM configuration
- [ ] Ensure rootfs is sized appropriately

## Testing Strategy

1. ✓ Unit tests for type serialization/deserialization
2. ✓ Unit tests for config building from job spec
3. ✓ Unit tests for byte/MiB conversion
4. Integration tests with mock API server (if available)
5. Manual testing with real API on Linux

## Out of Scope for Runner Binary

The runner should remain simple and stateless. These belong in the API server or other components:
- Runner registration (API handles this)
- Token management (API handles this)
- Job queueing/priority (API handles this)
- Billing/heartbeat processing (API handles this)
- Result storage (API handles this)
