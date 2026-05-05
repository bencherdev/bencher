# Bare Metal Benchmarks (Bencher Plus)

Run benchmarks on dedicated hardware with isolated Firecracker VMs,
eliminating noisy-neighbor effects from shared CI runners.

## Quick Start

The simplest bare metal run uses the built-in noise measurement tool:
```bash
bencher run --image alpine:latest "bencher noise"
```

This submits a job that runs `bencher noise` inside an Alpine container on
dedicated hardware and reports back environment noise metrics.

## Available Specs (Bencher Cloud)

See: https://bencher.dev/docs/explanation/testbeds/#--spec-spec

| Name | Slug | OS | Arch | Sandbox | CPU | Memory | Disk | Network |
|------|------|----|------|---------|-----|--------|------|---------|
| Intel v1 | `intel-v1` | Linux | x86_64 | Firecracker | 4 | 48.0 GiB | 128.0 GiB | No |

Specify with `--spec`:
```bash
bencher run --image my-bench:latest --spec intel-v1 "cargo bench"
```

## Workflow

1. Build an OCI image containing your benchmark suite
2. Push it to a container registry (Docker Hub, GHCR, etc.)
3. Submit with `bencher run --image <ref>`

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

```bash
docker build -t ghcr.io/myorg/my-bench:latest .
docker push ghcr.io/myorg/my-bench:latest
```

### Submit

```bash
bencher run \
  --project my-project \
  --branch main \
  --image ghcr.io/myorg/my-bench:latest \
  --spec intel-v1 \
  "cargo bench"
```

## Key Flags

| Flag | Purpose |
|------|---------|
| `--image <ref>` | OCI image reference (required for bare metal) |
| `--spec <slug>` | Hardware spec (default: platform-assigned) |
| `--entrypoint <cmd>` | Override container entrypoint |
| `--env KEY=VALUE` | Set environment variables (repeatable) |
| `--job-timeout <secs>` | Maximum execution time |
| `--job-poll-interval <secs>` | How often to check for completion |
| `--detach` | Submit without waiting for results |

## Fire-and-Forget

Submit the job and exit immediately (useful in CI where you check results later):
```bash
bencher run --image my-bench:latest --detach "cargo bench"
```

## Environment Variables

Pass secrets or config into the container:
```bash
bencher run \
  --image my-bench:latest \
  --env DATABASE_URL=postgres://... \
  --env FEATURE_FLAG=true \
  "pytest benchmarks/"
```

## Networking

Bare metal jobs on Bencher Cloud run without network access by default.
All dependencies must be baked into the image.
