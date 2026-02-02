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

## Testing Plan

### Prerequisites

- Linux system with KVM enabled (`/dev/kvm` accessible)
- User in `kvm` group or root access
- `mkfs.ext4` installed (part of e2fsprogs)
- Rust toolchain (see `rust-toolchain.toml`)

### Test Levels

#### Level 1: Unit Tests (No KVM Required)

Run on any platform (macOS, Linux, CI):

```bash
# Test rootfs creation
cargo test -p bencher_rootfs --features plus

# Test OCI parsing
cargo test -p bencher_oci --features plus

# Test VMM components (non-KVM parts)
cargo test -p bencher_vmm --features plus
```

**What's tested:**
- ext4 sparse file creation
- squashfs image creation
- OCI image parsing
- OCI registry client (mock)
- seccomp filter compilation
- vsock client protocol

#### Level 2: KVM Integration Tests (Linux + KVM Required)

Run on Linux with KVM access:

```bash
# Run all VMM tests including KVM
cargo test -p bencher_vmm --features plus

# Test that VM can be created
cargo test -p bencher_vmm --features plus -- test_kvm_open
```

**What's tested:**
- KVM device access
- VM creation
- vCPU setup
- Memory mapping
- Capability dropping
- Seccomp filter application

#### Level 3: End-to-End Tests (Linux + KVM + Test Image)

Full integration testing with a test OCI image:

```bash
# Build the runner binary
cargo build -p bencher_runner_bin --release

# Build test image (requires Docker)
cd tests/images/hello-world
docker build -t bencher-test:hello .

# Save as OCI layout
docker save bencher-test:hello | tar -xf - -C /tmp/test-image

# Run the runner
./target/release/runner run --image /tmp/test-image
```

**What's tested:**
- OCI image unpacking
- bencher-init installation
- ext4 rootfs creation
- Kernel boot
- Guest command execution
- vsock result collection
- VM shutdown

### Test Scenarios

#### Scenario 1: Basic Execution

**Purpose:** Verify the happy path works.

```bash
# Create minimal test image
cat > Dockerfile <<EOF
FROM busybox
CMD ["echo", "hello from vm"]
EOF
docker build -t test:basic .

# Run and verify output
./target/release/runner run --image test:basic
# Expected: "hello from vm" in output
```

#### Scenario 2: Environment Variables

**Purpose:** Verify environment variables are passed to guest.

```bash
cat > Dockerfile <<EOF
FROM busybox
ENV MY_VAR=test_value
CMD ["sh", "-c", "echo \$MY_VAR"]
EOF
docker build -t test:env .

./target/release/runner run --image test:env
# Expected: "test_value" in output
```

#### Scenario 3: Working Directory

**Purpose:** Verify working directory is set correctly.

```bash
cat > Dockerfile <<EOF
FROM busybox
WORKDIR /app
COPY . /app
CMD ["pwd"]
EOF
docker build -t test:workdir .

./target/release/runner run --image test:workdir
# Expected: "/app" in output
```

#### Scenario 4: File Output

**Purpose:** Verify output file collection via vsock.

```bash
cat > Dockerfile <<EOF
FROM busybox
CMD ["sh", "-c", "echo '{\"result\": 42}' > /tmp/output.json"]
EOF
docker build -t test:output .

./target/release/runner run --image test:output --output /tmp/output.json
# Expected: Output file contents returned
```

#### Scenario 5: Exit Code

**Purpose:** Verify non-zero exit codes are captured.

```bash
cat > Dockerfile <<EOF
FROM busybox
CMD ["sh", "-c", "exit 42"]
EOF
docker build -t test:exit .

./target/release/runner run --image test:exit
# Expected: Exit code 42 reported
```

#### Scenario 6: Timeout

**Purpose:** Verify timeout handling.

```bash
cat > Dockerfile <<EOF
FROM busybox
CMD ["sleep", "3600"]
EOF
docker build -t test:timeout .

./target/release/runner run --image test:timeout --timeout 5
# Expected: Timeout error after 5 seconds
```

#### Scenario 7: Large Output

**Purpose:** Verify large output doesn't cause issues.

```bash
cat > Dockerfile <<EOF
FROM busybox
CMD ["sh", "-c", "dd if=/dev/urandom bs=1M count=10 | base64"]
EOF
docker build -t test:large .

./target/release/runner run --image test:large
# Expected: ~13MB of base64 output returned
```

