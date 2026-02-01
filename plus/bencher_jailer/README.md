# Bencher Jailer

A security jail for isolating VMM processes with multiple layers of Linux isolation primitives.

## Platform Support

| Platform | Status |
|----------|--------|
| Linux | ✅ Full support |
| macOS | ❌ Stub only |
| Windows | ❌ Stub only |

## Isolation Layers

The jailer provides defense-in-depth with multiple isolation mechanisms:

### 1. Linux Namespaces

| Namespace | Purpose |
|-----------|---------|
| **User** | UID/GID isolation, unprivileged operation |
| **PID** | Process isolation, PID 1 inside jail |
| **Mount** | Filesystem isolation via `pivot_root` |
| **Network** | Network isolation (no network access) |
| **UTS** | Hostname isolation |
| **IPC** | IPC isolation (shared memory, semaphores) |
| **Cgroup** | Cgroup visibility isolation |

### 2. Cgroups v2

Resource limits enforced via cgroup v2:

| Resource | Controller | Limit |
|----------|------------|-------|
| CPU | `cpu.max` | Quota per period (e.g., 50% of 1 CPU) |
| Memory | `memory.max` | Hard memory limit in bytes |
| PIDs | `pids.max` | Maximum number of processes |

### 3. Filesystem Isolation

- **`pivot_root`**: Changes the root filesystem to an isolated directory
- **Minimal `/dev`**: Only essential devices (`null`, `zero`, `urandom`, `kvm`)
- **Read-only bind mounts**: For kernel, rootfs, and other required files
- **tmpfs `/dev`**: Prevents device node creation

### 4. Privilege Dropping

- **Capabilities**: All Linux capabilities dropped
- **`PR_SET_NO_NEW_PRIVS`**: Prevents privilege escalation via setuid binaries
- **UID/GID**: Runs as unprivileged user (default: nobody/nogroup)

### 5. Resource Limits (rlimits)

| Limit | Default | Purpose |
|-------|---------|---------|
| `RLIMIT_NOFILE` | 1024 | Max open file descriptors |
| `RLIMIT_NPROC` | 64 | Max processes/threads |
| `RLIMIT_FSIZE` | unlimited | Max file size |
| `RLIMIT_CORE` | 0 | Core dumps disabled |

## Usage

### As a Library

```rust
use bencher_jailer::{JailConfig, Jail};
use camino::Utf8PathBuf;

let config = JailConfig::new(
    "benchmark-123",
    Utf8PathBuf::from("/usr/lib/bencher/vmm"),
    Utf8PathBuf::from("/var/lib/bencher/jails/benchmark-123"),
)
.with_args(vec![
    "--kernel".into(),
    "/kernel".into(),
    "--rootfs".into(),
    "/rootfs.squashfs".into(),
])
.with_memory_limit(512 * 1024 * 1024)  // 512 MiB
.with_cpu_limit(1.0)                    // 1 CPU
.with_bind_mount(
    Utf8PathBuf::from("/path/to/kernel"),
    Utf8PathBuf::from("/kernel"),
);

let mut jail = Jail::new(config)?;
let exit_code = jail.run()?;
```

### As a CLI

```bash
# Basic usage
bencher-jailer \
    --id benchmark-123 \
    --exec /usr/lib/bencher/vmm \
    --jail-root /var/lib/bencher/jails/benchmark-123 \
    --memory 512M \
    --cpu 1.0 \
    -- --kernel /kernel --rootfs /rootfs.squashfs

# From config file
bencher-jailer --config /etc/bencher/jail.json
```

### Config File Format

```json
{
  "id": "benchmark-123",
  "exec_path": "/usr/lib/bencher/vmm",
  "exec_args": ["--kernel", "/kernel", "--rootfs", "/rootfs.squashfs"],
  "jail_root": "/var/lib/bencher/jails/benchmark-123",
  "uid": 65534,
  "gid": 65534,
  "limits": {
    "cpu_quota_us": 100000,
    "cpu_period_us": 100000,
    "memory_bytes": 536870912,
    "max_fds": 1024,
    "max_procs": 64
  },
  "namespaces": {
    "user": true,
    "pid": true,
    "mount": true,
    "network": true,
    "uts": true,
    "ipc": true,
    "cgroup": true
  },
  "bind_mounts": [
    {
      "source": "/path/to/kernel",
      "dest": "/kernel",
      "read_only": true
    }
  ],
  "workdir": "/",
  "env": [
    ["RUST_BACKTRACE", "1"]
  ]
}
```

## Security Model

```
┌─────────────────────────────────────────────────────────────┐
│                    bencher-jailer                           │
│  1. Create cgroup for resource limits                       │
│  2. Set up jail root filesystem                             │
│  3. fork()                                                  │
│     ├── Parent: Add child to cgroup, wait                   │
│     └── Child:                                              │
│         ├── unshare() - Create namespaces                   │
│         ├── Set up UID/GID mapping                          │
│         ├── Mount /proc, /dev                               │
│         ├── pivot_root() - Change root                      │
│         ├── Apply rlimits                                   │
│         ├── PR_SET_NO_NEW_PRIVS                             │
│         ├── Drop all capabilities                           │
│         └── execve() - Run VMM                              │
│             │                                               │
│             ▼                                               │
│         ┌─────────────────────────────────────────────┐     │
│         │              VMM Process                    │     │
│         │  - Isolated filesystem (pivot_root)         │     │
│         │  - No network access                        │     │
│         │  - Resource limited (cgroups)               │     │
│         │  - No capabilities                          │     │
│         │  - Seccomp filters (applied by VMM)         │     │
│         └─────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

## Requirements

- Linux kernel 4.6+ (for cgroup v2)
- Cgroup v2 unified hierarchy mounted at `/sys/fs/cgroup`
- Root or `CAP_SYS_ADMIN` for initial setup (dropped before exec)

## Testing

```bash
# Run tests (requires Linux)
cargo test -p bencher_jailer --features plus

# Check compilation on any platform
cargo check -p bencher_jailer --features plus
```
