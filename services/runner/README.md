# Bencher Runner

The Bencher Runner executes benchmarks in isolated KVM-based virtual machines.

## Overview

The runner takes an OCI image, extracts it into an ext4 rootfs, boots a VM with the bundled kernel, and collects results via virtio-vsock.

```
OCI Image → Unpack → Install bencher-init → Create ext4 → Boot VM → Collect Results
```

## Components

| Component        | Location               | Purpose                               |
| ---------------- | ---------------------- | ------------------------------------- |
| `bencher_runner` | `plus/bencher_runner/` | Runner library (uses Firecracker)     |
| `bencher_init`   | `plus/bencher_init/`   | Guest init binary (PID 1)             |
| `bencher_rootfs` | `plus/bencher_rootfs/` | Rootfs creation (ext4/squashfs)       |
| `bencher_oci`    | `plus/bencher_oci/`    | OCI image parsing and registry client |
| `runner` binary  | `services/runner/`     | CLI binary (this directory)           |

## Building

The `bencher-init` binary runs as PID 1 inside the Firecracker VM and **must be
statically linked** (the guest rootfs does not contain glibc). Always build it
with the musl target:

```bash
# Install the musl target (once)
rustup target add x86_64-unknown-linux-musl

# Build bencher-init (statically linked)
cargo build --release --target x86_64-unknown-linux-musl -p bencher_init

# Build bencher-runner (bundles bencher-init, firecracker, and vmlinux)
cargo build --release -p bencher_runner --features plus
```

The `bencher_runner` build script automatically finds the musl-built
`bencher-init` in `target/x86_64-unknown-linux-musl/`. You can also set
`BENCHER_INIT_PATH` to point to a pre-built binary.

## Running Tests

### Prerequisites

- Linux system with KVM enabled (`/dev/kvm` accessible)
- User in `kvm` group or root access
- `mkfs.ext4` installed (part of e2fsprogs)
- `musl-tools` installed (for static linking)
- Rust toolchain with `x86_64-unknown-linux-musl` target

### Unit Tests (No KVM Required)

Run on any platform:

```bash
# Test individual components
cargo test -p bencher_rootfs --features plus
cargo test -p bencher_oci --features plus
cargo test -p bencher_runner --features plus
cargo test -p bencher_init
```

### Integration Test (Linux + KVM Required)

The integration test boots a Firecracker VM and runs `bencher mock` inside it:

```bash
# Build bencher-init with musl first
cargo build --target x86_64-unknown-linux-musl -p bencher_init

# Run the integration test
BENCHER_INIT_PATH=$(pwd)/target/x86_64-unknown-linux-musl/debug/bencher-init \
  cargo test-runner test
```

### Scenario Tests (Linux + KVM + Docker Required)

The scenario tests each build a Docker image, convert it to OCI, and boot a Firecracker VM:

```bash
# Build the runner binary first
BENCHER_INIT_PATH=$(pwd)/target/x86_64-unknown-linux-musl/debug/bencher-init \
  cargo build -p bencher_runner_bin --features plus

# Run all scenarios
BENCHER_INIT_PATH=$(pwd)/target/x86_64-unknown-linux-musl/debug/bencher-init \
  cargo test-runner scenarios

# List available scenarios
cargo test-runner scenarios --list

# Run a single scenario
BENCHER_INIT_PATH=$(pwd)/target/x86_64-unknown-linux-musl/debug/bencher-init \
  cargo test-runner scenarios --scenario basic_execution
```

| Scenario | Description |
|----------|-------------|
| `basic_execution` | Simple echo command |
| `environment_variables` | ENV variables passed to guest |
| `working_directory` | WORKDIR set correctly |
| `file_output` | Output file collection via vsock |
| `exit_code` | Non-zero exit codes captured |
| `timeout_handling` | VM killed after timeout |
| `writable_filesystem` | Guest can write to ext4 rootfs |
| `stderr_capture` | Stderr captured separately |
| `multi_cpu` | Multiple vCPUs work |
| `entrypoint_with_args` | ENTRYPOINT + CMD combined |
| `no_network_access` | Guest has no network |
| `output_flood` | Large output is truncated (not OOM) |
| `timeout_enforced` | Timeout kills hanging process |
| `uid_namespace_isolation` | User namespace UID mapping works correctly |
| `dev_kvm_available` | /dev/kvm accessible inside jail |
| `proc_mount_works` | /proc accessible inside jail |
| `rootfs_writable` | Rootfs mounted read-write (not read-only) |
| `timeout_includes_partial_output` | Timeout errors include partial output |
| `no_seccomp_sigsys` | Seccomp filter allows required syscalls |
| `unique_output_validation` | Output comes from VM, not runner logs |
| `pid_namespace_isolation` | PID namespace prevents seeing host PIDs |
| `pid_namespace_procfs` | Fresh procfs mount works with PID namespace |
| `metrics_output_present` | Metrics marker present in stderr |
| `metrics_wall_clock_reasonable` | Wall clock time is within reasonable bounds |
| `metrics_timeout_flag` | Timeout flag set correctly in metrics |
| `hmac_verification_logged` | HMAC verification status is logged |
| `metrics_transport_type` | Transport type reported in metrics |
| `job_cancelled` | SIGTERM cancels a running VM cleanly |
| `stderr_only` | Stderr captured when stdout is empty |
| `empty_output` | Process exits 0 with no output |
| `binary_output` | Non-UTF8 stdout handled gracefully |
| `shell_form_cmd` | Shell-form CMD (string, not array) works |
| `entrypoint_only` | ENTRYPOINT exec form with no CMD |
| `shell_form_entrypoint` | ENTRYPOINT shell form (string, not array) |
| `entrypoint_shell_with_cmd` | Shell-form ENTRYPOINT ignores CMD args |
| `no_cmd_no_entrypoint` | No CMD or ENTRYPOINT fails gracefully |
| `rapid_exit` | Instantly exiting process doesn't lose results |
| `signal_exit` | Signal-killed process reports 128+signal exit code |
| `large_env` | Many/large environment variables work |

