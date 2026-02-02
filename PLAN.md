# Bencher Bare Metal Runner Architecture Plan

## Executive Summary

This document outlines the architecture for adding bare metal runner support to Bencher. The design supports both Bencher Cloud and Self-Hosted deployments, with a path toward bring-your-own-runner (BYOR) support. The solution uses **bencher_vmm**, a custom-built VMM based on the rust-vmm ecosystem, providing hardware-level security isolation with minimal performance overhead (<5%), while being fully integrated into the Bencher codebase and deployable as a single binary on any bare metal server with KVM support.

### Why a Custom VMM?

The `bencher_vmm` crate (`plus/bencher_vmm`) provides a purpose-built VMM specifically designed for Bencher's benchmark runner use case. It offers several advantages over using Firecracker as an external dependency:

| Aspect | bencher_vmm | Firecracker |
|--------|-------------|-------------|
| **Deployment** | Single Rust binary | External process + REST API |
| **Integration** | Native Rust, direct function calls | HTTP API, process management |
| **Maintenance** | Full control, tailored to Bencher | Depends on AWS release cycle |
| **Dependencies** | Same rust-vmm crates as Firecracker | External binary |
| **Security** | Equivalent (seccomp + capability dropping) | Equivalent |
| **Features** | Exactly what we need | Many unused features |

The custom VMM is built on the same foundation as Firecracker (rust-vmm crates) but is optimized for Bencher's specific requirements: running isolated benchmarks with minimal overhead and collecting results via vsock.

---

## Table of Contents

