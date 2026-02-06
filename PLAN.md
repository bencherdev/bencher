# Bencher Bare Metal Runner Architecture Plan

## Executive Summary

This document outlines the architecture for adding bare metal runner support to Bencher. The design supports both Bencher Cloud and Self-Hosted deployments, with a path toward bring-your-own-runner (BYOR) support. The solution uses **Firecracker**, AWS's open-source microVM manager, providing hardware-level security isolation with minimal performance overhead (<5%), battle-tested multi-tenant security, and a clean REST API for VM lifecycle management.

### Why Firecracker?

Firecracker is a purpose-built microVM manager created by AWS for Lambda and Fargate. It provides exactly the isolation properties Bencher needs, with years of production hardening and security audits at massive scale.

| Aspect | Firecracker | Alternatives |
|--------|-------------|--------------|
| **Security** | Years of security audits at AWS scale | Custom VMMs have unproven security posture |
| **Overhead** | <5% CPU, ~5 MiB memory per VM | Equivalent to any KVM-based solution |
| **Boot time** | ~125ms | Comparable to other microVMs |
| **Maturity** | Production-proven at massive scale | Custom solutions require extensive hardening |
| **Maintenance** | Active open-source community + AWS backing | Custom VMMs require in-house security expertise |
| **Jailer** | Built-in jailer with seccomp, cgroups, namespaces | Must be implemented from scratch |
| **vsock** | Native virtio-vsock support | Same |

Firecracker is managed as an external process per VM, controlled via a REST API over a Unix domain socket. The Bencher runner daemon manages Firecracker processes and communicates with guests via vsock for result collection.

---

## Table of Contents

