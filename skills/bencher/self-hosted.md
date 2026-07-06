# Self-Hosted Bencher

Run your own Bencher instance using Docker Compose via the CLI.

## Quick Start

```bash
bencher up --detach
```

This starts the API server (port 6610) and Console (port 3000) in the background.

Open `http://localhost:3000` to access the console and create your first account.

## Point the CLI at Your Instance

```bash
export BENCHER_HOST=http://localhost:6610
```

Or pass `--host http://localhost:6610` on each command.

All `bencher run` and resource commands respect `BENCHER_HOST`.

## Container Management

### Start

```bash
# Start all services in background
bencher up --detach

# Start only the API
bencher up api --detach

# Pin to a specific version
bencher up --tag v0.6.8

# Custom ports
bencher up --api-port 8080 --console-port 8081
```

### Logs

```bash
# All services
bencher logs

# API only
bencher logs api
```

### Stop

```bash
# Stop all
bencher down

# Stop only the console
bencher down console
```

## Configuration

### Environment Variables

Pass environment variables to containers:
```bash
bencher up --api-env SECRET_KEY=mykey --api-env DATABASE_URL=sqlite:///data/bencher.db
```

### Volumes

Mount host paths into containers:
```bash
bencher up --api-volume /host/data:/data
```

## Services

| Service | Default Port | Purpose |
|---------|-------------|---------|
| `api` | 6610 | REST API server |
| `console` | 3000 | Web UI |
| `all` | Both | Default: starts everything |

## Initial Setup

After `bencher up`:

1. Open `http://localhost:3000`
2. Create your admin account through the console
3. Create an organization and project
4. Generate a project API key (or user API key) for CLI/CI use

Or via CLI (the email is a positional argument and `--i-agree` is required):
```bash
bencher auth signup --name "Admin" --i-agree admin@example.com --host http://localhost:6610
```

## Full Documentation

See https://bencher.dev/docs/tutorial/self-hosted/ for the complete self-hosted tutorial
including production deployment, backups, and upgrades.