1. [Goals and Requirements](#goals-and-requirements)
2. [Architecture Overview](#architecture-overview)
3. [Isolation Strategy](#isolation-strategy)
4. [bencher_vmm Overview](#bencher_vmm-overview)
5. [OCI Image Handling](#oci-image-handling)
6. [Runner Daemon Design](#runner-daemon-design)
7. [Job Scheduling and Queue](#job-scheduling-and-queue)
8. [API and CLI Changes](#api-and-cli-changes)
9. [Security Considerations](#security-considerations)
10. [Performance Considerations](#performance-considerations)
11. [Networking](#networking)
12. [Storage and Artifacts](#storage-and-artifacts)
13. [Observability](#observability)
14. [Implementation Phases](#implementation-phases)
15. [Open Questions](#open-questions)

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
┌─────────────────────────────────────────────────────────────────────────┐
│                           Bencher API Server                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌──────────────┐   │
│  │   Job API   │  │ Runner API  │  │  Image API  │  │  Results API │   │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬───────┘   │
│         │                │                │                │           │
│  ┌──────┴────────────────┴────────────────┴────────────────┴───────┐   │
│  │                        Job Queue (PostgreSQL/SQLite)             │   │
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
            │ │bencher_  │ │ │ │bencher_  │ │ │ │bencher_  │ │
            │ │  vmm     │ │ │ │  vmm     │ │ │ │  vmm     │ │
            │ └──────────┘ │ │ └──────────┘ │ │ └──────────┘ │
            └──────────────┘ └──────────────┘ └──────────────┘
```

### Component Summary

| Component              | Description                                                                        |
| ---------------------- | ---------------------------------------------------------------------------------- |
| **Bencher API Server** | Existing API server, extended with Job, Runner, and Image APIs                     |
| **Job Queue**          | Persistent queue for benchmark jobs (SQLite for self-hosted, PostgreSQL for Cloud) |
| **Image Registry**     | OCI-compliant registry for storing benchmark images                                |
| **Runner Daemon**      | Long-running process on bare metal servers that polls for jobs and executes them   |
| **bencher_vmm**        | Custom VMM providing hardware-level isolation via KVM (part of Bencher codebase)   |

---

## Isolation Strategy

All benchmark jobs run inside isolated VMs managed by `bencher_vmm`. This provides hardware-level isolation suitable for multi-tenant environments.

### bencher_vmm Characteristics

| Criteria           | bencher_vmm                                          |
| ------------------ | ---------------------------------------------------- |
| CPU Overhead       | <5% (>95% of bare metal)                             |
| Boot Time          | ~100-200ms                                           |
| Memory Overhead    | <5 MiB per VM                                        |
| Security Isolation | Hardware-level via KVM + seccomp + capability drop   |
| Architecture       | x86_64 and aarch64 supported                         |
| Vendor Lock-in     | None (open source, runs on any KVM host)             |
| Deployment         | Single binary (kernel embedded in release builds)    |

### Why bencher_vmm?

| Alternative          | Why Not                                                                              |
| -------------------- | ------------------------------------------------------------------------------------ |
| **Plain Containers** | Shared kernel is insufficient security boundary for untrusted multi-tenant workloads |
| **Firecracker**      | External process management, HTTP API overhead, unused features                      |
| **Kata Containers**  | ~17% CPU overhead, 130 MiB memory per pod - too heavy                                |
| **gVisor**           | 10x syscall overhead, unsuitable for syscall-heavy benchmarks                        |
| **Cloud Hypervisor** | External dependency, less control over features                                      |
| **QEMU/KVM**         | Higher overhead, slower boot, more complex                                           |

### Configuration

```toml
# runner.toml
[vmm]
vcpus = 4
memory_mib = 4096
timeout_secs = 300
```

---

## bencher_vmm Overview

The `bencher_vmm` crate is located at `plus/bencher_vmm` and provides a complete VMM implementation.

### Current Features

| Feature | Status | Description |
|---------|--------|-------------|
| **KVM Integration** | ✅ Complete | Full KVM API support via kvm-ioctls |
| **x86_64 Support** | ✅ Complete | bzImage kernel loading, PIC/IOAPIC/PIT |
| **aarch64 Support** | ✅ Complete | GICv3/v2, device tree generation |
| **virtio-blk** | ✅ Complete | Writable ext4 rootfs (changed from read-only squashfs) |
| **virtio-vsock** | ✅ Complete | Host-guest communication for results |
| **Serial Console** | ✅ Complete | Kernel output, fallback results |
| **i8042 Controller** | ✅ Complete | Clean shutdown signaling |
| **Seccomp Filters** | ✅ Complete | ~50 syscall allowlist |
| **Capability Dropping** | ✅ Complete | All capabilities dropped (CAP_NET_ADMIN not needed) |
| **Bundled Kernel** | ✅ Complete | Linux 5.10 LTS, embedded in release |
| **Timeout Handling** | ✅ Complete | Configurable execution timeout |
| **Multi-vCPU** | ✅ Complete | Parallel vCPU execution |
| **Guest Init Binary** | ✅ Complete | Rust-based PID 1 (bencher-init) at `plus/bencher_init/` |

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         Host                                │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                    bencher_vmm                          ││
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────┐  ││
│  │  │  KVM FD  │ │  VM FD   │ │ vCPU FDs │ │  Devices   │  ││
│  │  └──────────┘ └──────────┘ └──────────┘ └────────────┘  ││
│  │                                              │          ││
│  │  ┌─────────────────────────────────────────────────────┐││
│  │  │              Device Manager                         │││
│  │  │  ┌────────┐ ┌────────┐ ┌───────────┐ ┌────────────┐ │││
│  │  │  │ Serial │ │ i8042  │ │virtio-blk │ │virtio-vsock│ │││
│  │  │  └────────┘ └────────┘ └───────────┘ └────────────┘ │││
│  │  └─────────────────────────────────────────────────────┘││
│  └─────────────────────────────────────────────────────────┘│
│                              │                              │
│  ┌───────────────────────────┴───────────────────────────┐  │
│  │                    Guest VM                           │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌────────────────┐ │  │
│  │  │   Kernel    │  │   ext4      │  │   Benchmark    │ │  │
│  │  │  (Linux)    │  │   rootfs    │  │    Process     │ │  │
│  │  └─────────────┘  └─────────────┘  └────────────────┘ │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Usage

```rust
use bencher_vmm::{VmConfig, run_vm};
use camino::Utf8PathBuf;

// Create VM configuration
let config = VmConfig::new(
    Utf8PathBuf::from("/path/to/vmlinux"),
    Utf8PathBuf::from("/path/to/rootfs.squashfs"),
)
.with_vsock(Utf8PathBuf::from("/tmp/vsock.sock"))
.with_timeout(120); // 2 minute timeout

// Run the VM and get results
let results = run_vm(&config)?;
println!("Benchmark results: {results}");
```

### Result Collection

Results are collected via virtio-vsock on dedicated ports:

| Port | Purpose     | Content                            |
| ---- | ----------- | ---------------------------------- |
| 5000 | stdout      | Standard output from the benchmark |
| 5001 | stderr      | Standard error from the benchmark  |
| 5002 | exit_code   | Exit code as a string (e.g., "0")  |
| 5005 | output_file | Optional output file contents      |

The guest init script buffers output and sends it via vsock after the benchmark completes.

### Security Hardening

The VMM implements defense-in-depth:

1. **Capability Dropping**: All Linux capabilities dropped (tested - CAP_NET_ADMIN not needed)
2. **Seccomp Filters**: Strict syscall allowlist (~50 syscalls)
3. **No Network**: No virtio-net device, vsock only for results
4. **Memory Isolation**: Fixed allocation, cannot exceed limit

> **Note**: The current implementation uses read-only squashfs for the rootfs. This needs to change to a writable format (ext4 or overlay) to support benchmarks that write files during execution.

If a guest exploits a bug in virtio parsing:
- ❌ Cannot `execve` to spawn processes
- ❌ Cannot `open` new files
- ❌ Cannot create network sockets
- ❌ Cannot escalate privileges
- ✅ Limited to ~50 KVM-related syscalls

---

## Required Changes to Existing Code

Before proceeding with the implementation phases, the following changes are needed to the existing `bencher_vmm` and `bencher_runner` code:

### 1. Writable Rootfs ✅ COMPLETE

**Solution implemented**: Changed rootfs from squashfs to ext4 using `mkfs.ext4 -d` option.

**Files modified**:
- `plus/bencher_rootfs/src/ext4.rs` - New module for ext4 image creation
- `plus/bencher_rootfs/src/lib.rs` - Export ext4 functions
- `plus/bencher_rootfs/src/error.rs` - Added Ext4 error variant
- `plus/bencher_runner/src/run.rs` - Changed to use ext4 instead of squashfs
- Kernel cmdline changed from `ro` to `rw` for writable root

**How it works**:
1. Create sparse file of specified size (default 1GB)
2. Run `mkfs.ext4 -F -q -m 0 -d <source_dir> <output>` to create and populate
3. The `-d` option copies directory contents during filesystem creation

### 2. CAP_NET_ADMIN Removal ✅ COMPLETE

**Result**: CAP_NET_ADMIN is NOT needed for vsock.

The virtio-vsock implementation uses Unix domain sockets on the host side (not AF_VSOCK), so no network capabilities are required. All 22 tests pass with CAP_NET_ADMIN removed.

**Files modified**:
- `plus/bencher_vmm/src/sandbox.rs` - Now drops ALL capabilities
- `plus/bencher_vmm/README.md` - Updated documentation

### 3. Remove Debug Logging ✅ COMPLETE

**Changes made**: Removed extensive debug logging from `plus/bencher_vmm/src/event_loop.rs`:
- Removed heartbeat logging every 5 seconds
- Removed exit counting and progress logging
- Removed first MMIO read/write logging
- Removed HLT exit logging
- Removed serial output preview logging

The event loop is now clean and produces no output unless there's an error.

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

For bencher_vmm, OCI images must be converted to a bootable rootfs:

```
OCI Image ──▶ Unpack layers ──▶ Install bencher-init ──▶ Write config.json ──▶ Create rootfs image
```

**Rootfs format options** (current: squashfs, needs change to writable):

| Format | Pros | Cons |
|--------|------|------|
| **squashfs** (current) | Compressed, fast to create | Read-only, benchmarks can't write files |
| **ext4 image** | Writable, standard | Larger size, slower to create |
| **overlay on squashfs** | Compressed base + writable layer | More complex, needs kernel support |

The runner daemon handles this conversion (currently in `plus/bencher_runner/src/run.rs`):

```rust
// Simplified flow from actual implementation
async fn convert_oci_to_rootfs(image_ref: &str, config: &BenchmarkConfig) -> Result<Utf8PathBuf> {
    // 1. Pull and unpack OCI image layers
    let unpacked_dir = unpack_oci_image(image_ref).await?;

    // 2. Install bencher-init binary at /init
    install_init_binary(&unpacked_dir)?;

    // 3. Write config.json with command, workdir, env, output_file
    write_init_config(&unpacked_dir, &config)?;

    // 4. Create rootfs image (currently squashfs, should be ext4)
    let rootfs_path = create_rootfs_image(&unpacked_dir)?;

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
1. Registers with the Bencher API server
2. Polls for available jobs
3. Executes benchmarks in isolation via bencher_vmm
4. Reports results back to the API

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
         │         │  (Poll for jobs)│         │
         │         └────────┬────────┘         │
         │                  │                  │
         │ shutdown         │ job assigned     │ job complete
         │                  ▼                  │
         │         ┌─────────────────┐         │
         │         │   PREPARING     │         │
         │         │ (Pull image,    │         │
         │         │  convert rootfs)│         │
         │         └────────┬────────┘         │
         │                  │                  │
         │                  ▼                  │
         │         ┌─────────────────┐         │
         │         │    RUNNING      │─────────┘
         │         │ (Execute in VM) │
         │         └─────────────────┘
         │
         ▼
┌─────────────────┐
│   STOPPED       │
└─────────────────┘
```

### Runner Registration

Runners register with the API server on startup:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct RunnerRegistration {
    /// Unique runner identifier
    pub runner_id: RunnerId,

    /// Human-readable name
    pub name: String,

    /// Runner capabilities
    pub capabilities: RunnerCapabilities,

    /// Runner version
    pub version: String,

    /// Authentication token
    pub token: RunnerToken,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunnerCapabilities {
    /// CPU architecture
    pub arch: Arch,  // x86_64, aarch64

    /// Available vCPUs
    pub vcpu_count: u32,

    /// Available memory (MiB)
    pub memory_mib: u64,

    /// Available disk (MiB)
    pub disk_mib: u64,

    /// CPU model (for matching)
    pub cpu_model: String,

    /// Labels for job matching
    pub labels: HashMap<String, String>,
}
```

### Job Polling

The runner uses long-polling to efficiently wait for jobs:

```rust
// Runner daemon main loop
async fn run_daemon(config: &RunnerConfig) -> Result<()> {
    let client = ApiClient::new(&config.api_url, &config.token)?;

    // Register runner
    let registration = client.register_runner(&capabilities).await?;

    loop {
        // Long-poll for next job (30s timeout)
        match client.poll_job(&registration.runner_id, Duration::from_secs(30)).await {
            Ok(Some(job)) => {
                // Execute the job
                let result = execute_job(&job, config).await;

                // Report result
                client.report_result(&job.job_id, &result).await?;
            }
            Ok(None) => {
                // No job available, continue polling
                continue;
            }
            Err(e) => {
                tracing::error!("Poll error: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}
```

### Job Execution Flow

```rust
use bencher_vmm::{VmConfig, run_vm, write_kernel_to_file};

async fn execute_job(job: &Job, config: &RunnerConfig) -> JobResult {
    // 1. Pull and convert image to squashfs
    let rootfs_path = prepare_image(&job.image_ref).await?;

    // 2. Write bundled kernel to temp file (if needed)
    let kernel_path = config.kernel_path.clone()
        .unwrap_or_else(|| {
            let path = config.cache_dir.join("vmlinux");
            write_kernel_to_file(&path).expect("Failed to write kernel");
            path
        });

    // 3. Create VM configuration
    let vsock_path = create_temp_socket()?;
    let vm_config = VmConfig::new(kernel_path, rootfs_path)
        .with_vsock(vsock_path.clone())
        .with_timeout(job.timeout.as_secs());

    // 4. Run the VM (blocking until complete or timeout)
    let output = run_vm(&vm_config)?;

    // 5. Parse results based on configured adapter
    let results = parse_results(&output, &job.output_format)?;

    // 6. Cleanup
    cleanup_temp_files(&vsock_path, &rootfs_path)?;

    JobResult {
        status: JobStatus::Success,
        results,
        logs: output,
    }
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

### Job Model

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct Job {
    pub job_id: JobId,
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub testbed_id: TestbedId,

    /// Image reference (registry/project/image:tag)
    pub image_ref: String,

    /// Command to execute (overrides image entrypoint)
    pub command: Option<Vec<String>>,

    /// Environment variables
    pub env: HashMap<String, String>,

    /// Resource requirements
    pub resources: ResourceRequirements,

    /// Expected output format
    pub output_format: OutputFormat,

    /// Maximum execution time
    pub timeout: Duration,

    /// Labels for runner matching
    pub labels: HashMap<String, String>,

    /// Job priority (higher = more urgent)
    pub priority: i32,

    /// Job state
    pub state: JobState,

    /// Timestamps
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobState {
    Pending,
    Assigned { runner_id: RunnerId },
    Running { runner_id: RunnerId },
    Completed { result: JobResult },
    Failed { error: String },
    Cancelled,
}
```

### Queue Implementation

For the initial implementation, use the existing SQLite database:

```sql
CREATE TABLE jobs (
    job_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    branch_id TEXT NOT NULL REFERENCES branches(id),
    testbed_id TEXT NOT NULL REFERENCES testbeds(id),

    image_ref TEXT NOT NULL,
    command TEXT,  -- JSON array
    env TEXT,  -- JSON object
    resources TEXT NOT NULL,  -- JSON ResourceRequirements
    output_format TEXT NOT NULL,
    timeout_secs INTEGER NOT NULL,
    labels TEXT,  -- JSON object
    priority INTEGER NOT NULL DEFAULT 0,

    state TEXT NOT NULL DEFAULT 'pending',
    state_data TEXT,  -- JSON, varies by state

    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,

    FOREIGN KEY (project_id) REFERENCES projects(id),
    FOREIGN KEY (branch_id) REFERENCES branches(id),
    FOREIGN KEY (testbed_id) REFERENCES testbeds(id)
);

CREATE INDEX idx_jobs_state ON jobs(state);
CREATE INDEX idx_jobs_priority ON jobs(priority DESC, created_at ASC);
CREATE INDEX idx_jobs_project ON jobs(project_id);
```

### Job Assignment Algorithm

```rust
async fn assign_job_to_runner(
    db: &Database,
    runner: &Runner,
) -> Result<Option<Job>> {
    // Find the highest priority job that:
    // 1. Is in Pending state
    // 2. Matches runner capabilities
    // 3. Has labels that match runner labels

    let job = sqlx::query_as!(Job, r#"
        SELECT * FROM jobs
        WHERE state = 'pending'
        AND (
            -- Resource requirements
            json_extract(resources, '$.vcpus') <= ?
            AND json_extract(resources, '$.memory_mib') <= ?
        )
        ORDER BY priority DESC, created_at ASC
        LIMIT 1
        FOR UPDATE SKIP LOCKED
    "#, runner.capabilities.vcpu_count, runner.capabilities.memory_mib)
    .fetch_optional(db)
    .await?;

    if let Some(job) = job {
        // Atomically assign to runner
        sqlx::query!(r#"
            UPDATE jobs
            SET state = 'assigned',
                state_data = json_object('runner_id', ?)
            WHERE job_id = ? AND state = 'pending'
        "#, runner.runner_id, job.job_id)
        .execute(db)
        .await?;

        Ok(Some(job))
    } else {
        Ok(None)
    }
}
```

### Job Timeout and Recovery

Jobs that don't complete within their timeout are automatically failed:

```rust
// Background task running on API server
async fn job_timeout_checker(db: &Database) {
    loop {
        // Find jobs that have been running too long
        let stuck_jobs = sqlx::query_as!(Job, r#"
            SELECT * FROM jobs
            WHERE state IN ('assigned', 'running')
            AND datetime(started_at, '+' || timeout_secs || ' seconds') < datetime('now')
        "#)
        .fetch_all(db)
        .await?;

        for job in stuck_jobs {
            // Mark as failed
            sqlx::query!(r#"
                UPDATE jobs
                SET state = 'failed',
                    state_data = json_object('error', 'Job timed out'),
                    completed_at = datetime('now')
                WHERE job_id = ?
            "#, job.job_id)
            .execute(db)
            .await?;

            // Notify user
            notify_job_failed(&job, "Job timed out").await?;
        }

        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
```

---

## API and CLI Changes

### New API Endpoints

#### Job API

```
POST   /v0/projects/{project}/jobs          # Create a new job
GET    /v0/projects/{project}/jobs          # List jobs
GET    /v0/projects/{project}/jobs/{job}    # Get job details
DELETE /v0/projects/{project}/jobs/{job}    # Cancel job
GET    /v0/projects/{project}/jobs/{job}/logs  # Get job logs
```

#### Runner API (Internal)

```
POST   /v0/runners/register                 # Register a runner
GET    /v0/runners/{runner}/poll            # Poll for jobs (long-poll)
PUT    /v0/runners/{runner}/heartbeat       # Runner heartbeat
POST   /v0/runners/{runner}/jobs/{job}/start    # Mark job as running
POST   /v0/runners/{runner}/jobs/{job}/complete # Complete job with results
POST   /v0/runners/{runner}/jobs/{job}/fail     # Fail job with error
```

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

# Register runner (outputs token)
bencher runner register --name "my-runner" --project <project>
```

### Example Workflow

```bash
# 1. Build your benchmark image
docker build -t my-benchmark:v1 .

# 2. Push to Bencher
bencher image push my-benchmark:v1

# 3. Run the benchmark
bencher run \
    --image my-benchmark:v1 \
    --branch main \
    --testbed bare-metal-linux \
    --adapter json

# Job created and queued...
# Job assigned to runner runner-001...
# Job running...
# Job completed!
# Results: ...
```

---

## Security Considerations

### Threat Model

| Threat                       | Mitigation                                                                       |
| ---------------------------- | -------------------------------------------------------------------------------- |
| **Malicious benchmark code** | bencher_vmm provides hardware isolation via KVM; code cannot escape VM           |
| **Resource exhaustion**      | Strict resource limits (CPU, memory, disk) enforced by VM configuration          |
| **Network attacks**          | VMs have no network access; vsock is the only communication channel              |
| **Data exfiltration**        | No network egress; results must be returned via vsock protocol                   |
| **VMM exploit**              | Seccomp filters limit VMM to ~50 syscalls; capabilities dropped                  |
| **Runner compromise**        | Runners use short-lived tokens; minimal API access                               |
| **Image tampering**          | Images are content-addressed (SHA256); verified on pull                          |
| **Timing attacks**           | VMs are destroyed after each job; no persistent state                            |

### bencher_vmm Security Layers

The VMM implements defense-in-depth with multiple layers:

```
Vm::new()           <- File opens, mmap, KVM setup (all syscalls allowed)
    │
    ▼
Vm::run()
    │
    ├── drop_capabilities()  <- Drop all caps except CAP_NET_ADMIN
    ├── apply_seccomp()      <- Install syscall filter (irreversible)
    │
    ▼
event_loop::run()   <- Only ~50 allowed syscalls work here
```

**Allowed syscall categories:**
- KVM: `ioctl`
- Memory: `mmap`, `munmap`, `mprotect`, `madvise`, `brk`
- File I/O: `read`, `write`, `close`, `fstat`
- Polling: `ppoll`, `epoll_wait`, `eventfd2`
- Threading: `futex`, `clone3`, `sched_yield`
- Signals: `rt_sigaction`, `rt_sigprocmask`
- Process: `exit`, `exit_group`, `getpid`

**Any syscall not in this list results in immediate process termination.**

### Runner Authentication

Runners authenticate using short-lived JWT tokens:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct RunnerToken {
    /// Runner ID
    pub sub: RunnerId,

    /// Organization ID (for BYOR)
    pub org_id: Option<OrganizationId>,

    /// Issued at
    pub iat: u64,

    /// Expiration (short-lived, 1 hour)
    pub exp: u64,

    /// Permissions
    pub permissions: Vec<RunnerPermission>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RunnerPermission {
    /// Can poll for jobs
    PollJobs,

    /// Can pull images
    PullImages,

    /// Can report results
    ReportResults,
}
```

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
   - bencher_vmm configures VMs without SMT by default

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
   - Currently squashfs (read-only), changing to ext4 (writable)
   - ext4 allows benchmarks to write files during execution
   - Consider sparse files for faster creation

3. **Pre-convert Images**
   - Image-to-squashfs conversion is cached
   - Subsequent runs reuse cached rootfs

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
                    └── bencher_vmm (boot + execute)
                          └── Guest (benchmark)
```

---

## Implementation Phases

### Phase 1: Foundation (3-4 weeks)

**Goal**: Basic job queue and runner daemon

1. **Job Queue**
   - Add `jobs` table to database
   - Implement job CRUD API endpoints
   - Basic job state machine

2. **Runner Daemon (MVP)**
   - Runner registration
   - Job polling (long-poll)
   - Result reporting

3. **CLI Updates**
   - `bencher run --image` flag (local image only)
   - `bencher job status/logs/cancel`

**Deliverable**: Job queue infrastructure ready for VM integration

### Phase 2: Image Registry (2-3 weeks)

**Goal**: OCI-compliant image registry

1. **Image Registry**
   - Implement OCI Distribution API
   - Blob storage (filesystem backend)
   - Image manifest handling

2. **CLI Updates**
   - `bencher image push/pull/list/delete`

3. **Image Caching**
   - Layer deduplication
   - LRU cache eviction

**Deliverable**: Users can push images to Bencher and reference them in jobs

### Phase 3: VMM Integration (2-3 weeks)

**Goal**: Hardware-isolated job execution using bencher_vmm

> Note: This phase is shorter than originally planned because bencher_vmm and the guest init system (bencher-init) already exist. The main work items are:

1. **Image-to-Rootfs Pipeline** (exists, needs modification)
   - OCI image unpacking ✅ exists
   - Init binary injection ✅ exists (bencher-init)
   - Config.json writing ✅ exists
   - **Change squashfs to ext4** ⚠️ needed for writable rootfs

2. **Guest Init System** ✅ Complete
   - Rust-based init binary (`plus/bencher_init/`)
   - vsock-based result streaming (ports 5000-5005)
   - Graceful shutdown via reboot(RB_POWER_OFF)

3. **Security Testing**
   - Test removal of CAP_NET_ADMIN from capability drop
   - Verify vsock still works without network capabilities

4. **Runner Integration** (partially exists)
   - Integrate bencher_vmm into runner daemon ✅ exists (`plus/bencher_runner/`)
   - Job queue integration ⚠️ needs API integration

**Deliverable**: Jobs run in isolated VMs via bencher_vmm with writable rootfs

### Phase 4: Production Hardening (2-3 weeks)

**Goal**: Production-ready performance and reliability

1. **Performance Tuning**
   - CPU pinning support
   - Boot time optimization
   - Rootfs caching

2. **Reliability**
   - Job timeout handling (already in bencher_vmm)
   - Runner failover
   - Graceful shutdown
   - Error recovery

3. **Observability**
   - Prometheus metrics
   - Structured logging
   - Distributed tracing

**Deliverable**: Production-ready bare metal runner system

### Phase 5: Cloud Features (3-4 weeks)

**Goal**: Bencher Cloud-specific features

1. **Multi-tenant Scheduling**
   - Fair scheduling
   - Priority queues
   - Resource quotas

2. **Billing Integration**
   - Usage tracking
   - Cost attribution

3. **BYOR Foundation**
   - Runner token management
   - Organization-scoped runners

**Deliverable**: Bencher Cloud bare metal runner service

### Phase 6: BYOR (Bring Your Own Runner) (3-4 weeks)

**Goal**: Users can connect their own runners to Bencher Cloud

1. **Runner Management UI**
   - Register runner
   - View runner status
   - Manage runner tokens

2. **Runner Security**
   - Token rotation
   - IP allowlisting
   - Audit logging

3. **Documentation**
   - Runner setup guide
   - Security best practices
   - Troubleshooting

**Deliverable**: Full BYOR support for Bencher Cloud

---

## Open Questions

### 1. Image Size Limits
- **Question**: What's the maximum image size we should support?
- **Considerations**: Storage costs, pull time, cache size
- **Proposed**: 10GB max, configurable per plan

### 2. Concurrent Jobs per Runner
- **Question**: Should runners support multiple concurrent jobs?
- **Considerations**: Resource isolation, benchmark accuracy, utilization
- **Proposed**: Single job per runner for Phase 1, optional concurrency later with strict CPU pinning

### 3. Windows/macOS Support
- **Question**: When/if to add non-Linux runner support?
- **Considerations**: bencher_vmm requires KVM (Linux-only)
- **Proposed**: Linux-only; Windows/macOS would require alternative isolation (containers or different VMM)

### 4. ARM Production Readiness
- **Question**: When to enable ARM runners in production?
- **Considerations**: bencher_vmm already supports aarch64, needs production testing
- **Proposed**: Phase 5 or 6, after x86_64 is stable in production

### 5. GPU/Accelerator Support
- **Question**: Will benchmarks need GPU access?
- **Considerations**: bencher_vmm doesn't support GPU passthrough
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

| Feature         | bencher_vmm    | Firecracker    | Cloud Hypervisor | Kata      | gVisor                   | Containers |
| --------------- | -------------- | -------------- | ---------------- | --------- | ------------------------ | ---------- |
| CPU Overhead    | <5%            | <5%            | <5%              | ~17%      | 0% (CPU), 10x (syscalls) | ~0%        |
| Boot Time       | ~100-200ms     | ~125ms         | ~200ms           | 150-300ms | 50-100ms                 | <50ms      |
| Memory Overhead | <5 MiB         | <5 MiB         | ~5-10 MiB        | ~130 MiB  | Low                      | Minimal    |
| Security        | Hardware + seccomp | Hardware    | Hardware         | Hardware  | Application              | Process    |
| OCI Support     | Via conversion | Via containerd | Via Kata         | Native    | Native                   | Native     |
| KVM Required    | Yes            | Yes            | Yes              | Yes       | No                       | No         |
| Integration     | Native Rust    | External proc  | External proc    | External  | External                 | External   |
| ARM Support     | Yes            | Yes            | Yes              | Yes       | Yes                      | Yes        |

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

## Appendix C: bencher_vmm Module Structure

```
plus/bencher_vmm/
├── src/
│   ├── lib.rs              # Public API
│   ├── vm.rs               # VM lifecycle management
│   ├── memory.rs           # Guest memory setup
│   ├── event_loop.rs       # vCPU event loop
│   ├── sandbox.rs          # Seccomp + capability dropping
│   ├── kernel.rs           # Bundled kernel handling
│   ├── error.rs            # Error types
│   ├── vsock_client.rs     # Host-side vsock client
│   ├── boot/
│   │   ├── mod.rs          # Boot abstraction
│   │   ├── x86_64.rs       # x86_64 kernel loading
│   │   └── aarch64.rs      # ARM64 kernel + device tree
│   ├── vcpu/
│   │   ├── mod.rs          # vCPU abstraction
│   │   ├── x86_64.rs       # x86_64 vCPU setup
│   │   └── aarch64.rs      # ARM64 vCPU setup
│   ├── devices/
│   │   ├── mod.rs          # Device manager
│   │   ├── serial.rs       # 16550A UART
│   │   ├── i8042.rs        # Keyboard controller
│   │   ├── pit.rs          # Programmable interval timer
│   │   ├── virtio_blk.rs   # Block device
│   │   └── virtio_vsock.rs # Vsock device
│   └── gic.rs              # ARM GIC (aarch64 only)
├── build.rs                # Kernel download at build time
├── Cargo.toml
└── README.md
```

## Appendix D: References

1. [rust-vmm Project](https://github.com/rust-vmm)
2. [Firecracker](https://github.com/firecracker-microvm/firecracker) - Architecture inspiration
3. [OCI Distribution Specification](https://github.com/opencontainers/distribution-spec)
4. [OCI Image Specification](https://github.com/opencontainers/image-spec)
5. [KVM API Documentation](https://www.kernel.org/doc/html/latest/virt/kvm/api.html)
6. [virtio Specification](https://docs.oasis-open.org/virtio/virtio/v1.1/virtio-v1.1.html)
7. [Linux cgroups v2](https://docs.kernel.org/admin-guide/cgroup-v2.html)
8. [seccomp BPF](https://www.kernel.org/doc/html/latest/userspace-api/seccomp_filter.html)
