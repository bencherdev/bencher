# Bencher VMM

A minimal, security-focused Virtual Machine Monitor (VMM) for running benchmarks in isolated Linux VMs.

Built on the [rust-vmm](https://github.com/rust-vmm) ecosystem, this VMM is designed specifically for Bencher's benchmark runner. It prioritizes security and simplicity over feature completeness.

## Platform Support

| Platform      | Status               |
| ------------- | -------------------- |
| Linux x86_64  | ✅ Full support       |
| Linux aarch64 | ✅ Full support       |
| macOS         | ❌ Stub only (no KVM) |
| Windows       | ❌ Stub only (no KVM) |

**Requirements:**
- Linux kernel 4.14+ with KVM enabled
- `/dev/kvm` accessible (user must be in `kvm` group or root)

## Features

### Devices

| Device                           | Purpose                                                         |
| -------------------------------- | --------------------------------------------------------------- |
| **Serial Console** (16550A UART) | Kernel output capture, fallback result collection               |
| **i8042 Keyboard Controller**    | Clean VM shutdown via keyboard commands                         |
| **virtio-blk**                   | Read-only squashfs rootfs mounting                              |
| **virtio-vsock**                 | High-performance host-guest communication for result collection |

### Bundled Kernel

The VMM includes a pre-built Linux kernel, eliminating the need for users to provide one:

- **Source**: [Firecracker CI](https://github.com/firecracker-microvm/firecracker) builds (v1.10)
- **Kernel Version**: Linux 5.10 (LTS)
- **Architectures**: x86_64 (~43MB), aarch64 (~19MB)

**Build Behavior:**
- **Release builds**: Kernel is embedded in the binary via `include_bytes!()`
- **Debug builds**: Kernel is loaded from disk at runtime (faster compilation)

The kernel is downloaded once during the first build and cached in `OUT_DIR`.

```rust
use bencher_vmm::{kernel_bytes, write_kernel_to_file};

// Get raw kernel bytes
let bytes = kernel_bytes();

// Or write to a file (needed for KVM)
write_kernel_to_file(Path::new("/tmp/vmlinux"))?;
```

### Execution Timeout

VMs have a configurable execution timeout (default: 5 minutes):

```rust
let config = VmConfig::new(kernel_path, rootfs_path)
    .with_timeout(60); // 60 second timeout
```

If the VM doesn't shut down within the timeout, it's forcefully terminated and `VmmError::Timeout` is returned.

## Security Hardening

The VMM implements defense-in-depth with multiple security layers:

### 1. Capability Dropping

All Linux capabilities are dropped before VM execution, except `CAP_NET_ADMIN` (required for vsock on some systems). This prevents privilege escalation even if the VMM process is compromised.

### 2. Seccomp Filters

A strict syscall allowlist is enforced using seccomp-bpf. Only ~35 syscalls required for KVM operation are permitted:

| Category      | Allowed Syscalls                                                                 |
| ------------- | -------------------------------------------------------------------------------- |
| **KVM**       | `ioctl`                                                                          |
| **Memory**    | `mmap`, `munmap`, `mprotect`, `madvise`, `brk`                                   |
| **File I/O**  | `read`, `write`, `close`, `fstat`, `newfstatat`                                  |
| **Polling**   | `ppoll`, `epoll_wait`, `epoll_pwait`, `eventfd2`, `fcntl`                        |
| **Timing**    | `clock_gettime`, `nanosleep`, `clock_nanosleep`                                  |
| **Threading** | `futex`, `set_robust_list`, `rseq`, `sched_yield`, `sched_getaffinity`, `gettid` |
| **Signals**   | `rt_sigaction`, `rt_sigprocmask`, `rt_sigreturn`, `sigaltstack`                  |
| **Process**   | `exit`, `exit_group`, `getpid`, `getrandom`, `prctl`                             |

**Any syscall not in this list results in immediate process termination.**

### 3. Guest Isolation

| Aspect         | Protection                                          |
| -------------- | --------------------------------------------------- |
| **Filesystem** | Read-only squashfs, no write capability             |
| **Network**    | None (no virtio-net device)                         |
| **Host Files** | No access (vsock is the only communication channel) |
| **Memory**     | Fixed allocation, cannot exceed configured limit    |

### When Sandboxing is Applied

The sandbox is applied in `Vm::run()`, **after** all privileged setup is complete:

```
Vm::new()           <- File opens, mmap, KVM setup (privileged syscalls allowed)
    │
    ▼
Vm::run()
    │
    ├── drop_capabilities()  <- Drop all caps except CAP_NET_ADMIN
    ├── apply_seccomp()      <- Install syscall filter (irreversible)
    │
    ▼
event_loop::run()   <- Only allowed syscalls work here
```

This ensures setup operations succeed while maximally restricting the VM execution phase.

### Attack Surface Reduction

If a guest exploits a bug in virtio parsing or vsock handling, the attacker:

- ❌ Cannot `execve` to spawn processes
- ❌ Cannot `open` new files
- ❌ Cannot create network sockets
- ❌ Cannot escalate privileges
- ❌ Cannot access the filesystem
- ✅ Limited to ~35 KVM-related syscalls

## Usage

### Basic Usage

```rust
use bencher_vmm::{VmConfig, run_vm};
use camino::Utf8PathBuf;

let config = VmConfig::new(
    Utf8PathBuf::from("/path/to/vmlinux"),
    Utf8PathBuf::from("/path/to/rootfs.squashfs"),
);

let results = run_vm(&config)?;
println!("Benchmark results: {results}");
```

### With Options

```rust
let config = VmConfig::new(kernel_path, rootfs_path)
    .with_vsock(Utf8PathBuf::from("/tmp/vsock.sock"))
    .with_timeout(120); // 2 minute timeout

let mut vm = Vm::new(&config)?;
let results = vm.run()?;
```

### Configuration Options

| Field            | Type                  | Default   | Description                      |
| ---------------- | --------------------- | --------- | -------------------------------- |
| `kernel_path`    | `Utf8PathBuf`         | Required  | Path to vmlinux kernel           |
| `rootfs_path`    | `Utf8PathBuf`         | Required  | Path to squashfs rootfs          |
| `vcpus`          | `u8`                  | 1         | Number of virtual CPUs           |
| `memory_mib`     | `u32`                 | 512       | Memory in MiB                    |
| `kernel_cmdline` | `String`              | See below | Kernel boot arguments            |
| `vsock_path`     | `Option<Utf8PathBuf>` | None      | Unix socket for vsock            |
| `timeout_secs`   | `u64`                 | 300       | Execution timeout (0 = disabled) |

**Default kernel command line:**
```
console=ttyS0 reboot=k panic=1 pci=off root=/dev/vda ro
```

## Architecture

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
│  │  │   Kernel    │  │  squashfs   │  │   Benchmark    │ │  │
│  │  │  (Linux)    │  │   rootfs    │  │    Process     │ │  │
│  │  └─────────────┘  └─────────────┘  └────────────────┘ │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Result Collection

Results can be collected via two mechanisms:

### 1. virtio-vsock (Preferred)

The guest sends results over vsock to the host using dedicated ports. This is faster and more reliable than serial output.

- **Guest CID**: 3
- **Host CID**: 2

**Port Assignments:**

| Port | Purpose | Content |
| ---- | ------- | ------- |
| 5000 | stdout | Standard output from the benchmark |
| 5001 | stderr | Standard error from the benchmark |
| 5002 | exit_code | Exit code as a string (e.g., "0") |
| 5005 | output_file | Optional output file contents |

The guest init script:
1. Buffers stdout/stderr to `/run/bencher/stdout` and `/run/bencher/stderr`
2. Captures the exit code to `/run/bencher/exit_code`
3. Sends all files via vsock after the benchmark completes
4. If `BENCHER_OUTPUT_FILE` is set, sends that file on port 5005

### 2. Serial Output (Fallback)

If vsock is not configured or fails, results are captured from the serial console (ttyS0).

## Dependencies

Core rust-vmm crates:
- `kvm-ioctls` / `kvm-bindings` - KVM interface
- `vm-memory` - Guest memory management
- `linux-loader` - Kernel loading (ELF/bzImage)
- `vm-superio` - Serial/i8042 device emulation
- `virtio-queue` - Virtio queue handling

Security:
- `seccompiler` - Seccomp filter compilation
- `caps` - Linux capability management

Build:
- `ureq` - HTTP client for kernel downloads

## Error Handling

All errors are wrapped in `VmmError`:

```rust
pub enum VmmError {
    Kvm(kvm_ioctls::Error),      // KVM ioctl failures
    Memory(String),               // Memory allocation/mapping
    Boot(String),                 // Kernel loading
    Device(String),               // Device setup/handling
    Vcpu(String),                 // vCPU creation/execution
    Gic(String),                  // ARM GIC setup
    Io(std::io::Error),          // General I/O
    KernelLoad(String),          // Kernel file loading
    UnsupportedArch,             // Non-x86_64/aarch64
    UnsupportedPlatform,         // Non-Linux
    Timeout(u64),                // Execution timeout
    Sandbox(String),             // Seccomp/capability errors
}
```

## Testing

```bash
# Run tests (requires /dev/kvm on Linux)
cargo test -p bencher_vmm --features plus

# Check compilation on any platform
cargo check -p bencher_vmm --features plus
```

## License

See the repository root for license information.

## References

- [rust-vmm Project](https://github.com/rust-vmm)
- [Firecracker](https://github.com/firecracker-microvm/firecracker) - Inspiration for architecture
- [KVM API Documentation](https://www.kernel.org/doc/html/latest/virt/kvm/api.html)
- [virtio Specification](https://docs.oasis-open.org/virtio/virtio/v1.1/virtio-v1.1.html)