1. [Goals and Requirements](#goals-and-requirements)
2. [Architecture Overview](#architecture-overview)
3. [Isolation Strategy](#isolation-strategy)
4. [Firecracker Integration](#firecracker-integration)
5. [Migration: bencher_vmm to Firecracker](#step-by-step-migration-from-bencher_vmm-to-firecracker)
6. [Production Hardening Checklist](#production-hardening-checklist)
7. [OCI Image Handling](#oci-image-handling)
8. [Runner Daemon Design](#runner-daemon-design)
9. [Job Scheduling and Queue](#job-scheduling-and-queue)
10. [API and CLI Changes](#api-and-cli-changes)
11. [Security Considerations](#security-considerations)
12. [Performance Considerations](#performance-considerations)
13. [Networking](#networking)
14. [Storage and Artifacts](#storage-and-artifacts)
15. [Observability](#observability)
16. [Implementation Phases](#implementation-phases)
17. [Open Questions](#open-questions)

---

## Goals and Requirements

### Primary Goals

1. **Multi-tenant isolation**: Multiple users can run benchmarks on shared hardware without interference or security risks
2. **Bare metal performance**: Achieve >95% of native bare metal performance for CPU-bound workloads
3. **No vendor lock-in**: Solution must be deployable on any bare metal server, not locked to AWS/GCP/Azure proprietary offerings
4. **Support Cloud and Self-Hosted**: Architecture works identically for Bencher Cloud and self-hosted deployments
5. **OCI-based workflow**: Users package benchmarks as standard OCI images, enabling reproducibility and portability

### Secondary Goals

1. **BYOR support**: Path toward users bringing their own runners to Bencher Cloud
2. **Resource efficiency**: Maximize runner utilization while maintaining benchmark accuracy
3. **Reproducibility**: Same image produces consistent results across runs

### Non-Goals (for initial implementation)

1. GPU/accelerator support (future consideration)
2. Windows runner support (Linux-only initially)
3. macOS runner support (no KVM on macOS)

---

## Architecture Overview

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Bencher CLI    │     │  Bencher API    │     │  Runner Agent   │
│  (submits jobs) │────▶│  (job queue)    │◀────│  (polls/claims) │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                               │                        │
                               ▼                        ▼
                        ┌─────────────┐          ┌─────────────┐
                        │   SQLite    │          │  Bare Metal │
                        │  (jobs tbl) │          │   Machine   │
                        └─────────────┘          └─────────────┘
```

Runners are **server-scoped** — they can execute jobs from ANY project on the server. This applies to both self-hosted and cloud deployments:

- **Self-hosted**: Runners serve all projects on that Bencher instance
- **Cloud**: Bencher-provided runners serve all organizations/projects on Bencher Cloud

See `RUNNER_DESIGN.md` for the full cloud-side API design, including database schema, API endpoints, WebSocket protocol, authentication, billing, and job recovery.

### Detailed Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Bencher API Server                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌──────────────┐   │
│  │   Job API   │  │ Runner API  │  │  Image API  │  │  Results API │   │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬───────┘   │
│         │                │                │                │           │
│  ┌──────┴────────────────┴────────────────┴────────────────┴───────┐   │
│  │                        Job Queue (SQLite)                        │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│         │                                                              │
│  ┌──────┴──────────────────────────────────────────────────────────┐   │
│  │                    Image Registry (OCI Distribution)              │   │
│  └──────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    │               │               │
                    ▼               ▼               ▼
            ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
            │   Runner 1   │ │   Runner 2   │ │   Runner N   │
            │  (Bare Metal)│ │  (Bare Metal)│ │   (BYOR)     │
            │              │ │              │ │              │
            │ ┌──────────┐ │ │ ┌──────────┐ │ │ ┌──────────┐ │
            │ │  Runner  │ │ │ │  Runner  │ │ │ │  Runner  │ │
            │ │  Daemon  │ │ │ │  Daemon  │ │ │ │  Daemon  │ │
            │ └────┬─────┘ │ │ └────┬─────┘ │ │ └────┬─────┘ │
            │      │       │ │      │       │ │      │       │
            │ ┌────┴─────┐ │ │ ┌────┴─────┐ │ │ ┌────┴─────┐ │
            │ │Firecrac- │ │ │ │Firecrac- │ │ │ │Firecrac- │ │
            │ │  ker VM  │ │ │ │  ker VM  │ │ │ │  ker VM  │ │
            │ └──────────┘ │ │ └──────────┘ │ │ └──────────┘ │
            └──────────────┘ └──────────────┘ └──────────────┘
```

### Component Summary

| Component              | Description                                                                        |
| ---------------------- | ---------------------------------------------------------------------------------- |
| **Bencher API Server** | Existing API server, extended with Job, Runner, and Image APIs                     |
| **Job Queue**          | Persistent queue for benchmark jobs (SQLite, linked to reports)                    |
| **Image Registry**     | OCI-compliant registry for storing benchmark images                                |
| **Runner Daemon**      | Long-running process on bare metal servers that claims jobs and executes them      |
| **Firecracker**        | AWS open-source microVM manager providing hardware-level isolation via KVM         |

---

## Isolation Strategy

All benchmark jobs run inside isolated Firecracker microVMs. This provides hardware-level isolation suitable for multi-tenant environments, backed by years of production hardening at AWS.

### Firecracker Characteristics

| Criteria           | Firecracker                                          |
| ------------------ | ---------------------------------------------------- |
| CPU Overhead       | <5% (>95% of bare metal)                             |
| Boot Time          | ~125ms                                               |
| Memory Overhead    | ~5 MiB per VM                                        |
| Security Isolation | Hardware-level via KVM + jailer (seccomp, cgroups, namespaces) |
| Architecture       | x86_64 and aarch64 supported                         |
| Vendor Lock-in     | None (open source, runs on any KVM host)             |
| Deployment         | Single static binary + kernel image                  |

### Why Firecracker over Alternatives?

| Alternative          | Why Not                                                                              |
| -------------------- | ------------------------------------------------------------------------------------ |
| **Plain Containers** | Shared kernel is insufficient security boundary for untrusted multi-tenant workloads |
| **Custom VMM**       | Unproven security posture; VMMs are hard to get right and require dedicated security audits |
| **Kata Containers**  | ~17% CPU overhead, 130 MiB memory per pod - too heavy                                |
| **gVisor**           | 10x syscall overhead, unsuitable for syscall-heavy benchmarks                        |
| **Cloud Hypervisor** | Less mature, smaller community, fewer security audits                                |
| **QEMU/KVM**         | Higher overhead, slower boot, larger attack surface                                  |

### Configuration

```toml
# runner.toml
[firecracker]
binary_path = "/usr/local/bin/firecracker"
jailer_path = "/usr/local/bin/jailer"
kernel_path = "/var/lib/bencher/vmlinux"
vcpus = 4
memory_mib = 4096
timeout_secs = 300
```

---

## Firecracker Integration

The runner daemon manages Firecracker processes to provide hardware-isolated VM execution for each benchmark job.

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         Host                                │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                  Runner Daemon                          ││
│  │  ┌──────────────────────────────────────────────────┐   ││
│  │  │          Firecracker Manager                     │   ││
│  │  │  ┌─────────────┐  ┌───────────┐  ┌────────────┐ │   ││
│  │  │  │ API Client  │  │  Process  │  │   Vsock    │ │   ││
│  │  │  │ (Unix sock) │  │  Manager  │  │  Listener  │ │   ││
│  │  │  └─────────────┘  └───────────┘  └────────────┘ │   ││
│  │  └──────────────────────────────────────────────────┘   ││
│  └─────────────────────────────────────────────────────────┘│
│                              │                              │
│  ┌───────────────────────────┴───────────────────────────┐  │
│  │              Firecracker (jailer)                      │  │
│  │  ┌─────────────┐  ┌─────────────┐                     │  │
│  │  │  microVM    │  │  Devices:   │                     │  │
│  │  │  (KVM)      │  │  virtio-blk │                     │  │
│  │  │             │  │  virtio-vsock│                     │  │
│  │  └─────────────┘  └─────────────┘                     │  │
│  │                                                       │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │                  Guest VM                       │  │  │
│  │  │  ┌──────────┐  ┌──────────┐  ┌──────────────┐  │  │  │
│  │  │  │  Kernel  │  │  ext4    │  │  Benchmark   │  │  │  │
│  │  │  │ (Linux)  │  │  rootfs  │  │   Process    │  │  │  │
│  │  │  └──────────┘  └──────────┘  └──────────────┘  │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Firecracker VM Lifecycle

The runner daemon manages each Firecracker VM through its REST API:

```rust
use crate::firecracker::{FirecrackerProcess, FirecrackerConfig, MachineConfig, BootSource, Drive, Vsock, Action};

// 1. Start Firecracker process (via jailer for production)
let fc = FirecrackerProcess::start(&FirecrackerConfig {
    binary: "/usr/local/bin/firecracker".into(),
    jailer: Some("/usr/local/bin/jailer".into()),
    socket_path: work_dir.join("firecracker.sock"),
    id: job_id.to_string(),
})?;

// 2. Configure the VM via REST API
fc.put_machine_config(&MachineConfig {
    vcpu_count: 4,
    mem_size_mib: 4096,
    smt: false, // Disable SMT for consistent benchmarks
})?;

fc.put_boot_source(&BootSource {
    kernel_image_path: "/var/lib/bencher/vmlinux".into(),
    boot_args: "console=ttyS0 reboot=k panic=1 pci=off rw".into(),
})?;

fc.put_drive(&Drive {
    drive_id: "rootfs".into(),
    path_on_host: rootfs_path.into(),
    is_root_device: true,
    is_read_only: false,
})?;

fc.put_vsock(&Vsock {
    guest_cid: 3,
    uds_path: vsock_path.into(),
})?;

// 3. Boot the VM
fc.put_action(&Action::InstanceStart)?;

// 4. Collect results via vsock
let results = collect_results_via_vsock(&vsock_path)?;

// 5. Kill the process when done
fc.kill()?;
```

### Result Collection

Results are collected via virtio-vsock on dedicated ports:

| Port | Purpose     | Content                            |
| ---- | ----------- | ---------------------------------- |
| 5000 | stdout      | Standard output from the benchmark |
| 5001 | stderr      | Standard error from the benchmark  |
| 5002 | exit_code   | Exit code as a string (e.g., "0")  |
| 5005 | output_file | Optional output file contents      |

The guest init binary (bencher-init) buffers output and sends it via vsock after the benchmark completes.

### Security Model

Firecracker provides defense-in-depth through multiple layers:

1. **Jailer**: Runs Firecracker in a restricted environment with:
   - New PID, network, mount, and user namespaces
   - `chroot` to an isolated directory
   - Seccomp filters (Firecracker's built-in allowlist)
   - `cgroups` for resource limits
   - All capabilities dropped
2. **Minimal device model**: Only virtio-blk, virtio-vsock, serial — no USB, no PCI, no GPU
3. **No network**: No virtio-net device configured; vsock is the only communication channel
4. **Memory isolation**: Fixed allocation, no ballooning, cannot exceed limit
5. **Battle-tested**: Firecracker's security has been audited and hardened for AWS Lambda/Fargate workloads

---

## Required Changes to Existing Code

Before proceeding with the implementation phases, the following changes are needed to migrate from the existing `bencher_vmm` custom VMM to Firecracker:

### 1. Replace bencher_vmm with Firecracker Client

**Status**: Needed

The custom `bencher_vmm` crate (`plus/bencher_vmm`) will be replaced with a Firecracker API client module in the runner daemon. The runner will manage Firecracker as an external process via its REST API over a Unix domain socket.

**Changes needed**:
- Remove `plus/bencher_vmm` crate (replaced by Firecracker binary)
- Add `plus/bencher_runner/src/firecracker/` module with:
  - `client.rs` — HTTP client for Firecracker's REST API (Unix socket)
  - `process.rs` — Firecracker/jailer process lifecycle management
  - `config.rs` — VM configuration types matching Firecracker's API
- Update `plus/bencher_runner/src/run.rs` to use Firecracker client instead of `bencher_vmm`

### 2. Writable Rootfs ✅ COMPLETE

**Solution implemented**: Changed rootfs from squashfs to ext4 using `mkfs.ext4 -d` option.

This work carries forward — Firecracker supports ext4 rootfs via its virtio-blk device with `is_read_only: false`.

**Files modified**:
- `plus/bencher_rootfs/src/ext4.rs` - ext4 image creation
- `plus/bencher_rootfs/src/lib.rs` - Export ext4 functions
- `plus/bencher_rootfs/src/error.rs` - Added Ext4 error variant
- `plus/bencher_runner/src/run.rs` - Uses ext4 instead of squashfs

### 3. Guest Init System (bencher-init) — No Changes Needed

The `bencher-init` binary at `plus/bencher_init/` works identically under Firecracker as it did under `bencher_vmm`. It communicates via virtio-vsock (CID-based addressing), which Firecracker supports natively. No changes required.

### Step-by-Step Migration from bencher_vmm to Firecracker

The current execution model is a two-phase process: the `run` subcommand prepares the rootfs and jail, then `exec()`s into the `vmm` subcommand which sets up namespaces, mounts, pivot_root, and calls `bencher_vmm::run_vm()`. Firecracker replaces the second phase — its jailer handles namespace/mount/pivot_root isolation, and the Firecracker process handles KVM, devices, and the event loop.

#### What stays the same

- `plus/bencher_init/` — Guest init binary. Unchanged. Communicates via vsock (CID 2, ports 5000-5005).
- `plus/bencher_rootfs/` — ext4 rootfs creation. Unchanged. Firecracker's `virtio-blk` with `is_read_only: false` supports the same ext4 images.
- `plus/bencher_oci/` — OCI image pulling and unpacking. Unchanged.
- `plus/bencher_runner/src/run.rs` — Phase 1 orchestration (OCI pull, rootfs creation, init binary injection, config.json writing). Mostly unchanged; the exec-to-vmm-subcommand call at the end is replaced with a Firecracker launch.
- Vsock result collection protocol (ports 5000-5005, same data format).

#### Step 1: Add Firecracker binary management

Create `plus/bencher_runner/src/firecracker/process.rs`.

This module manages the Firecracker process lifecycle:
- Start the `firecracker` binary (or `jailer` wrapping `firecracker`) as a child process
- The process listens on a Unix domain socket for its REST API
- Wait for the socket to become available (poll with short backoff)
- Provide `kill()` and `kill_after_grace_period()` for cleanup
- In production, use the jailer: `jailer --id {job_id} --exec-file /usr/local/bin/firecracker --uid {uid} --gid {gid} -- --api-sock /run/firecracker.sock`
- In development, run Firecracker directly without the jailer

The jailer replaces the manual namespace/mount/pivot_root setup currently in `plus/bencher_runner/src/vmm.rs`.

#### Step 2: Add Firecracker REST API client

Create `plus/bencher_runner/src/firecracker/client.rs`.

HTTP client that speaks to Firecracker's API over the Unix domain socket. Needs to support these endpoints:

| Method | Path | Purpose |
|--------|------|---------|
| `PUT` | `/machine-config` | Set vCPU count, memory, SMT |
| `PUT` | `/boot-source` | Set kernel path and boot args |
| `PUT` | `/drives/{id}` | Attach rootfs block device |
| `PUT` | `/vsock` | Configure vsock (guest CID + UDS path) |
| `PUT` | `/actions` | `InstanceStart`, `SendCtrlAltDel` |
| `GET` | `/` | Health check / wait for ready |

Use `hyper` with a Unix socket connector (already in the dependency tree via `reqwest`), or a minimal hand-rolled HTTP/1.1 client since the Firecracker API is simple PUT/GET with JSON bodies. No external SDK crate is needed.

#### Step 3: Add Firecracker configuration types

Create `plus/bencher_runner/src/firecracker/config.rs`.

Define Rust structs matching Firecracker's API request/response schemas:

```rust
#[derive(Serialize)]
pub struct MachineConfig {
    pub vcpu_count: u8,
    pub mem_size_mib: u32,
    pub smt: bool,
}

#[derive(Serialize)]
pub struct BootSource {
    pub kernel_image_path: String,
    pub boot_args: String,
}

#[derive(Serialize)]
pub struct Drive {
    pub drive_id: String,
    pub path_on_host: String,
    pub is_root_device: bool,
    pub is_read_only: bool,
}

#[derive(Serialize)]
pub struct Vsock {
    pub guest_cid: u32,
    pub uds_path: String,
}

#[derive(Serialize)]
pub struct Action {
    pub action_type: ActionType,
}

#[derive(Serialize)]
pub enum ActionType {
    InstanceStart,
    SendCtrlAltDel,
}
```

#### Step 4: Add vsock result listener

Create `plus/bencher_runner/src/firecracker/vsock.rs`.

Firecracker exposes guest vsock connections as connections on a host-side Unix domain socket. When the guest connects to CID 2 on port 5000, Firecracker forwards the connection to the UDS path specified in the vsock config. The host-side listener must:

1. Accept connections on the vsock UDS
2. Read the port number from Firecracker's vsock handshake (the first line is `CONNECT {port}\n` or similar, depending on the Firecracker vsock implementation — consult Firecracker docs for the exact protocol)
3. Route data to the appropriate buffer based on port (5000=stdout, 5001=stderr, 5002=exit_code, 5005=output_file)
4. Enforce a maximum buffer size (10 MiB) per port to prevent memory exhaustion
5. Return collected results once all expected ports have reported or the VM exits

This replaces the vsock handling that was previously internal to `bencher_vmm::event_loop`.

#### Step 5: Replace vmm.rs with Firecracker orchestration

Rewrite `plus/bencher_runner/src/vmm.rs` (~242 LOC).

The current `vmm.rs` does:
1. Create namespaces (user, mount, network, UTS, IPC, PID)
2. Fork into PID namespace
3. Mount procfs, devtmpfs, tmpfs
4. Bind-mount `/dev/kvm`
5. `pivot_root` to jail directory
6. Apply rlimits, `PR_SET_NO_NEW_PRIVS`, drop capabilities
7. Construct `VmConfig` and call `bencher_vmm::run_vm()`
8. Print results to stdout

Steps 1-6 are replaced by the Firecracker jailer. The new `vmm.rs` becomes:

1. Copy the rootfs image for this job (each job gets its own writable copy)
2. Start Firecracker via jailer (Step 1)
3. Configure VM via REST API (Steps 2-3): machine config, boot source, rootfs drive, vsock
4. Start vsock listener (Step 4)
5. Boot the VM (`InstanceStart`)
6. Wait for results from vsock listener, with timeout
7. On timeout or completion: send `SendCtrlAltDel`, wait grace period, then kill process
8. Print results to stdout (same format as before)

#### Step 6: Update run.rs to remove exec-to-vmm pattern

Currently `plus/bencher_runner/src/run.rs` prepares everything and then calls `Command::new(current_exe).arg("vmm").exec()` which replaces the process. With Firecracker, the runner can manage the Firecracker process as a child instead of replacing itself. This simplifies error handling and allows the runner to monitor the Firecracker process.

Change the end of `run.rs` from:
```rust
// Old: exec() into vmm subcommand (never returns)
Command::new(std::env::current_exe()?).arg("vmm").args(&vmm_args).exec();
```
To:
```rust
// New: launch Firecracker as a child process
let results = firecracker::run_job(&firecracker_config, &job_config).await?;
println!("{}", results.output);
```

This also means the `vmm` subcommand in the runner binary (`services/runner/src/runner/vmm.rs`) can be removed — the runner no longer needs to re-exec itself.

#### Step 7: Update kernel management

The current `bencher_vmm` build script (`plus/bencher_vmm/build.rs`) downloads a Linux kernel at compile time and embeds it in the binary via `include_bytes!`. With Firecracker:

- The kernel is a separate file on disk, not embedded in the runner binary
- Use the same kernel that Firecracker CI publishes (the current build script already downloads from Firecracker's CI artifacts)
- Move the kernel download logic from `bencher_vmm/build.rs` to a setup step in the runner daemon, or distribute the kernel alongside the Firecracker binary
- Update `runner.toml` to require `kernel_path` configuration

#### Step 8: Remove HMAC verification

The current HMAC-SHA256 "integrity verification" (vsock port 5003, nonce in config.json) provides no security benefit: the guest has the key and can compute a valid HMAC over arbitrary data. Remove:

- `verify_hmac()` from the runner's result handling
- `nonce` field from the init config.json schema
- HMAC computation from `plus/bencher_init/src/init.rs`
- Vsock port 5003 (HMAC port)
- `---BENCHER_HMAC:{hex}---` serial markers

This simplifies both the guest init and the host-side result collection.

#### Step 9: Remove bencher_vmm crate

Once steps 1-8 are complete and tests pass:

1. Delete `plus/bencher_vmm/` entirely (~5,800 LOC)
2. Remove `bencher_vmm` from the workspace `Cargo.toml`
3. Remove `bencher_vmm` dependency from `plus/bencher_runner/Cargo.toml`
4. Remove `bencher_vmm` feature flags from `plus/bencher_runner/Cargo.toml`
5. Delete `plus/bencher_runner/src/jail/` — namespace, chroot, and rlimit setup is now handled by the jailer. Keep `cgroup.rs` only if the jailer's built-in cgroup support is insufficient for I/O throttling (check Firecracker jailer docs).

#### Step 10: Update tests

- **Unit tests**: Add tests for the Firecracker API client (mock the Unix socket)
- **Integration tests**: Update `tasks/test_runner/` to test Firecracker-based execution. These tests require a Linux host with KVM and the Firecracker binary installed.
- **Remove**: All `bencher_vmm` unit tests (they test KVM internals that are now Firecracker's responsibility)
- **Keep**: `bencher_init` tests, `bencher_rootfs` tests, OCI tests — all unchanged

#### Summary of LOC changes (estimated)

| Action | LOC |
|--------|-----|
| Delete `plus/bencher_vmm/` | -5,800 |
| Delete `plus/bencher_runner/src/vmm.rs` (old) | -242 |
| Delete `plus/bencher_runner/src/jail/` (most of it) | -400 |
| Add `plus/bencher_runner/src/firecracker/` | +600 |
| Modify `plus/bencher_runner/src/run.rs` | ~100 changed |
| Modify `plus/bencher_init/src/init.rs` (remove HMAC) | -80 |
| **Net change** | ~-5,800 |

---

## Production Hardening Checklist

Before the runner is ready to handle arbitrary benchmark workloads from untrusted users, the following items must be addressed. Switching to Firecracker eliminates several categories of issues (seccomp, capability management, VMM-level race conditions) since Firecracker and its jailer handle these.

### Runner-Level Issues

- [x] **1. Disk I/O Limits** (`plus/bencher_runner/src/jail/cgroup.rs`) ✅
  - Workload can perform excessive disk I/O, affecting other processes on the host
  - **Fixed**: Added `io_read_bps`/`io_write_bps` to `ResourceLimits`, enabled io controller

- [x] **2. Environment Variable Sanitization** (`plus/bencher_runner/src/run.rs`) ✅
  - `LD_PRELOAD`, `LD_LIBRARY_PATH`, `PATH` passed directly from OCI image
  - **Fixed**: Added allowlist-based `sanitize_env()` function (only explicitly permitted variables are passed through)

- [x] **3. Cgroup Controller Failures** (`plus/bencher_runner/src/jail/cgroup.rs`) ✅
  - `enable_controllers()` silently ignores failures
  - **Fixed**: Validate required controllers (cpu, memory, pids) are enabled after write; graceful fallback when io controller unavailable

- [x] **4. OOM Killer Protection** (`plus/bencher_runner/src/jail/cgroup.rs`) ✅
  - **Fixed**: Set `memory.oom.group=1` for group kill on OOM, disabled swap with `memory.swap.max=0`

### Guest Init Issues

- [x] **5. Vsock Reliability** (`plus/bencher_init/src/init.rs`) ✅
  - Blocking connect with no timeout - can hang indefinitely
  - **Fixed**: Added `SO_SNDTIMEO` (5s) on vsock sockets, retry logic (3 attempts with backoff), EINTR handling in write loop

- [ ] **6. Output Buffer Limits**
  - The runner must cap the amount of data it reads from vsock to prevent a malicious guest from exhausting host memory
  - Set a maximum size (e.g., 10 MiB) when reading from vsock ports

### Addressed by Firecracker

The following issues from the previous custom VMM approach are now handled by Firecracker and its jailer:

- **Seccomp filters**: Firecracker ships its own seccomp profile, maintained by AWS
- **Capability dropping**: The jailer drops all capabilities
- **PID/mount/network namespaces**: The jailer creates all necessary namespaces
- **Timeout enforcement**: Runner sends `SendCtrlAltDel` action via API, then kills the Firecracker process after a grace period
- **Serial output race conditions**: Not applicable — Firecracker handles serial internally

### Deferred

- [ ] **7. Telemetry/Metrics** — Feature addition, not a security fix
- [ ] **8. Spectre/Meltdown/MDS Mitigations** — Host kernel must be configured with appropriate mitigations (`l1tf=flush`, separate physical cores per tenant). Document required host kernel parameters.

---

## OCI Image Handling

### Image Workflow

```
┌─────────────┐      ┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│   User      │      │  Bencher    │      │   Image     │      │   Runner    │
│   builds    │ ───▶ │   CLI       │ ───▶ │  Registry   │ ───▶ │   pulls     │
│   image     │ push │   pushes    │      │   stores    │ pull │   image     │
└─────────────┘      └─────────────┘      └─────────────┘      └─────────────┘
```

### Image Requirements

Users must build OCI images that conform to the Bencher Runner Image Specification:

```dockerfile
# Example Bencher benchmark image
FROM bencher/runner-base:latest

# Install benchmark dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    hyperfine

# Copy benchmark code
COPY ./benchmarks /benchmarks
COPY ./src /src

# Build the project
RUN cd /src && cargo build --release

# Define the benchmark entrypoint
# Must output results to stdout in a supported format (JSON, Bencher JSON, etc.)
ENTRYPOINT ["/benchmarks/run.sh"]
```

### Image Specification

```yaml
# bencher-image.yaml (embedded in image labels)
apiVersion: bencher.dev/v1
kind: BenchmarkImage
metadata:
  name: my-benchmark
  version: 1.0.0
spec:
  # Command to run (overrides ENTRYPOINT if specified)
  command: ["/benchmarks/run.sh"]

  # Output format (for parsing results)
  outputFormat: "json"  # json | bencher | criterion | hyperfine | custom

  # Resource requirements
  resources:
    vcpus: 4
    memory: "4Gi"

  # Timeout for the benchmark run
  timeout: "30m"

  # Environment variables
  env:
    - name: RUST_BACKTRACE
      value: "1"
```

### Image Registry

The Bencher API server embeds an OCI Distribution-compliant registry:

```
POST /v2/{project}/blobs/uploads/     # Start blob upload
PUT  /v2/{project}/blobs/uploads/{id} # Complete blob upload
PUT  /v2/{project}/manifests/{tag}    # Push manifest
GET  /v2/{project}/manifests/{tag}    # Pull manifest
GET  /v2/{project}/blobs/{digest}     # Pull blob
```

#### Storage Backend Options

| Backend          | Use Case                   |
| ---------------- | -------------------------- |
| Local filesystem | Self-hosted, single-node   |
| S3/R2/GCS        | Bencher Cloud, distributed |
| SQLite BLOB      | Self-hosted, embedded      |

### Image-to-Rootfs Conversion

For Firecracker, OCI images must be converted to a bootable rootfs:

```
OCI Image ──▶ Unpack layers ──▶ Install bencher-init ──▶ Write config.json ──▶ Create rootfs image
```

**Rootfs format**: ext4 (writable, standard, supported by Firecracker's virtio-blk with `is_read_only: false`).

The runner daemon handles this conversion (in `plus/bencher_runner/src/run.rs`):

```rust
// Simplified flow from actual implementation
async fn convert_oci_to_rootfs(image_ref: &str, config: &BenchmarkConfig) -> Result<Utf8PathBuf> {
    // 1. Pull and unpack OCI image layers
    let unpacked_dir = unpack_oci_image(image_ref).await?;

    // 2. Install bencher-init binary at /init
    install_init_binary(&unpacked_dir)?;

    // 3. Write config.json with command, workdir, env, output_file
    write_init_config(&unpacked_dir, &config)?;

    // 4. Create ext4 rootfs image
    let rootfs_path = create_ext4_rootfs(&unpacked_dir)?;

    Ok(rootfs_path)
}
```

### Image Caching

To avoid repeated conversion overhead:

```
~/.bencher/cache/
├── images/
│   └── sha256-{digest}/
│       ├── rootfs.ext4      # Converted rootfs (writable)
│       ├── config.json      # OCI config
│       └── metadata.json    # Cache metadata
└── layers/
    └── sha256-{digest}.tar  # Cached layers
```

> **Note**: Since ext4 images are writable, each job gets a copy-on-write clone or a fresh copy of the cached rootfs to ensure isolation between runs.

Cache eviction policy:
- LRU with configurable max size (default: 50GB)
- Images not used in 7 days are eligible for eviction
- Manual cache clear via `bencher runner cache clear`

---

## Runner Daemon Design

### Overview

The **Runner Daemon** (`bencher-runner`) is a long-running process that:
1. Connects to the Bencher API server using a pre-registered runner token
2. Long-polls to claim jobs from the server-scoped job queue
3. Executes benchmarks in isolated Firecracker microVMs
4. Reports results back via WebSocket channel

Runners are **server-scoped** — they can execute jobs from ANY project on the server. This applies to both self-hosted and Bencher Cloud deployments. See `RUNNER_DESIGN.md` for the full cloud-side API design.

### Runner States

| State      | Network Behavior                       | Notes                                |
| ---------- | -------------------------------------- | ------------------------------------ |
| **Idle**   | Long-poll for jobs                     | Can be noisy, responsiveness matters |
| **Active** | Minimal heartbeat on separate CPU core | Benchmark cores completely isolated  |

### State Machine

```
                    ┌─────────────────┐
                    │   STARTING      │
                    │  (Initialize)   │
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
         ┌─────────│     IDLE        │◀────────┐
         │         │ (Long-poll for  │         │
         │         │  jobs to claim) │         │
         │         └────────┬────────┘         │
         │                  │                  │
         │ shutdown         │ job claimed      │ job complete
         │                  ▼                  │
         │         ┌─────────────────┐         │
         │         │   PREPARING     │         │
         │         │ (Clone repo,    │         │
         │         │  run setup_cmd) │         │
         │         └────────┬────────┘         │
         │                  │                  │
         │                  ▼                  │
         │         ┌─────────────────┐         │
         │         │    RUNNING      │─────────┘
         │         │ (Execute in VM, │
         │         │  WS heartbeat)  │
         │         └─────────────────┘
         │
         ▼
┌─────────────────┐
│   STOPPED       │
└─────────────────┘
```

### Runner Authentication

Runners authenticate using hashed bearer tokens (not JWTs):

```
Authorization: Bearer bencher_runner_<token>
```

- Token shown exactly once at creation via `POST /v0/runners` (cannot be retrieved later)
- Only SHA-256 hash stored in database
- Prefix `bencher_runner_` makes token type obvious
- Token scoped to runner agent endpoints only

See `RUNNER_DESIGN.md` for token generation, validation, and rotation details.

### Job Claiming (Long-Poll)

The runner uses long-polling to claim jobs via `POST /v0/runners/{runner}/jobs`:

```rust
// Runner daemon main loop
async fn run_daemon(config: &RunnerConfig) -> Result<()> {
    let client = ApiClient::new(&config.api_url, &config.token)?;

    loop {
        // Long-poll to claim a job (max 60s timeout)
        match client.claim_job(&config.runner_slug, &ClaimRequest {
            labels: config.labels.clone(),
            poll_timeout_seconds: 60,
        }).await {
            Ok(ClaimResponse { job: Some(job) }) => {
                // Execute the job with WebSocket channel for heartbeat
                execute_job(&job, config).await;
            }
            Ok(ClaimResponse { job: None }) => {
                // No job available, continue polling
                continue;
            }
            Err(e) => {
                tracing::error!("Claim error: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}
```

### WebSocket Job Channel

After claiming a job, the runner opens a WebSocket connection at `/v0/runners/{runner}/jobs/{job}/channel` for heartbeat and status updates:

```rust
// Runner → Server messages
enum RunnerMessage {
    Running,                                    // Setup complete, benchmark starting
    Heartbeat,                                  // ~1/sec, keeps job alive + triggers billing
    Completed { exit_code: i32, output: Option<String> },
    Failed { exit_code: Option<i32>, error: String },
}

// Server → Runner messages
enum ServerMessage {
    Ack,     // Acknowledge received message
    Cancel,  // Stop execution immediately
}
```

See `RUNNER_DESIGN.md` for the full WebSocket protocol, reconnection flow, and timeout-based job recovery.

### Job Execution Flow

```rust
use crate::firecracker::{FirecrackerProcess, MachineConfig, BootSource, Drive, Vsock, Action};

async fn execute_job(job: &Job, config: &RunnerConfig) {
    // 1. Open WebSocket channel for heartbeat and status
    let ws = connect_ws(&config, &job).await?;

    // 2. Clone repo and run setup_command from job spec
    let work_dir = config.work_dir.join(&job.uuid.to_string());
    std::fs::create_dir_all(&work_dir)?;
    clone_repo(&job.spec.repository, &job.spec.branch, &job.spec.commit, &work_dir)?;
    if let Some(setup_cmd) = &job.spec.setup_command {
        run_setup(setup_cmd, &work_dir)?;
    }

    // 3. Create rootfs from workspace
    let rootfs_path = create_rootfs(&work_dir)?;
    let socket_path = work_dir.join("firecracker.sock");
    let vsock_path = work_dir.join("vsock.sock");

    // 4. Start Firecracker process (via jailer in production)
    let fc = FirecrackerProcess::start(&config.firecracker, &socket_path, &job.uuid)?;

    // 5. Configure and boot the VM
    fc.put_machine_config(&MachineConfig {
        vcpu_count: config.vcpus,
        mem_size_mib: config.memory_mib,
        smt: false,
    }).await?;
    fc.put_boot_source(&BootSource {
        kernel_image_path: config.kernel_path.clone(),
        boot_args: "console=ttyS0 reboot=k panic=1 pci=off rw".into(),
    }).await?;
    fc.put_drive("rootfs", &rootfs_path, true, false).await?;
    fc.put_vsock(3, &vsock_path).await?;
    fc.put_action(Action::InstanceStart).await?;

    // 6. Send "running" over WebSocket, start heartbeat thread
    ws.send(RunnerMessage::Running).await?;
    let heartbeat_handle = spawn_heartbeat(&ws); // ~1/sec on separate CPU core

    // 7. Collect results via vsock (with timeout)
    let output = collect_results_via_vsock(&vsock_path, job.timeout_seconds).await;

    // 8. Shutdown and cleanup
    heartbeat_handle.abort();
    let _ = fc.put_action(Action::SendCtrlAltDel).await;
    fc.kill_after_grace_period(Duration::from_secs(5)).await;

    // 9. Report result over WebSocket
    match output {
        Ok(output) => ws.send(RunnerMessage::Completed {
            exit_code: output.exit_code,
            output: Some(output.stdout),
        }).await?,
        Err(e) => ws.send(RunnerMessage::Failed {
            exit_code: None,
            error: e.to_string(),
        }).await?,
    }

    ws.close().await;
    cleanup_work_dir(&work_dir)?;
}
```

### Guest Init System (bencher-init)

A purpose-built Rust binary (`bencher-init`) runs as PID 1 inside the VM. It is located at `plus/bencher_init/` and compiled into the runner binary.

**Configuration**: The runner writes `/etc/bencher/config.json` to the rootfs before VM boot:

```json
{
  "command": ["/benchmarks/run.sh", "--arg1"],
  "workdir": "/benchmarks",
  "env": [["RUST_BACKTRACE", "1"], ["MY_VAR", "value"]],
  "output_file": "/tmp/results.json"
}
```

**Execution flow**:

1. **Mount filesystems**: `/proc`, `/sys`, `/dev` (devtmpfs), `/tmp` (tmpfs), `/run` (tmpfs)
2. **Setup signal handlers**: SIGTERM/SIGINT for graceful shutdown
3. **Read config**: Parse `/etc/bencher/config.json`
4. **Change to workdir**: `chdir()` to configured working directory
5. **Set environment**: Apply all environment variables from config
6. **Run benchmark**: `fork()` + `execvp()`, capturing stdout/stderr via pipes
7. **Send results via vsock**: Connect to host (CID 2) on dedicated ports
8. **Shutdown**: `reboot(RB_POWER_OFF)`

**Vsock communication** (guest → host):

| Port | Content | Description |
|------|---------|-------------|
| 5000 | stdout | Standard output (full buffer) |
| 5001 | stderr | Standard error (full buffer) |
| 5002 | exit_code | Exit code as string (e.g., "0") |
| 5005 | output_file | Optional file contents (if configured) |

The init binary uses direct serial port I/O (COM1 at 0x3F8) for debug logging, ensuring output even before `/dev` is mounted.

---

## Job Scheduling and Queue

### Database Schema

The database schema follows the design in `RUNNER_DESIGN.md`:

```sql
-- Runner registration and state (server-scoped, serves all projects)
CREATE TABLE runner (
    id UUID PRIMARY KEY,
    uuid UUID NOT NULL UNIQUE,        -- Runner's self-generated ID
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,        -- URL-friendly name
    token_hash TEXT NOT NULL,         -- SHA-256 hash of token (token itself never stored)
    labels JSONB NOT NULL DEFAULT '[]', -- ["arch:arm64", "os:linux"]
    state TEXT NOT NULL DEFAULT 'offline', -- offline, idle, running
    locked TIMESTAMP,              -- If set, runner is locked (token compromised)
    archived TIMESTAMP,               -- Soft delete
    last_heartbeat TIMESTAMP,
    created TIMESTAMP NOT NULL,
    modified TIMESTAMP NOT NULL
);

-- Job queue
CREATE TABLE job (
    id UUID PRIMARY KEY,
    report_id UUID NOT NULL REFERENCES report(id) ON DELETE CASCADE,

    -- Job specification
    status INTEGER NOT NULL DEFAULT 0,  -- JobStatus enum (0-5)
    spec JSONB NOT NULL,                -- Repository, command, env, etc.
    required_labels JSONB DEFAULT '[]',
    timeout_seconds INTEGER NOT NULL DEFAULT 3600,
    priority INTEGER NOT NULL DEFAULT 0,  -- 0 = free, 100 = plus

    -- Execution tracking
    runner_id UUID REFERENCES runner(id) ON DELETE RESTRICT,
    claimed TIMESTAMP,
    started TIMESTAMP,                -- When benchmark actually began (after setup)
    completed TIMESTAMP,
    last_heartbeat TIMESTAMP,
    last_billed_minute INTEGER DEFAULT 0,  -- Minutes billed so far

    -- Results
    exit_code INTEGER,

    created TIMESTAMP NOT NULL,
    modified TIMESTAMP NOT NULL
);

-- Index for job claiming (ordered by priority, then FIFO)
CREATE INDEX idx_job_pending
    ON job(status, priority DESC, created ASC)
    WHERE status = 0;  -- pending
```

### Job Status Enum

```rust
#[repr(u8)]
pub enum JobStatus {
    Pending = 0,
    Claimed = 1,
    Running = 2,
    Completed = 3,
    Failed = 4,
    Canceled = 5,
}
```

### Job Spec Structure

```rust
pub struct JobSpec {
    // Source
    pub repository: Url,
    pub branch: Option<String>,
    pub commit: Option<GitHash>,

    // Execution
    pub setup_command: Option<String>,   // e.g., "cargo build --release"
    pub benchmark_command: String,        // e.g., "cargo bench"
    pub adapter: Option<Adapter>,         // How to parse output

    // Environment
    pub env: HashMap<String, String>,
    pub working_dir: Option<PathBuf>,

    // Timing
    pub timeout_seconds: u32,
    pub expected_seconds: Option<u32>,    // Hint for UI
}
```

### Job State Machine

```
pending ───▶ claimed ───▶ running ───▶ completed
   │            │            │
   │            │            ├────────▶ failed
   │            │            │
   └────────────┴────────────┴────────▶ canceled
```

| From    | To        | Trigger                           |
| ------- | --------- | --------------------------------- |
| pending | claimed   | Runner claims job                 |
| pending | canceled  | User cancels                      |
| claimed | running   | Runner sends `Running` via WS     |
| claimed | failed    | Runner fails during setup         |
| claimed | canceled  | User cancels                      |
| running | completed | Runner sends `Completed` via WS   |
| running | failed    | Runner sends `Failed` or timeout  |
| running | canceled  | User cancels                      |

**Terminal states:** completed, failed, canceled (no transitions out)

### Job Claiming

The claim endpoint (`POST /v0/runners/{runner}/jobs`) handles atomic job assignment:

1. Applies IP-based rate limiting to prevent abuse of long-polling
2. Finds pending jobs across all projects where `required_labels` ⊆ runner's `labels`
3. Atomically updates job status to `claimed`, sets `runner_id` and `claimed` timestamp
4. If no matching jobs, holds connection open until timeout or job arrives
5. Returns job (including project context) or empty response on timeout

Jobs are ordered by `(priority DESC, created ASC)` so Bencher Plus customers always get served first, with FIFO within each tier.

### Timeout-Based Job Recovery

Instead of a periodic reaper, stale jobs are recovered via per-job timeout tasks:

1. **Inline WS timeout** — While the WebSocket connection is open, `tokio::time::timeout(heartbeat_timeout, rx.next())` detects a "connected but silent" runner. On timeout, the job is marked `Failed`.

2. **Spawned disconnect timeout** — When a WebSocket disconnects and the job is still in-flight (non-terminal), a background `tokio::spawn` task sleeps for `heartbeat_timeout`. After waking, it checks:
   - If the job reached a terminal state: do nothing
   - If `last_heartbeat` is recent (within the timeout window): the runner reconnected — reschedule
   - Otherwise: mark the job as `Failed`

3. **Startup recovery** — On server startup, all `Claimed` or `Running` jobs are queried and a timeout task is spawned for each.

Heartbeat timeout is configurable via `ApiContext.heartbeat_timeout` (default: 90 seconds in production, 5 seconds in tests).

### Billing & Priority

Jobs are queued with priority based on the submitting organization's plan:

| Plan                | Priority | Behavior               |
| ------------------- | -------- | ---------------------- |
| Bencher Plus (paid) | High     | Front of queue         |
| Free                | Low      | Waits behind paid jobs |

Usage is tracked per-minute via Stripe's usage-based pricing. Heartbeats serve double duty:

1. **Liveness check** — Confirms runner is still executing the job
2. **Billing increment** — Reports usage to Stripe

On each heartbeat:
1. Update `last_heartbeat` on job and runner
2. Calculate `elapsed_minutes = (now - started) / 60`
3. If `elapsed_minutes > last_billed_minute`, bill the difference to Stripe
4. Update `last_billed_minute = elapsed_minutes`

---

## API and CLI Changes

### New API Endpoints

#### Runner Management (Server Scoped)

Requires server admin permissions.

| Method | Endpoint                     | Description                                    |
| ------ | ---------------------------- | ---------------------------------------------- |
| POST   | `/v0/runners`                | Create runner, returns token                   |
| GET    | `/v0/runners`                | List runners                                   |
| GET    | `/v0/runners/{runner}`       | Get runner details                             |
| PATCH  | `/v0/runners/{runner}`       | Update runner (name, labels, locked, archived) |
| POST   | `/v0/runners/{runner}/token` | Generate new token (invalidates old)           |

#### Job Management (Project Scoped)

Jobs belong to projects (via reports), but can be executed by any runner on the server.

| Method | Endpoint                            | Description               |
| ------ | ----------------------------------- | ------------------------- |
| GET    | `/v0/projects/{project}/jobs`       | List jobs (filterable)    |
| GET    | `/v0/projects/{project}/jobs/{job}` | Get job details + results |

#### Runner Agent Endpoints

Authenticated via runner token (`Authorization: Bearer bencher_runner_<token>`)

| Method    | Endpoint                                  | Description                                            |
| --------- | ----------------------------------------- | ------------------------------------------------------ |
| POST      | `/v0/runners/{runner}/jobs`               | Long-poll to claim a job (from any accessible project) |
| WebSocket | `/v0/runners/{runner}/jobs/{job}/channel` | Heartbeat and status updates during job execution      |

#### Image API (OCI Distribution)

```
# Standard OCI Distribution spec
GET    /v2/                                 # API version check
GET    /v2/{project}/tags/list              # List tags
HEAD   /v2/{project}/manifests/{ref}        # Check manifest exists
GET    /v2/{project}/manifests/{ref}        # Pull manifest
PUT    /v2/{project}/manifests/{ref}        # Push manifest
HEAD   /v2/{project}/blobs/{digest}         # Check blob exists
GET    /v2/{project}/blobs/{digest}         # Pull blob
POST   /v2/{project}/blobs/uploads/         # Start blob upload
PATCH  /v2/{project}/blobs/uploads/{id}     # Upload blob chunk
PUT    /v2/{project}/blobs/uploads/{id}     # Complete blob upload
DELETE /v2/{project}/blobs/{digest}         # Delete blob
```

### CLI Changes

#### New Commands

```bash
# Push an image to Bencher
bencher image push <image-ref>
# Example: bencher image push myproject/benchmark:v1

# List images in a project
bencher image list --project <project>

# Delete an image
bencher image delete <image-ref>

# Run a benchmark using an image
bencher run --image <image-ref> [other existing flags]
# Example: bencher run --image myproject/benchmark:v1 --branch main --testbed linux

# Check job status
bencher job status <job-id>

# View job logs
bencher job logs <job-id>

# Cancel a job
bencher job cancel <job-id>

# List jobs
bencher job list --project <project>
```

#### Runner Commands (for self-hosted)

```bash
# Start the runner daemon
bencher runner start --config runner.toml

# Check runner status
bencher runner status

# Stop the runner daemon
bencher runner stop

# Clear image cache
bencher runner cache clear
```

#### Runner Management (server admin)

```bash
# Create runner (outputs token - shown once)
bencher runner create --name "my-runner" --labels "arch:arm64,os:linux"

# List runners
bencher runner list

# View runner details
bencher runner view <runner>

# Update runner
bencher runner update <runner> --name "new-name" --locked true

# Rotate runner token (invalidates old)
bencher runner token <runner>
```

### Example Workflow

```bash
# 1. Build your benchmark image
docker build -t my-benchmark:v1 .

# 2. Push to Bencher
bencher image push my-benchmark:v1

# 3. Run the benchmark (creates Report + Job)
bencher run \
    --image my-benchmark:v1 \
    --branch main \
    --testbed bare-metal-linux \
    --adapter json

# Report created...
# Job queued (priority based on org plan)...
# Job claimed by runner...
# Job running (heartbeat via WebSocket)...
# Job completed!
# Results attached to Report
```

---

## Security Considerations

### Threat Model

| Threat                       | Mitigation                                                                       |
| ---------------------------- | -------------------------------------------------------------------------------- |
| **Malicious benchmark code** | Firecracker provides hardware isolation via KVM; code cannot escape VM            |
| **Resource exhaustion**      | Strict resource limits (CPU, memory, disk) enforced by VM + jailer cgroups       |
| **Network attacks**          | VMs have no network access; vsock is the only communication channel              |
| **Data exfiltration**        | No network egress; results must be returned via vsock protocol                   |
| **VMM exploit**              | Firecracker's jailer: seccomp, namespaces, chroot, capabilities dropped           |
| **Runner compromise**        | Runners use hashed bearer tokens; scoped to agent endpoints only                 |
| **Image tampering**          | Images are content-addressed (SHA256); verified on pull                          |
| **Timing attacks**           | VMs are destroyed after each job; no persistent state                            |

### Firecracker Security Layers

Firecracker implements defense-in-depth with multiple layers, managed by its **jailer** binary:

```
jailer (PID 1 in namespace)
    │
    ├── Create namespaces (PID, mount, network, user)
    ├── chroot to isolated directory
    ├── Drop all capabilities
    ├── Apply cgroup limits (CPU, memory, pids)
    │
    ▼
firecracker process
    │
    ├── Apply seccomp BPF filters (Firecracker's built-in allowlist)
    │
    ▼
KVM VM execution    <- Hardware-isolated guest
```

**Key security properties:**
- Firecracker runs in its own PID namespace (cannot see host processes)
- Chrooted to a per-VM directory (cannot access host filesystem)
- Network namespace is empty (no host network access)
- Seccomp filter maintained by AWS, regularly audited
- Minimal device model: only virtio-blk, virtio-vsock, serial, i8042

**The runner daemon never needs to implement seccomp or capability management — the jailer handles it.**

### Runner Authentication

Runners authenticate using hashed bearer tokens (not JWTs). Tokens use random bytes with a `bencher_runner_` prefix:

```rust
// Generation (only done once, at runner creation via POST /v0/runners)
let random_bytes: [u8; 32] = rand::random();
let token = format!("bencher_runner_{}", hex::encode(&random_bytes));
// Example: bencher_runner_a1b2c3d4e5f6...

// Storage (only the hash is stored, never the token itself)
let token_hash = sha256(token.as_bytes());
```

**Key properties:**
- Token shown exactly once at creation (cannot be retrieved later)
- Only SHA-256 hash stored in database
- DB breach doesn't expose usable tokens
- Prefix `bencher_runner_` makes token type obvious

**Token scope:**
- Only runner agent endpoints (`/v0/runners/{runner}/jobs[/{job}[/channel]]`)
- Can claim jobs from any project on the server
- Can only perform operations on jobs claimed by this runner

**Token rotation** (if compromised):
1. Lock the runner: `PATCH /v0/runners/{runner}` with `locked: true`
2. Generate new token: `POST /v0/runners/{runner}/token`
3. Update runner agent with new token
4. Unlock the runner: `PATCH /v0/runners/{runner}` with `locked: false`

### Image Security

- Images are stored with content-addressed digests
- Manifests are signed (future: sigstore/cosign integration)
- Images are scanned for vulnerabilities (future: trivy integration)
- Maximum image size enforced (default: 10GB)

### Network Isolation

By default, VMs have **no network access**. The only communication channel is virtio-vsock for returning results to the host.

### Secrets Management

For benchmarks that need secrets (API keys, etc.):

```bash
# Add secret to project
bencher secret set MY_API_KEY --project myproject

# Reference in job
bencher run --image myproject/benchmark:v1 --secret MY_API_KEY
```

Secrets are:
- Encrypted at rest
- Injected into VM as environment variables
- Never logged or persisted in job output
- Scoped to project

---

## Performance Considerations

### CPU Performance

To achieve consistent benchmark results:

1. **Disable SMT (Hyperthreading) in Guest**
   - Firecracker's `machine-config` API accepts `smt: false`

2. **CPU Pinning on Host**
   ```bash
   # Pin runner daemon to specific cores
   taskset -c 4-7 bencher-runner start
   ```

3. **CPU Frequency Scaling**
   ```bash
   # Disable on host for consistent results
   echo performance | tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
   ```

4. **NUMA Awareness**
   ```bash
   # Pin to single NUMA node
   numactl --cpunodebind=0 --membind=0 bencher-runner start
   ```

### Memory Performance

1. **Huge Pages**
   ```bash
   # Enable huge pages for reduced TLB misses
   echo 1024 > /proc/sys/vm/nr_hugepages
   ```

2. **Fixed Memory Allocation**
   - VMs are configured with fixed memory, no ballooning

### Storage Performance

1. **Use local NVMe**
   - Store images and rootfs on local NVMe, not network storage

2. **Rootfs Format**
   - ext4 with `is_read_only: false` in Firecracker drive config
   - ext4 allows benchmarks to write files during execution
   - Use sparse files for faster creation

3. **Pre-convert Images**
   - Image-to-ext4 conversion is cached
   - Subsequent runs get a copy-on-write clone of the cached rootfs

### Warmup Runs

For accurate results, the runner can perform warmup runs:

```rust
// Configurable in job
let job = Job {
    warmup_runs: 2,      // Number of warmup runs (discarded)
    benchmark_runs: 10,  // Number of measured runs
    // ...
};
```

---

## Networking

### Default: No Network

Most benchmarks don't need network access. By default, VMs have no network interface - only virtio-vsock for result collection.

### Future: Isolated Network (Optional)

For benchmarks that need network (e.g., HTTP server benchmarks), a future enhancement could add virtio-net support:

```
┌─────────────────────────────────────────────┐
│                   Host                       │
│  ┌─────────────────────────────────────┐    │
│  │         Bridge: bencher-br0         │    │
│  │                                     │    │
│  │  ┌─────────┐         ┌─────────┐   │    │
│  │  │  VM 1   │         │  VM 2   │   │    │
│  │  │ tap0    │         │ tap1    │   │    │
│  │  │10.0.0.2 │         │10.0.0.3 │   │    │
│  │  └─────────┘         └─────────┘   │    │
│  └─────────────────────────────────────┘    │
│                                             │
│  No connection to external network          │
└─────────────────────────────────────────────┘
```

This is explicitly out of scope for the initial implementation.

---

## Storage and Artifacts

### Artifact Collection

Benchmarks can produce artifacts (logs, profiles, etc.):

```yaml
# In bencher-image.yaml
spec:
  artifacts:
    - path: /benchmarks/results/*.json
      name: results
    - path: /benchmarks/flamegraph.svg
      name: flamegraph
```

Artifacts are collected via vsock after the benchmark completes on port 5005.

### Artifact Storage

Artifacts are stored alongside job results:

```
/artifacts/{project_id}/{job_id}/
├── results/
│   └── benchmark.json
└── flamegraph/
    └── flamegraph.svg
```

Storage backends:
- **Self-hosted**: Local filesystem or S3-compatible
- **Cloud**: S3/R2 with signed URLs for access

### Result Upload

Benchmark results flow back through vsock:

```
VM stdout ──▶ vsock port 5000 ──▶ Runner Daemon ──▶ API Server ──▶ Database
```

Results are parsed based on the configured adapter (same as existing `bencher run` adapters).

---

## Observability

### Metrics

The runner exports Prometheus metrics:

```
# Runner metrics
bencher_runner_jobs_total{status="completed|failed|cancelled"}
bencher_runner_job_duration_seconds{status}
bencher_runner_vm_boot_duration_seconds
bencher_runner_image_pull_duration_seconds
bencher_runner_active_vms

# Resource metrics
bencher_runner_cpu_usage_percent
bencher_runner_memory_usage_bytes
bencher_runner_disk_usage_bytes
```

### Logging

Structured logging with tracing:

```rust
#[instrument(skip(config), fields(job_id = %job.job_id))]
async fn execute_job(job: &Job, config: &RunnerConfig) -> Result<JobResult> {
    tracing::info!("Starting job execution");

    // ... execution ...

    tracing::info!(exit_code = %result.exit_code, "Job completed");
    Ok(result)
}
```

Log levels:
- **ERROR**: Job failures, VM crashes, API errors
- **WARN**: Timeouts, retries, resource pressure
- **INFO**: Job start/complete, runner lifecycle
- **DEBUG**: Detailed execution steps
- **TRACE**: Wire protocol, vsock communication

### Distributed Tracing

OpenTelemetry integration for end-to-end tracing:

```
CLI (bencher run)
  └── API Server (create job)
        └── Job Queue (enqueue)
              └── Runner (poll)
                    └── Firecracker (boot + execute)
                          └── Guest (benchmark)
```

---

## Implementation Phases

These phases align with the `RUNNER_DESIGN.md` implementation phases. Phases 1-2 cover the cloud-side API, phases 3-4 integrate the runner daemon, and phases 5-6 add cloud and BYOR features.

### Phase 1: Runner Registration & Heartbeat

**Goal**: Runners can connect and stay alive

1. **Runner Database & API**
   - Add `runner` table to database (server-scoped)
   - Implement runner management endpoints (`POST/GET/PATCH /v0/runners`)
   - Hashed token authentication (`bencher_runner_<token>`)
   - Token rotation endpoint (`POST /v0/runners/{runner}/token`)

2. **Runner Daemon (MVP)**
   - Accept pre-configured runner token
   - Connect to API and validate token
   - Basic idle/active state tracking

3. **CLI Updates**
   - `bencher runner create/list/view/update/token` (admin commands)

**Deliverable**: Runners can register, authenticate, and maintain connection

### Phase 2: Job Queue & Claiming

**Goal**: Basic job distribution

1. **Job Database & API**
   - Add `job` table linked to `report` via `report_id`
   - Job status enum (pending/claimed/running/completed/failed/canceled)
   - Job spec JSONB (repository, commands, env, adapter)
   - Priority field (0=free, 100=plus)

2. **Job Claiming**
   - Long-poll endpoint (`POST /v0/runners/{runner}/jobs`)
   - Atomic claim with label matching (`required_labels` ⊆ runner `labels`)
   - Priority + FIFO ordering

3. **WebSocket Job Channel**
   - `WebSocket /v0/runners/{runner}/jobs/{job}/channel`
   - Runner → Server: Running, Heartbeat, Completed, Failed
   - Server → Runner: Ack, Cancel
   - Timeout-based job recovery (inline WS timeout + spawned disconnect timeout)
   - Startup recovery for in-flight jobs

4. **Billing Integration**
   - Per-minute usage tracking via heartbeats
   - Stripe usage-based pricing integration
   - `last_billed_minute` tracking to prevent double-counting

5. **Job Management API**
   - `GET /v0/projects/{project}/jobs` (list, filterable)
   - `GET /v0/projects/{project}/jobs/{job}` (details + results)
   - CLI: `bencher job status/list`

**Deliverable**: Jobs can be queued, claimed by runners, and tracked through completion

### Phase 3: Firecracker Integration

**Goal**: Hardware-isolated job execution using Firecracker

1. **Firecracker Client Module**
   - REST API client over Unix domain socket
   - Firecracker/jailer process lifecycle management
   - VM configuration and boot
   - Graceful shutdown (`SendCtrlAltDel` + kill after grace period)

2. **Image-to-Rootfs Pipeline** (exists, needs minor updates)
   - OCI image unpacking ✅ exists
   - Init binary injection ✅ exists (bencher-init)
   - Config.json writing ✅ exists
   - ext4 rootfs creation ✅ exists

3. **Guest Init System** ✅ Complete (no changes needed)
   - Rust-based init binary (`plus/bencher_init/`)
   - vsock-based result streaming (ports 5000-5005)
   - Works identically under Firecracker

4. **Runner Integration**
   - Replace `bencher_vmm` calls with Firecracker client in `plus/bencher_runner/`
   - WebSocket channel integration (heartbeat on separate CPU core)
   - Vsock result collection from Firecracker's vsock UDS

**Deliverable**: Jobs run in isolated Firecracker microVMs with writable rootfs

### Phase 4: Labels & Affinity + Production Hardening

**Goal**: Match jobs to appropriate hardware; production-ready reliability

1. **Labels & Affinity**
   - Runner labels (e.g., `["arch:arm64", "os:linux"]`)
   - Job `required_labels` matching
   - Label-based job routing

2. **Performance Tuning**
   - CPU pinning support
   - Boot time optimization
   - Rootfs caching

3. **Reliability**
   - Job timeout handling (Firecracker `SendCtrlAltDel` + process kill)
   - WebSocket reconnection for transient disconnects
   - Graceful shutdown
   - Error recovery

4. **Observability**
   - Prometheus metrics
   - Structured logging
   - Distributed tracing

**Deliverable**: Production-ready bare metal runner system with hardware-aware scheduling

### Phase 5: Console UI

**Goal**: Manage runners and view job history from the web console

1. **Runner Management UI**
   - List/view runners and their state (offline/idle/running)
   - Create runners, view token (once)
   - Lock/unlock/archive runners
   - Token rotation

2. **Job Dashboard**
   - View job queue and history per project
   - Job details with status, runner assignment, timing
   - Cancel running jobs

3. **Billing Dashboard**
   - Usage tracking per organization
   - Minutes consumed per project

**Deliverable**: Full console UI for runner and job management

### Phase 6: BYOR (Bring Your Own Runner)

**Goal**: Users can connect their own runners to Bencher Cloud

1. **BYOR Security**
   - IP allowlisting
   - Audit logging
   - Rate limiting per runner

2. **Documentation**
   - Runner setup guide
   - Security best practices
   - Troubleshooting

**Deliverable**: Full BYOR support for Bencher Cloud

---

## Open Questions

These open questions apply to this plan. See `RUNNER_DESIGN.md` for additional cloud-side open questions (result storage, output storage, retry policy).

### 1. Image Size Limits
- **Question**: What's the maximum image size we should support?
- **Considerations**: Storage costs, pull time, cache size
- **Proposed**: 10GB max, configurable per plan

### 2. Concurrent Jobs per Runner
- **Question**: Should runners support multiple concurrent jobs?
- **Considerations**: Resource isolation, benchmark accuracy, utilization
- **Proposed**: Single job per runner initially, optional concurrency later with strict CPU pinning

### 3. Windows/macOS Support
- **Question**: When/if to add non-Linux runner support?
- **Considerations**: Firecracker requires KVM (Linux-only)
- **Proposed**: Linux-only; Windows/macOS would require alternative isolation (containers or different VMM)

### 4. ARM Production Readiness
- **Question**: When to enable ARM runners in production?
- **Considerations**: Firecracker supports aarch64, needs production testing
- **Proposed**: After x86_64 is stable in production

### 5. GPU/Accelerator Support
- **Question**: Will benchmarks need GPU access?
- **Considerations**: Firecracker doesn't support GPU passthrough
- **Proposed**: Out of scope for initial implementation; revisit based on demand

### 6. Persistent Storage
- **Question**: Should benchmarks have access to persistent storage across runs?
- **Considerations**: Reproducibility vs. convenience (e.g., caching dependencies)
- **Proposed**: No persistent storage for MVP; ephemeral only

### 7. Network Access
- **Question**: What level of network access should benchmarks have?
- **Considerations**: Security vs. functionality (e.g., downloading dependencies)
- **Proposed**: No network by default; future enhancement for isolated network

### 8. Secret Injection
- **Question**: How should secrets be handled?
- **Considerations**: Security, usability, audit trail
- **Proposed**: Project-scoped secrets, injected as env vars, never logged

---

## Appendix A: Technology Comparison Matrix

| Feature         | Firecracker (chosen) | Cloud Hypervisor | Kata      | gVisor                   | Containers |
| --------------- | -------------------- | ---------------- | --------- | ------------------------ | ---------- |
| CPU Overhead    | <5%                  | <5%              | ~17%      | 0% (CPU), 10x (syscalls) | ~0%        |
| Boot Time       | ~125ms               | ~200ms           | 150-300ms | 50-100ms                 | <50ms      |
| Memory Overhead | ~5 MiB               | ~5-10 MiB        | ~130 MiB  | Low                      | Minimal    |
| Security        | Hardware + jailer    | Hardware         | Hardware  | Application              | Process    |
| OCI Support     | Via conversion       | Via Kata         | Native    | Native                   | Native     |
| KVM Required    | Yes                  | Yes              | Yes       | No                       | No         |
| Maturity        | AWS production       | Moderate         | CNCF      | Google production        | Mature     |
| ARM Support     | Yes                  | Yes              | Yes       | Yes                      | Yes        |

## Appendix B: Runner Hardware Recommendations

### Bencher Cloud Runners (Proposed)

| Tier     | CPU      | RAM    | Storage     | Use Case           |
| -------- | -------- | ------ | ----------- | ------------------ |
| Standard | 8 cores  | 32 GB  | 500 GB NVMe | General benchmarks |
| Compute  | 16 cores | 64 GB  | 500 GB NVMe | CPU-intensive      |
| Memory   | 8 cores  | 128 GB | 500 GB NVMe | Memory-intensive   |

### Self-Hosted Minimum Requirements

| Component | Minimum     | Recommended |
| --------- | ----------- | ----------- |
| CPU       | 4 cores     | 8+ cores    |
| RAM       | 8 GB        | 16+ GB      |
| Storage   | 100 GB SSD  | 500 GB NVMe |
| OS        | Linux 5.10+ | Linux 6.1+  |
| KVM       | Required    | Required    |

## Appendix C: Firecracker Integration Module Structure

```
plus/bencher_runner/src/
├── firecracker/
│   ├── mod.rs              # Public API
│   ├── client.rs           # HTTP client for Firecracker REST API (Unix socket)
│   ├── process.rs          # Firecracker/jailer process lifecycle
│   ├── config.rs           # VM configuration types (MachineConfig, BootSource, Drive, Vsock)
│   └── error.rs            # Error types
├── vsock/
│   ├── mod.rs              # Vsock result collection
│   └── listener.rs         # Listen on vsock UDS for guest results
├── image/
│   ├── mod.rs              # OCI image handling
│   ├── pull.rs             # Image pull from registry
│   ├── rootfs.rs           # OCI-to-ext4 conversion
│   └── cache.rs            # Image/rootfs cache (LRU)
├── run.rs                  # Job execution orchestration
├── daemon.rs               # Runner daemon main loop
└── lib.rs

plus/bencher_init/          # Guest init binary (unchanged)
├── src/
│   └── main.rs             # PID 1: mount, exec benchmark, send results via vsock
└── Cargo.toml

plus/bencher_rootfs/        # Rootfs creation utilities (unchanged)
├── src/
│   ├── ext4.rs             # ext4 image creation via mkfs.ext4
│   └── lib.rs
└── Cargo.toml
```

## Appendix D: References

1. [Firecracker](https://github.com/firecracker-microvm/firecracker) - MicroVM manager
2. [Firecracker Jailer](https://github.com/firecracker-microvm/firecracker/blob/main/docs/jailer.md) - Security isolation
3. [OCI Distribution Specification](https://github.com/opencontainers/distribution-spec)
4. [OCI Image Specification](https://github.com/opencontainers/image-spec)
5. [KVM API Documentation](https://www.kernel.org/doc/html/latest/virt/kvm/api.html)
6. [virtio Specification](https://docs.oasis-open.org/virtio/virtio/v1.1/virtio-v1.1.html)
7. [Linux cgroups v2](https://docs.kernel.org/admin-guide/cgroup-v2.html)
8. [seccomp BPF](https://www.kernel.org/doc/html/latest/userspace-api/seccomp_filter.html)
