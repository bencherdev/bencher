# Runner Ops

Operational tooling for managing bare metal runner servers. Invoked via `cargo ops` (alias for `cargo runner-ops`).

## Server Configuration

Servers are configured in `tasks/runner_ops/runners.json`. Each entry maps a runner name to its SSH connection details and runner key. The runner name is used as the first positional argument in all commands.

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

## How It Works

1. `deploy` downloads the runner binary from a GitHub Actions CI artifact, SSHes into the server, stops the existing service, copies the new binary, configures systemd, and starts the service.
2. Server SSH details, runner key, and host URL are resolved by merging `runners.json` with any CLI flags. CLI flags override the JSON file.
3. The runner key and host are written to a systemd drop-in at `/etc/systemd/system/bencher-runner.service.d/credentials.conf`.
