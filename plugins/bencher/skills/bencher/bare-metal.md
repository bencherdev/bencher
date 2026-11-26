# Bare Metal Benchmarks (Bencher Plus)

Run benchmarks on dedicated hardware with isolated Firecracker VMs,
eliminating noisy-neighbor effects from shared CI runners.

## Quick Start

The simplest bare metal run uses the `bencher` CLI image and its built-in noise
measurement tool. `--image` runs an image from the Bencher registry, whose
repository is the project's slug, so push the CLI image there first:
```bash
docker pull --platform linux/amd64 ghcr.io/bencherdev/bencher:latest
docker tag ghcr.io/bencherdev/bencher:latest registry.bencher.dev/my-project:latest
docker push registry.bencher.dev/my-project:latest
bencher run --project my-project --image my-project:latest noise --format json
```

This submits a job that runs `bencher noise --format json` (the image's entrypoint
is `bencher`) on dedicated hardware and reports back environment noise metrics.
For a claimed project, log in to the registry before the push (see
[Build and Push](#build-and-push)).

## Available Specs (Bencher Cloud)

See: https://bencher.dev/docs/explanation/testbeds/#--spec-spec

| Name | Slug | OS | Arch | Sandbox | CPU | Memory | Disk | Network |
|------|------|----|------|---------|-----|--------|------|---------|
| Intel v1 | `intel-v1` | Linux | x86_64 | Firecracker | 4 | 48.0 GiB | 128.0 GiB | No |

Specify with `--spec`:
```bash
bencher run --project my-project --image my-project:latest --spec intel-v1 "cargo bench"
```

## Workflow

1. Build an OCI image containing your benchmark suite
2. Push it to the Bencher registry (`registry.bencher.dev/<project>`; on Bencher
   Self-Hosted, the API server's host, such as `localhost:6610/<project>`), the only
   registry `--image` pulls from
3. Submit with `bencher run --project <project> --image <ref>`

### Writing a Dockerfile

The image must contain everything needed to run benchmarks.
The command passed to `bencher run` executes inside the container.

Example for Rust (Criterion):
```dockerfile
FROM rust:1.87-slim
WORKDIR /app
COPY . .
RUN cargo build --release --benches
```

Example for Go:
```dockerfile
FROM golang:1.24
WORKDIR /app
COPY . .
RUN go build ./...
```

Example for Python (pytest-benchmark):
```dockerfile
FROM python:3.13-slim
WORKDIR /app
COPY . .
RUN pip install -e ".[bench]"
```

### Build and Push

A claimed project's repository needs a login first: the username is the project
slug and the password is an API key, read from `BENCHER_API_KEY`.
```bash
docker login registry.bencher.dev -u my-project --password-stdin <<< "$BENCHER_API_KEY"
docker build -t registry.bencher.dev/my-project:latest .
docker push registry.bencher.dev/my-project:latest
```

### Submit

```bash
bencher run \
  --project my-project \
  --branch main \
  --image my-project:latest \
  --spec intel-v1 \
  --adapter rust_criterion \
  "cargo bench"
```

Pass `--project` with `--image`, so the run reads the project's BMF version;
without it, the run declares version 1 (see `local-runs.md`).

## Key Flags

| Flag | Purpose |
|------|---------|
| `--image <ref>` | OCI image reference (required for bare metal) |
| `--spec <slug>` | Hardware spec (default: platform-assigned) |
| `--spec-reset` | Reset testbed spec (use with `--testbed`, conflicts with `--image`) |
| `--entrypoint <cmd>` | Override container entrypoint |
| `--env KEY=VALUE` | Set environment variables (repeatable) |
| `--job-timeout <secs>` | Maximum execution time |
| `--job-poll-interval <secs>` | How often to check for completion |
| `--detach` | Submit without waiting for results (conflicts with `--job-poll-interval`) |
| `--callback-url <url>` | `https` URL that Bencher sends one request to when the detached job finishes, only for a claimed project: a callback to exactly `https://api.github.com/repos/<OWNER>/<REPO>/dispatches` with an `Authorization: Bearer <token>` header and `"event_type": "bencher_run"` in its body goes out on any plan, any other callback needs a Bencher Plus plan, and otherwise the callback is skipped; use only a URL the user provides (requires `--image` and `--detach`, conflicts with `--github-actions`) |
| `--callback-header '<NAME: VALUE>'` | Callback request header, quoted as one argument; use only headers the user provides (repeatable, requires `--callback-url`) |
| `--callback-body '<json>'` | Callback JSON body, quoted as one argument: `{{ job.uuid }}`, `{{ job.status }}`, `{{ report.uuid }}`, `{{ project.uuid }}`, `{{ project.name }}`, and `{{ project.slug }}` are replaced anywhere inside a string, a string that is exactly `{{ report }}` becomes the whole report (once per body), and any other name in braces is refused; optional, and without it the body is the report (requires `--callback-url`) |
| `--ci-callback-token <token>` | With `--detach` and `--github-actions`, the token for the `repository_dispatch` the CLI composes in place of `--callback-*`; without it, the project's linked repository supplies the token (see `ci.md`) |
| `--job <uuid>` | Wait for a submitted job, then post its results (requires `--project`; `--job-timeout` is the wait budget; refuses report options, including from `BENCHER_BRANCH`, `BENCHER_TESTBED`, `BENCHER_ADAPTER`, and `BENCHER_CMD`) |

## Build Time and File Size Tracking

Build time and file size tracking work with bare metal runs:
```bash
bencher run --project my-project --image my-project:latest --build-time "cargo build --release"
bencher run --project my-project --image my-project:latest --file-size /app/target/release/my-binary
```

## Detached Runs

Submit the job and exit immediately (useful in CI where you check results later):
```bash
bencher run --project my-project --image my-project:latest --detach "cargo bench"
```

On GitHub Actions, `--detach` with `--github-actions` sends a `repository_dispatch` when the job
finishes, with a token Bencher mints for the project's linked repository or with
`--ci-callback-token`, so a second workflow can attach with `bencher run --job` and complete the
GitHub Check (see `ci.md`).

## Environment Variables

Pass non-secret config into the container:
```bash
bencher run \
  --project my-project \
  --image my-project:latest \
  --env BENCH_SIZE=large \
  --env FEATURE_FLAG=true \
  "pytest benchmarks/"
```

`--env` values are stored with the job, so do not pass secrets.

## Networking

Bare metal jobs on Bencher Cloud run without network access by default.
All dependencies must be baked into the image.

## Self-Hosted Runners

Beyond Bencher Cloud's managed hardware, you can operate your own bare metal runner
with the self-hosted `runner` binary (distinct from the `bencher runner` management
subcommands). It has two subcommands:

- `runner up`: long-running agent that polls for and executes Jobs
- `runner run`: pull an image and execute it once on the local host (for testing)

For registration, specs, sandboxing, and the full workflow, see the public docs:

- Bare Metal overview: https://bencher.dev/docs/explanation/bare-metal/
- Self-Hosted Runners: https://bencher.dev/docs/explanation/self-hosted-runners/
- `runner` CLI reference: https://bencher.dev/docs/reference/runner/
