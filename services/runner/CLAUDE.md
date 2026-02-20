# Runner Integration Tests

The runner integration tests (`cargo test-runner scenarios`) require Linux with KVM. They cannot run on macOS directly. A GCP VM is available for running these tests remotely.

## Prerequisites

- `gcloud` CLI installed and available locally
- A GCP service account key file (JSON) — ask the user where to find this key file when you need to connect to the remote VM

## Connecting to the GCP VM

Authenticate and SSH:

```bash
gcloud auth activate-service-account --key-file=<KEY_FILE>
gcloud compute instances list --project=bencher-411313
gcloud compute ssh bencher-vmm-test --zone=us-central1-a --project=bencher-411313 --command="<COMMAND>"
```

The VM does not have `cargo` in `PATH`. Prefix commands with:

```bash
export PATH=$HOME/.cargo/bin:$PATH
```

The repo is cloned at `~/bencher` on the VM. It uses plain git (not jj).

## Transferring Code to the VM

Use `jj diff --git` to create a patch from the VM's current HEAD to your working copy, then `gcloud compute scp` to transfer and `git apply` to apply:

```bash
# 1. Check the VM's current HEAD
gcloud compute ssh bencher-vmm-test --zone=us-central1-a --project=bencher-411313 \
  --command="cd bencher && git log --oneline -3"

# 2. Create a patch from the VM's HEAD to your local working copy
#    Replace <VM_COMMIT> with the commit hash from step 1
jj diff --git -r '<VM_COMMIT>..@' > /tmp/patch.patch

# 3. Copy the patch to the VM
gcloud compute scp /tmp/patch.patch bencher-vmm-test:~/patch.patch \
  --zone=us-central1-a --project=bencher-411313

# 4. Clean and apply on the VM
gcloud compute ssh bencher-vmm-test --zone=us-central1-a --project=bencher-411313 \
  --command="cd bencher && git checkout -- . && git clean -fd && git apply ~/patch.patch"
```

Do NOT use `git push` / `git pull` to transfer code. Always use patches via `gcloud compute scp`.

## Running Tests

All `cargo test-runner` commands must be run on the VM (they require Linux + KVM).

### Clean test artifacts

```bash
gcloud compute ssh bencher-vmm-test --zone=us-central1-a --project=bencher-411313 \
  --command="export PATH=\$HOME/.cargo/bin:\$PATH && cd bencher && cargo test-runner clean"
```

Always clean before a fresh run to avoid stale artifacts.

### Run all scenarios

```bash
gcloud compute ssh bencher-vmm-test --zone=us-central1-a --project=bencher-411313 \
  --command="export PATH=\$HOME/.cargo/bin:\$PATH && cd bencher && cargo test-runner scenarios 2>&1"
```

This builds `bencher-init` (musl, statically linked) and the runner CLI, then runs all ~58 integration scenarios. Each scenario builds a Docker image, converts it to OCI format, and runs it inside a Firecracker microVM. Expect this to take several minutes.

### Run a single scenario

```bash
gcloud compute ssh bencher-vmm-test --zone=us-central1-a --project=bencher-411313 \
  --command="export PATH=\$HOME/.cargo/bin:\$PATH && cd bencher && cargo test-runner scenarios --scenario basic_execution 2>&1"
```

### List all scenarios

```bash
gcloud compute ssh bencher-vmm-test --zone=us-central1-a --project=bencher-411313 \
  --command="export PATH=\$HOME/.cargo/bin:\$PATH && cd bencher && cargo test-runner scenarios --list"
```

### Run the full integration test (kernel + OCI image + VM)

```bash
gcloud compute ssh bencher-vmm-test --zone=us-central1-a --project=bencher-411313 \
  --command="export PATH=\$HOME/.cargo/bin:\$PATH && cd bencher && cargo test-runner 2>&1"
```

This is the default subcommand (`cargo test-runner test`). It downloads a kernel, builds a test OCI image, and runs it in a Firecracker microVM.

## Subcommands Reference

| Command | Description |
|---|---|
| `cargo test-runner` | Full test: kernel + OCI image + benchmark VM (default) |
| `cargo test-runner scenarios` | Run all integration test scenarios |
| `cargo test-runner scenarios --scenario <name>` | Run a single scenario by name |
| `cargo test-runner scenarios --list` | List all available scenario names |
| `cargo test-runner kernel` | Download/cache the Firecracker-compatible kernel only |
| `cargo test-runner oci` | Build the test OCI image only |
| `cargo test-runner clean` | Remove all test artifacts |

## Typical Workflow

1. Make changes locally (to crates under `plus/bencher_runner/`, `plus/bencher_oci/`, `plus/bencher_init/`, `services/runner/`, `tasks/test_runner/`, etc.)
2. Run local unit tests: `cargo test -p bencher_oci --features plus`, `cargo test -p bencher_runner --features plus`
3. Transfer code to VM via patch (see above)
4. Clean: `cargo test-runner clean`
5. Run scenarios: `cargo test-runner scenarios`
6. If a specific scenario fails, re-run it individually with `--scenario <name>` to iterate faster
7. Fix locally, re-patch, re-run

## Key Crates

| Crate | Path | Role |
|---|---|---|
| `bencher_runner` | `plus/bencher_runner/` | Core runner library (Firecracker VM management, vsock, jail, metrics) |
| `bencher_runner_cli` | `services/runner/` | Runner CLI binary (`runner run`, `runner up`) |
| `bencher_init` | `plus/bencher_init/` | Statically linked init binary that runs inside the VM guest |
| `bencher_oci` | `plus/bencher_oci/` | OCI image parsing and layer extraction |
| `bencher_rootfs` | `plus/bencher_rootfs/` | Rootfs creation (ext4 image from OCI layers) |
| `bencher_output_protocol` | `plus/bencher_output_protocol/` | Length-prefixed binary protocol for multi-file output over vsock |
| `test_runner` | `tasks/test_runner/` | Test harness (`cargo test-runner` task) |

## Common Failure Patterns

- **"Path traversal detected"**: The OCI layer extraction in `bencher_oci/src/layer.rs` has defense-in-depth path checks. Docker-saved tars often start with a `./` root entry. If the canonicalization check rejects `./`, the fix is to skip the parent-directory check when `target_path == target_dir`.
- **Compilation errors about private imports**: The `plus` feature gates most code. Check `pub use` re-exports if a type is accessible within a crate but not from outside its module.
- **"KVM is not available"**: The scenarios require `/dev/kvm` on the host. This is why they must run on the GCP VM, not macOS.
- **Timeout scenarios take wall-clock time**: Scenarios like `timeout_handling`, `timeout_enforced`, and `minimum_timeout` intentionally wait for the VM to time out (1-10 seconds each). This is expected.
- **Tuning permission errors in stderr**: Messages like `Tuning: ASLR — skipped (write failed: Permission denied)` are expected when not running as root. They do not cause test failures.
