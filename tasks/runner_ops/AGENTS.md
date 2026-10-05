# Runner Ops

Operational tooling for managing bare metal runner servers. Invoked via `cargo ops` (alias for `cargo runner-ops`).

## Server Configuration

Servers are configured in `tasks/runner_ops/runners.json`. Each entry maps a runner name to its SSH connection details and runner key. The runner name is used as the first positional argument in all commands.

Optional per-runner fields:

- `"update_channel": "canary"` puts the runner on the canary update channel: it self-updates to the rolling canary build published on each `cloud` branch deploy, instead of waiting for versioned releases. Omit (or use `"stable"`) for release-only updates. The channel is written to the systemd drop-in as `BENCHER_UPDATE_CHANNEL` by `deploy` and `start`, and `deploy` installs that channel's release; both commands also accept an `--update-channel` flag that overrides the file.

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

```bash
cargo ops provision <runner>
```

### CPU isolation boot args

Configure `isolcpus=`/`nohz_full=`/`rcu_nocbs=` kernel boot args for the benchmark cores via a GRUB drop-in (`/etc/default/grub.d/zz-bencher-isolation.cfg`, named to sort after provider drop-ins that overwrite `GRUB_CMDLINE_LINUX_DEFAULT`), then reboot the server and verify. This clears the runner preflight notice about missing isolation boot args. Idempotent: exits early if the cmdline already has the args (presence-only; it will not re-scope an existing CPU list, even with `--cpus`). The benchmark CPU list defaults to `1-(nproc-1)` (CPU 0 is housekeeping); override with `--cpus`.

```bash
cargo ops isolate <runner>
cargo ops isolate <runner> --cpus 1-5
```

## How It Works

1. `deploy` downloads the runner binary from the release of the runner's update channel, or from a devel CI artifact with `--run-id`, checks it against its published checksum, SSHes into the server, stops the existing service, copies the new binary, configures systemd, and starts the service.
2. Server SSH details, runner key, and host URL are resolved by merging `runners.json` with any CLI flags. CLI flags override the JSON file.
3. The runner key and host are written to a systemd drop-in at `/etc/systemd/system/bencher-runner.service.d/credentials.conf`.
