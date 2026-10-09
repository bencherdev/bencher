# Runner Integration Tests

The runner integration tests require Linux with KVM **and root**. They cannot run on macOS directly. A GCP VM is available for running these tests remotely.

`cargo test-runner scenarios` on its own always fails now: the sandbox is built by dropping privilege rather than by starting without it, so the scenarios refuse to run unelevated. Build unprivileged, then run elevated:

```bash
cargo test-runner scenarios --build-only
sudo BENCHER_RUNNER_BIN=./target/debug/runner ./target/debug/test_runner scenarios
```

Keeping the build out of the elevated half is what stops `cargo` leaving root-owned artifacts in the target directory, which is why `BENCHER_RUNNER_BIN` points the elevated binary at the one already built.

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

### Run all scenarios

Two commands, because only the second may be elevated:

```bash
gcloud compute ssh bencher-vmm-test --zone=us-central1-a --project=bencher-411313 \
  --command="export PATH=\$HOME/.cargo/bin:\$PATH && cd bencher && cargo test-runner scenarios --build-only 2>&1"
gcloud compute ssh bencher-vmm-test --zone=us-central1-a --project=bencher-411313 \
  --command="cd bencher && sudo BENCHER_RUNNER_BIN=./target/debug/runner ./target/debug/test_runner scenarios 2>&1"
```

The first builds `bencher-init` (musl, statically linked) and the runner CLI; the second runs all integration scenarios as root. Each scenario builds a Docker image, converts it to OCI format, and runs it inside a Firecracker microVM. Expect this to take several minutes.

### The tuning scenario

`host_tuning` is the only scenario that runs with host tuning enabled, and it runs last. It asserts that each setting the host lets the runner change is changed while the Job runs and still holds after the runner exits, since the runner never reverts its tuning, and that the cpuset partition still reads the level the runner reports. The harness then restores every setting from its own snapshot and removes the `bencher` cgroup, which undoes the partition. It excludes two knobs deliberately: SMT stays on, because offlining a sibling changes the core count for everything that follows and no harness can put a CPU back, and IRQ steering is skipped, because an unmovable IRQ rejects the restoring write with `EIO` so the harness cannot promise to undo it. Governor and turbo are restored but not asserted, since whether the runner can change them depends on the host's cpufreq driver, and most VMs have none. The deep C-state hold is a file descriptor the runner keeps open, and `/dev/cpu_dma_latency` reads 0 while it does, so the scenario asserts that whenever the runner reports the hold.

### Run a single scenario

```bash
gcloud compute ssh bencher-vmm-test --zone=us-central1-a --project=bencher-411313 \
  --command="export PATH=\$HOME/.cargo/bin:\$PATH && cd bencher && cargo test-runner scenarios --build-only 2>&1"
gcloud compute ssh bencher-vmm-test --zone=us-central1-a --project=bencher-411313 \
  --command="cd bencher && sudo BENCHER_RUNNER_BIN=./target/debug/runner ./target/debug/test_runner scenarios --scenario basic_execution 2>&1"
```

### List all scenarios

```bash
gcloud compute ssh bencher-vmm-test --zone=us-central1-a --project=bencher-411313 \
  --command="export PATH=\$HOME/.cargo/bin:\$PATH && cd bencher && cargo test-runner scenarios --list"
```

## Subcommands Reference

| Command                                                                                                | Description                                           |
| ------------------------------------------------------------------------------------------------------ | ----------------------------------------------------- |
| `cargo test-runner scenarios --build-only`                                                             | Build `bencher-init`, the runner CLI, and the harness |
| `sudo BENCHER_RUNNER_BIN=./target/debug/runner ./target/debug/test_runner scenarios`                   | Run all integration test scenarios, as root           |
| `sudo BENCHER_RUNNER_BIN=./target/debug/runner ./target/debug/test_runner scenarios --scenario <name>` | Run a single scenario by name, as root                |
| `cargo test-runner scenarios --list`                                                                   | List all available scenario names                     |

## Typical Workflow

1. Make changes locally (to crates under `plus/bencher_runner/`, `plus/bencher_oci/`, `plus/bencher_init/`, `services/runner/`, `tasks/test_runner/`, etc.)
2. Run local unit tests: `cargo test -p bencher_oci --features plus`, `cargo test -p bencher_runner --features plus`
3. Transfer code to VM via patch (see above)
4. Run scenarios: `cargo test-runner scenarios --build-only`, then `sudo BENCHER_RUNNER_BIN=./target/debug/runner ./target/debug/test_runner scenarios`
5. If a specific scenario fails, re-run it individually with `--scenario <name>` to iterate faster
6. Fix locally, re-patch, re-run

## Key Crates

| Crate                     | Path                            | Role                                                                  |
| ------------------------- | ------------------------------- | --------------------------------------------------------------------- |
| `bencher_runner`          | `plus/bencher_runner/`          | Core runner library (Firecracker VM management, jail, metrics)        |
| `bencher_runner_cli`      | `services/runner/`              | Runner CLI binary (`runner run`, `runner up`)                         |
| `bencher_init`            | `plus/bencher_init/`            | Statically linked init binary that runs inside the VM guest           |
| `bencher_oci`             | `plus/bencher_oci/`             | OCI image parsing and layer extraction                                |
| `bencher_rootfs`          | `plus/bencher_rootfs/`          | Rootfs creation (ext4 image from OCI layers)                          |
| `bencher_output_protocol` | `plus/bencher_output_protocol/` | The guest's results record and its multi-file output protocol         |
| `test_runner`             | `tasks/test_runner/`            | Test harness (`cargo test-runner` task)                               |

## Common Failure Patterns

- **"Path traversal detected"**: The OCI layer extraction in `bencher_oci/src/layer.rs` has defense-in-depth path checks. Docker-saved tars often start with a `./` root entry. If the canonicalization check rejects `./`, the fix is to skip the parent-directory check when `target_path == target_dir`.
- **Compilation errors about private imports**: The `plus` feature gates most code. Check `pub use` re-exports if a type is accessible within a crate but not from outside its module.
- **"KVM is not available"**: The scenarios require `/dev/kvm` on the host. This is why they must run on the GCP VM, not macOS.
- **Timeout scenarios take wall-clock time**: Scenarios like `timeout_handling`, `timeout_enforced`, and `minimum_timeout` intentionally wait for the VM to time out (1-10 seconds each). This is expected.
- **"The scenarios must run as root"**: `cargo test-runner scenarios` was run directly. Use the two-step invocation at the top of this file; the message itself repeats it.
- **Only `Tuning` records with `action` `left`**: expected for every scenario except `host_tuning`. The rest pass `--no-tuning`, because they run as root and would otherwise apply real host tuning to the machine running them, including offlining SMT siblings mid-suite, so the runner only logs each setting's current value.
- **`host_tuning` fails with "No tuning knob on this host can be exercised"**: the host offers nothing the runner can change, so the scenario refuses to pass vacuously. The usual cause is a runner that ran on this host since it booted, which left every knob at its target; reboot the host. The line above it lists what it considered and why each was skipped: absent, already at the target, or not writable.
- **`host_tuning` left the machine tuned**: it should not be possible. The runner leaves its tuning in place until the host reboots, so the harness snapshots every setting the scenario lets the runner change, each CPU's governor and the turbo switches included, restores each from its own copy afterwards, printing `harness restored ...`, and removes the `bencher` cgroup the runner left partitioned. The kernel drops the C-state hold when the runner exits. A `could NOT restore` or `could NOT remove` line names what is still tuned.