### Manual Testing

```bash
# Build a test image
cat > /tmp/Dockerfile <<EOF
FROM busybox
CMD ["echo", "hello from vm"]
EOF
docker build -t test:basic /tmp

# Save as OCI layout
mkdir -p /tmp/test-oci
docker save test:basic | tar -xf - -C /tmp/test-oci

# Run the runner
./target/debug/runner run --image /tmp/test-oci --timeout 60
```

## Google Cloud VM Setup

To run integration tests on a Google Cloud VM, follow these steps:

### 1. Create VM with Nested Virtualization

Create an n1-standard-2 (or larger) instance with nested virtualization enabled:

```bash
gcloud compute instances create bencher-vmm-test \
  --zone=us-central1-a \
  --machine-type=n1-standard-2 \
  --image-family=ubuntu-2404-lts-amd64 \
  --image-project=ubuntu-os-cloud \
  --min-cpu-platform="Intel Haswell" \
  --enable-nested-virtualization
```

### 2. Install Required Packages

SSH into the VM and install dependencies:

```bash
# Install system packages
sudo apt-get update
sudo apt-get install -y e2fsprogs squashfs-tools musl-tools

# Add user to kvm group
sudo usermod -aG kvm $USER

# Install Rust with musl target
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup target add x86_64-unknown-linux-musl
```

### 3. Disable AppArmor User Namespace Restrictions

Ubuntu 24.04 has AppArmor restrictions on unprivileged user namespaces that must be disabled:

```bash
# Disable the restriction (required each boot)
sudo sysctl kernel.apparmor_restrict_unprivileged_userns=0

# Unload AppArmor profiles
sudo aa-teardown
```

To make the sysctl setting persistent across reboots:

```bash
echo "kernel.apparmor_restrict_unprivileged_userns=0" | sudo tee /etc/sysctl.d/99-userns.conf
```

### 4. Verify Prerequisites

```bash
# Check KVM access
ls -la /dev/kvm

# Check Docker
docker version

# Check mkfs.ext4
mkfs.ext4 -V

# Run prerequisite check
cargo test -p bencher_runner_bin --test integration --features plus -- check_prerequisites --nocapture
```

Expected output:
```
KVM available: true
Docker available: true
mkfs.ext4 available: true
```

### 5. Run Integration Tests

```bash
# Build bencher-init with musl (statically linked for guest VM)
cargo build --target x86_64-unknown-linux-musl -p bencher_init

# Run the integration test (boots a Firecracker VM)
BENCHER_INIT_PATH=$(pwd)/target/x86_64-unknown-linux-musl/debug/bencher-init \
  cargo test-runner test
```

## Troubleshooting

### "Permission denied" on /dev/kvm

```bash
# Add user to kvm group
sudo usermod -aG kvm $USER
# Log out and back in
```

### "mkfs.ext4 not found"

```bash
# Ubuntu/Debian
sudo apt-get install e2fsprogs

# Fedora/RHEL
sudo dnf install e2fsprogs
```

### VM boot hangs

Enable debug output:

```bash
RUST_LOG=debug ./target/debug/runner run --image /tmp/test-oci
```

### vsock connection fails

Verify vsock module is loaded:

```bash
lsmod | grep vsock
# If not loaded:
sudo modprobe vhost_vsock
```

### "failed to write uid_map: Operation not permitted"

This error occurs when AppArmor is blocking user namespace operations. Fix:

```bash
# Check if AppArmor is restricting user namespaces
sysctl kernel.apparmor_restrict_unprivileged_userns

# Disable the restriction
sudo sysctl kernel.apparmor_restrict_unprivileged_userns=0

# Unload AppArmor profiles
sudo aa-teardown
```

### "unshare user namespace failed: EACCES"

Same fix as above - AppArmor restrictions need to be disabled.
