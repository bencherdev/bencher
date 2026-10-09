# Runner Ops

Operational tooling for managing bare metal runner servers. Invoked via `cargo ops` (alias for `cargo runner-ops`).

## Server Configuration

Servers are configured in `tasks/runner_ops/runners.json`. Each entry maps a runner name to its SSH connection details and runner key. The runner name is used as the first positional argument in all commands.

Optional per-runner fields:

- `"update_channel": "canary"` puts the runner on the canary update channel: it self-updates to the rolling canary build published on each `cloud` branch deploy, instead of waiting for versioned releases. Omit (or use `"stable"`) for release-only updates. The channel is written to the systemd drop-in as `BENCHER_UPDATE_CHANNEL` by `deploy` and `start`, and `deploy` installs that channel's release; both commands also accept an `--update-channel` flag that overrides the file.
- `"scrub_day": 5` is the day of the month, 1 to 28, of the runner's monthly RAID scrub (Ubuntu's `mdcheck_start.timer`), at 02:00 with no random delay. A scrub disturbs the runner's Jobs, so each runner's day must be at least 2 days from every other runner's, counting around the month (day 28 and day 1 are 1 day apart). Every runner needs one: `host` fails without it, and `audit` fails for any runner in `runners.json` that has none.

## Add a runner

1. Add the runner's entry to `runners.json`, with a `scrub_day` at least 2 days from every other runner's.
2. `cargo ops provision <runner>` installs the OS from the rescue system, applies the host settings, and hardens it.
3. `cargo ops isolate <runner>` sets the CPU isolation boot args and reboots. The runner service is not installed yet, and `isolate` says so.
4. Wait for the RAID1 initial sync to complete; the runner must take no jobs before then. `cargo ops audit <runner>` reports the sync until it is done (until `deploy` installs and starts the runner, it also reports the runner service as not active, the `runner unit`, `runner binary`, and `cgroup` sections as failed, and the runner key file, the `bencher` cgroup, the state directory, and the runner binary as differing from the desired state, which is expected).
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

### Host settings

`host` writes every host setting the runner needs that differs from the desired state, then reports the desired state and how many settings it wrote. It is idempotent, so on a runner that matches it writes nothing and says so; run it while the runner is idle. `provision` runs it before its first package upgrade. The settings it writes:

- the SSH hardening drop-in, `/etc/ssh/sshd_config.d/hardening.conf`, then reloads `ssh`;
- the unattended upgrades configs, `/etc/apt/apt.conf.d/50unattended-upgrades-local` and `20auto-upgrades`;
- the kernel pin, `/etc/apt/preferences.d/bencher-kernels.pref`, so kernels come only from the security pocket, never `noble-updates`;
- the RAID scrub drop-in, `/etc/systemd/system/mdcheck_start.timer.d/zz-bencher-scrub.conf`, named to sort after the installer's drop-in that sets a random day, which pins the scrub to the runner's `scrub_day`. It touches the timer's stamp so no day before now counts as missed, writes the drop-in with the timer stopped, reloads systemd, and starts the timer, so moving the day never starts a scrub at once; a scrub the runner missed is not caught up. If a step fails, it starts the timer again only if it was running, so the failure leaves the timer as it found it;
- the masks, with `systemctl mask --now`, of the units a runner has no use for, so none can wake during a Job: `thermald`; the `man-db`, `motd-news`, `update-notifier`, and `e2scrub_all` timers; `sysstat`; Ubuntu Pro's `ua-timer`, `ua-reboot-cmds`, and `ubuntu-advantage`; `apport`; `multipathd`; `iscsid` and `open-iscsi`; `open-vm-tools` and `vgauth`; `lxd-installer`, `gpu-manager`, and `pollinate`; and `snapd`. They are masked, never purged, so the package set stays identical to the image and across runners. A unit that stopping leaves failed is then cleared with `systemctl reset-failed`, so the system reads `running`, not `degraded`. The apt timers, `fstrim`, the RAID check and monitor units, `logrotate`, `dpkg-db-backup`, systemd's own units, `rsyslog`, `fail2ban`, and `acpid` stay.

