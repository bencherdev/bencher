# Runner Ops

Operational tooling for managing bare metal runner servers. Invoked via `cargo ops` (alias for `cargo runner-ops`).

## Server Configuration

Servers are configured in `tasks/runner_ops/runners.json`. Each entry maps a runner name to its SSH connection details and runner key. The runner name is used as the first positional argument in all commands.

Optional per-runner fields:

- `"update_channel": "canary"` puts the runner on the canary update channel: it self-updates to the rolling canary build published on each `cloud` branch deploy, instead of waiting for versioned releases. Omit (or use `"stable"`) for release-only updates. The channel is written to the systemd drop-in as `BENCHER_UPDATE_CHANNEL` by `deploy` and `start`; both commands also accept an `--update-channel` flag that overrides the file.

## Common Operations

### Deploy a tagged release

To deploy a specific release version, find the CI run ID for the tag and pass it via `--run-id`:

```bash
gh run list --repo bencherdev/bencher --branch v0.X.Y --workflow ci.yml --status success --json databaseId -L 1
cargo ops deploy <runner> --run-id <run_id>
```

### Deploy latest from devel

```bash
cargo ops deploy <runner>
```

### Deploy a canary (cloud branch) build

Canary channel runners self-update automatically after each `cloud` push, so this is only needed to bootstrap a runner onto the channel or to force an immediate deploy:

```bash
gh run list --repo bencherdev/bencher --branch cloud --workflow ci.yml --status success --json databaseId -L 1
cargo ops deploy <runner> --run-id <run_id> --update-channel canary
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

1. `deploy` downloads the runner binary from a GitHub Actions CI artifact, SSHes into the server, stops the existing service, copies the new binary, configures systemd, and starts the service.
2. Server SSH details, runner key, and host URL are resolved by merging `runners.json` with any CLI flags. CLI flags override the JSON file.
3. The runner key and host are written to a systemd drop-in at `/etc/systemd/system/bencher-runner.service.d/credentials.conf`.
