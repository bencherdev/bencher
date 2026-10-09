# Runner Ops

Operational tooling for managing bare metal runner servers. Invoked via `cargo ops` (alias for `cargo runner-ops`).

## Server Configuration

Servers are configured in `tasks/runner_ops/runners.json`. Each entry maps a runner name to its SSH connection details and runner key. The runner name is used as the first positional argument in all commands.

Optional per-runner fields:

- `"update_channel": "canary"` puts the runner on the canary update channel: it self-updates to the rolling canary build published on each `cloud` branch deploy, instead of waiting for versioned releases. Omit (or use `"stable"`) for release-only updates. The channel is written to the systemd drop-in as `BENCHER_UPDATE_CHANNEL` by `deploy` and `start`, and `deploy` installs that channel's release; both commands also accept an `--update-channel` flag that overrides the file.

## Add a runner

1. Add the runner's entry to `runners.json`.
2. `cargo ops provision <runner>` installs the OS from the rescue system and hardens it.
3. `cargo ops isolate <runner>` sets the CPU isolation boot args and reboots. The runner service is not installed yet, and `isolate` says so.
4. Wait for the RAID1 initial sync to complete; the runner must take no jobs before then. `cargo ops audit <runner>` reports the sync until it is done (until `deploy` installs the runner, it also reports the runner service as not active and the `runner unit` and `runner binary` sections as failed, which is expected).
5. `cargo ops deploy <runner>` installs the binary of the runner's update channel, checked against its release's checksum. `cargo ops audit <runner>` prints the deployed binary's sha256 under `runner binary`.
6. `cargo ops audit <runner> --against <existing-runner>` until it is clean.

Unattended upgrades install new kernels without rebooting, so a new runner can boot a newer kernel than the rest of the fleet; audit shows it as a `kernel` difference and a pending reboot on the older runners. Kernel parity needs a coordinated reboot of every runner.

## Common Operations

### Deploy the release of the runner's channel

By default `deploy` follows the runner's update channel, from `--update-channel` or `runners.json`, and stable when neither sets it, as on the runner. A canary runner gets `runner-canary-linux-x86-64` from the `canary` release; a stable runner gets `runner-v0.X.Y-linux-x86-64` from the latest tagged release. Each binary is checked against its own `.sha256` from the same release before it is installed. Runners self-update on their channel, so this is only needed to bootstrap a runner or to force an immediate deploy.

```bash
cargo ops deploy <runner>
```

### Deploy a devel build

`--run-id` deploys the `runner-devel-linux-x86-64` artifact of a CI run instead, checked against the checksum uploaded with it. Only a push run of `devel` is accepted, since a pull request from a fork's branch named `devel` uploads artifacts under the same name. GitHub's filtered run list can return a stale page, months old, so check that the run's `headSha` is the commit you want:

```bash
gh run list --repo bencherdev/bencher --branch devel --workflow ci.yml --event push --json databaseId,conclusion,headSha -L 32
cargo ops deploy <runner> --run-id <run_id>
```

### Start/stop/logs

```bash
cargo ops start <runner>
cargo ops stop <runner>
cargo ops logs <runner>
cargo ops logs <runner> --follow
```

### Full provisioning (new server)

See [Add a runner](#add-a-runner) for the full sequence.

```bash
cargo ops provision <runner>
```

### CPU isolation boot args

Configure `isolcpus=`/`nohz_full=`/`rcu_nocbs=` kernel boot args for the benchmark cores via a GRUB drop-in (`/etc/default/grub.d/zz-bencher-isolation.cfg`, named to sort after provider drop-ins that overwrite `GRUB_CMDLINE_LINUX_DEFAULT`), then reboot the server and verify. This clears the runner preflight notice about missing isolation boot args. Idempotent: exits early if the cmdline already has the args (presence-only; it will not re-scope an existing CPU list, even with `--cpus`). The benchmark CPU list defaults to the lowest logical CPU of each physical core, read from the sysfs SMT sibling lists, leaving out the core with CPU 0 for housekeeping: `1-5` on a 6-core Intel part where CPU N pairs with CPU N+6, and `2,4,6,...` where siblings are interleaved, whether SMT is on or already off. Override with `--cpus`; every listed CPU must exist (`/sys/devices/system/cpu/present`) and CPU 0 is not allowed.

```bash
cargo ops isolate <runner>
cargo ops isolate <runner> --cpus 1-5
```

### Audit

Read-only. Collects a snapshot from each runner over one SSH connection, prints how many sections and lines it audited, and checks its health: every section has output and its command exited 0, runner service active, no pending reboot, RAID arrays active and neither syncing nor degraded, no auto-removable packages, and isolation args on the kernel cmdline. Without `--against` it prints the snapshot; with `--against` it prints only the differing lines of each section (`~ order differs` when the same lines come in another order), including non-zero exit statuses, after normalizing the runner name, the root UUID, and the RAID member order. Exits non-zero on any failed check or difference. The runner unit is read through an allowlist of its lines, never `systemctl cat`, and any line holding a runner key is dropped on the server and again on parse, so the key never leaves the runner. The reference runner resolves from `runners.json` only.

```bash
cargo ops audit <runner>
cargo ops audit <runner> --against <reference-runner>
```

## How It Works

1. `deploy` downloads the runner binary from the release of the runner's update channel, or from a devel CI artifact with `--run-id`, checks it against its published checksum, SSHes into the server, stops the existing service, copies the new binary, configures systemd, and starts the service.
2. Server SSH details, runner key, and host URL are resolved by merging `runners.json` with any CLI flags. CLI flags override the JSON file.
3. The host, runner, and update channel are written to a systemd drop-in at `/etc/systemd/system/bencher-runner.service.d/credentials.conf`, and the runner key to `/etc/bencher-runner/key.env`, which the drop-in loads with `EnvironmentFile=`. Both are root only (`0600`): `systemctl show` lists a unit's `Environment=` values to any local user, but never an environment file's contents.
