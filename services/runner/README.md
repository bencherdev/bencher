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
| `bencher_runner` | `plus/bencher_runner/` | Runner library                        |
| `bencher_vmm`    | `plus/bencher_vmm/`    | Virtual machine monitor               |
| `bencher_init`   | `plus/bencher_init/`   | Guest init binary (PID 1)             |
| `bencher_rootfs` | `plus/bencher_rootfs/` | Rootfs creation (ext4/squashfs)       |
| `bencher_oci`    | `plus/bencher_oci/`    | OCI image parsing and registry client |
| `runner` binary  | `services/runner/`     | CLI binary (this directory)           |

## Running Tests

### Prerequisites

- Linux system with KVM enabled (`/dev/kvm` accessible)
- User in `kvm` group or root access
- `mkfs.ext4` installed (part of e2fsprogs)
- Docker (for building test images)
- Rust toolchain (see `rust-toolchain.toml`)

### Unit Tests (No KVM Required)

Run on any platform:

```bash
# Test individual components
cargo test -p bencher_rootfs --features plus
cargo test -p bencher_oci --features plus
cargo test -p bencher_vmm --features plus
```

### Integration Scenarios (Linux + KVM + Docker Required)

Integration scenarios are run via the test_runner task:

```bash
# Build the runner first
cargo build -p bencher_runner_bin

# List available scenarios
cargo test-runner scenarios --list

# Run all scenarios
cargo test-runner scenarios

# Run a specific scenario
cargo test-runner scenarios --scenario basic_execution
```

The scenarios cover:

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
# Install Docker
sudo apt-get update
sudo apt-get install -y docker.io e2fsprogs

# Add user to docker and kvm groups
sudo usermod -aG docker,kvm $USER

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
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
# Build the runner
cargo build -p bencher_runner_bin --features plus

# Run all integration tests
cargo test -p bencher_runner_bin --test integration --features plus -- --ignored --nocapture
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