#### Scenario 8: Writable Filesystem

**Purpose:** Verify guest can write files to the ext4 rootfs.

```bash
cat > Dockerfile <<EOF
FROM busybox
CMD ["sh", "-c", "echo test > /data.txt && cat /data.txt"]
EOF
docker build -t test:write .

./target/release/runner run --image test:write
# Expected: "test" in output (proves write worked)
```

#### Scenario 9: Stderr Capture

**Purpose:** Verify stderr is captured separately.

```bash
cat > Dockerfile <<EOF
FROM busybox
CMD ["sh", "-c", "echo stdout && echo stderr >&2"]
EOF
docker build -t test:stderr .

./target/release/runner run --image test:stderr
# Expected: Both stdout and stderr captured
```

#### Scenario 10: Multi-CPU

**Purpose:** Verify multi-vCPU works.

```bash
cat > Dockerfile <<EOF
FROM busybox
CMD ["sh", "-c", "cat /proc/cpuinfo | grep processor | wc -l"]
EOF
docker build -t test:cpu .

./target/release/runner run --image test:cpu --vcpus 4
# Expected: "4" in output
```

### Performance Tests

#### Boot Time Measurement

```bash
time ./target/release/runner run --image test:basic --timeout 10
# Target: < 500ms total execution for simple command
```

#### Memory Overhead

```bash
# Monitor memory during execution
./target/release/runner run --image test:basic &
PID=$!
while kill -0 $PID 2>/dev/null; do
    ps -o rss= -p $PID
    sleep 0.1
done
# Target: < 100 MiB overhead for VMM process
```

### Security Tests

#### Scenario S1: Seccomp Enforcement

**Purpose:** Verify seccomp filters block disallowed syscalls.

```bash
cat > Dockerfile <<EOF
FROM busybox
CMD ["sh", "-c", "reboot"]
EOF
docker build -t test:seccomp .

./target/release/runner run --image test:seccomp
# Expected: reboot should fail (blocked by seccomp in guest)
```

#### Scenario S2: Capability Dropping

**Purpose:** Verify capabilities are dropped.

```bash
cat > Dockerfile <<EOF
FROM busybox
CMD ["sh", "-c", "cat /proc/self/status | grep Cap"]
EOF
docker build -t test:caps .

./target/release/runner run --image test:caps
# Expected: CapEff should be 0 or minimal
```

#### Scenario S3: No Network Access

**Purpose:** Verify guest has no network access.

```bash
cat > Dockerfile <<EOF
FROM busybox
CMD ["sh", "-c", "ping -c 1 8.8.8.8 2>&1 || echo no network"]
EOF
docker build -t test:network .

./target/release/runner run --image test:network
# Expected: "no network" or network unreachable error
```

### Continuous Integration

For CI environments without KVM:

```yaml
# GitHub Actions example
jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run unit tests
        run: cargo test -p bencher_rootfs -p bencher_oci --features plus

  integration-tests:
    runs-on: [self-hosted, kvm]  # Requires KVM-enabled runner
    steps:
      - uses: actions/checkout@v4
      - name: Run all tests
        run: cargo test --features plus
      - name: Run E2E tests
        run: ./scripts/run-e2e-tests.sh
```

### Manual Testing Checklist

- [ ] Unit tests pass on macOS
- [ ] Unit tests pass on Linux (no KVM)
- [ ] KVM tests pass on Linux
- [ ] Basic execution works
- [ ] Environment variables work
- [ ] Working directory works
- [ ] Output file collection works
- [ ] Exit code capture works
- [ ] Timeout handling works
- [ ] Large output works
- [ ] Writable filesystem works
- [ ] Stderr capture works
- [ ] Multi-CPU works
- [ ] Boot time < 500ms
- [ ] Memory overhead < 100 MiB
- [ ] Seccomp blocks unauthorized syscalls
- [ ] Capabilities are dropped
- [ ] No network access in guest

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

Check serial output for kernel messages:
```bash
# Enable debug output
RUST_LOG=debug ./target/release/runner run --image test:basic
```

### vsock connection fails

Verify vsock module is loaded:
```bash
lsmod | grep vsock
# If not loaded:
sudo modprobe vhost_vsock
```