The desired state is one table that `host` writes and `audit` checks. Its other rows are set elsewhere, and `host` reports them without failing: the runner key file's mode and owner (`start` writes it; neither `host` nor `audit` ever reads its content), the runner's `bencher` cgroup and state directory (the runner sets them up), and the runner binary (`deploy` installs it). `host` fails only when a setting it writes still differs after writing it.

```bash
cargo ops host <runner>
```

### CPU isolation boot args

Configure `isolcpus=`/`nohz_full=`/`rcu_nocbs=` kernel boot args for the benchmark cores via a GRUB drop-in (`/etc/default/grub.d/zz-bencher-isolation.cfg`, named to sort after provider drop-ins that overwrite `GRUB_CMDLINE_LINUX_DEFAULT`), then reboot the server and verify. This clears the runner preflight notice about missing isolation boot args. Idempotent: exits early if the cmdline already has the args (presence-only; it will not re-scope an existing CPU list, even with `--cpus`). The benchmark CPU list defaults to the lowest logical CPU of each physical core, read from the sysfs SMT sibling lists, leaving out the core with CPU 0 for housekeeping: `1-5` on a 6-core Intel part where CPU N pairs with CPU N+6, and `2,4,6,...` where siblings are interleaved, whether SMT is on or already off. Override with `--cpus`; every listed CPU must exist (`/sys/devices/system/cpu/present`) and CPU 0 is not allowed.

```bash
cargo ops isolate <runner>
cargo ops isolate <runner> --cpus 1-5
```

### Audit

Read-only. Collects a snapshot from each runner over one SSH connection, prints how many sections and lines it audited, and checks its health: every section has output and its command exited 0, runner service active, no pending reboot, RAID arrays active and neither syncing nor degraded, no auto-removable packages, and isolation args on the kernel cmdline. It also checks each runner against the desired state that `host` writes: the files `host` writes, the runner key file's mode and owner, the `bencher` cgroup's controllers (`cpuset memory pids`), its effective CPUs (exactly the kernel's isolated CPUs), and its partition (not `invalid`), the state directory (`700 root:root`, on a mount that is neither `nodev` nor `noexec`), the runner binary against the checksum its channel's release publishes, the RAID scrub timer active, on the runner's `scrub_day` alone with no random delay, and each masked unit masked and not running. It fails when a runner in `runners.json` has no `scrub_day`, or when two runners' scrub days are less than 2 days apart. Without `--against` it prints the snapshot; with `--against` it prints only the differing lines of each section (`~ order differs` when the same lines come in another order), including non-zero exit statuses, after normalizing the runner name, the root UUID, and the RAID member order. Exits non-zero on any failed check or difference. The runner unit is read through an allowlist of its lines, never `systemctl cat`, and any line holding a runner key is dropped on the server and again on parse, so the key never leaves the runner. The reference runner resolves from `runners.json` only.

```bash
cargo ops audit <runner>
cargo ops audit <runner> --against <reference-runner>
```

## How It Works

1. `deploy` downloads the runner binary from the release of the runner's update channel, or from a devel CI artifact with `--run-id`, checks it against its published checksum, SSHes into the server, stops the existing service, copies the new binary, configures systemd, and starts the service.
2. Server SSH details, runner key, and host URL are resolved by merging `runners.json` with any CLI flags. CLI flags override the JSON file.
3. The host, runner, and update channel are written to a systemd drop-in at `/etc/systemd/system/bencher-runner.service.d/credentials.conf`, and the runner key to `/etc/bencher-runner/key.env`, which the drop-in loads with `EnvironmentFile=`. Both are root only (`0600`): `systemctl show` lists a unit's `Environment=` values to any local user, but never an environment file's contents.
