# Bare Metal Benchmark Runner Design

This document outlines the design for Bencher's bare metal benchmark runner system.

## Overview

A pull-based runner agent architecture where runners claim jobs from the API, execute benchmarks on bare metal, and report results back. Designed to work for both Bencher Cloud (SaaS) and self-hosted deployments.

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Bencher CLI    │     │  Bencher API    │     │  Runner Agent   │
│  (submits jobs) │────▶│ api.bencher.dev │◀────│  (polls/claims) │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                      │                        │
        │               ┌──────┴──────┐                 │
        │               ▼             ▼                 │
        │        ┌───────────┐ ┌─────────────┐          │
        │        │  SQLite   │ │ OCI Registry│◀─────────┤
        │        │ (jobs tbl)│ │ registry.   │  (pulls  │
        │        └───────────┘ │ bencher.dev │  images) │
        │                      └─────────────┘          │
        │                             ▲                 ▼
        └─────────────────────────────┘          ┌─────────────┐
                (pushes images)                  │  Bare Metal │
                                                 │   Machine   │
                                                 └─────────────┘
```

**Flow:**
1. CLI pushes benchmark OCI image to `registry.bencher.dev/{project}/...`
2. CLI submits job to API with image digest
3. Runner claims job from API, receives `JsonJobConfig` and spec details
4. Runner pulls image from registry using project-scoped OCI auth
5. Runner executes image in isolated VM, reports results via WebSocket

## Runner States

| State      | Network Behavior                       | Notes                                |
| ---------- | -------------------------------------- | ------------------------------------ |
| **Idle**   | Long-poll for jobs                     | Can be noisy, responsiveness matters |
| **Active** | Minimal heartbeat on separate CPU core | Benchmark cores completely isolated  |

## Runner Scope

Runners are **server-scoped** - they can execute jobs from ANY project on the server. This applies to both self-hosted and cloud deployments.

- **Self-hosted**: Runners serve all projects on that Bencher instance
- **Cloud**: Bencher-provided runners serve all organizations/projects on Bencher Cloud

This is the shared infrastructure model (similar to GitHub-hosted runners). A pool of bare metal machines serves the entire platform.

## Data Model

### Runner

Represents a registered bare metal machine capable of executing benchmark jobs.

| Field            | Description                                          |
| ---------------- | ---------------------------------------------------- |
| uuid             | Runner's self-generated ID                           |
| name / slug      | Human-readable name and URL-friendly slug            |
| token_hash       | SHA-256 hash of runner token (token itself never stored) |
| state            | `offline`, `idle`, or `running`                      |
| archived         | Soft delete timestamp                                |
| last_heartbeat   | Last heartbeat received from this runner             |

### Spec

Represents a hardware specification that runners can be associated with and jobs can target.

| Field            | Description                                          |
| ---------------- | ---------------------------------------------------- |
| uuid             | Unique identifier                                    |
| cpu              | Number of CPUs                                       |
| memory           | Memory size in bytes                                 |
| disk             | Disk size in bytes                                   |
| network          | Whether network access is available                  |
| archived         | Soft delete timestamp                                |
| created          | Creation timestamp                                   |
| modified         | Last modification timestamp                          |

### Runner-Spec

Many-to-many association between runners and specs. A runner can support multiple specs, and a spec can be supported by multiple runners. When a job targets a spec, only runners associated with that spec are eligible to claim it.

| Field            | Description                                          |
| ---------------- | ---------------------------------------------------- |
| runner_id        | FK to runner                                         |
| spec_id          | FK to spec                                           |

### Job

Represents a benchmark execution request linked to a report.

| Field              | Description                                          |
| ------------------ | ---------------------------------------------------- |
| report_id          | Reference to the parent report                       |
| organization_id    | Owning organization (for concurrency limits)         |
| source_ip          | Submitter IP (for unclaimed project rate limiting)   |
| priority           | Scheduling priority (0=unclaimed, 100=free, 200=team, 300=enterprise) |
| status             | Job lifecycle state (see Job State Machine)          |
| spec_id            | FK to spec (hardware requirements for this job)      |
| config             | `JsonJobConfig` — execution details the runner needs |
| runner_id          | Runner that claimed this job                         |
| claimed / started / completed | Lifecycle timestamps                      |
| last_heartbeat     | Last heartbeat for this specific job                 |
| last_billed_minute | Minutes billed so far (prevents double-counting)     |
| exit_code          | Process exit code from the benchmark                 |

### Job Status

`Pending` (0), `Claimed` (1), `Running` (2), `Completed` (3), `Failed` (4), `Canceled` (5)

## Job Config Structure

The job config is designed to minimize data sent to runners, reducing leakage risk. Runners only receive what's necessary to pull and execute an OCI image. Hardware resource constraints (cpu, memory, disk, network) are defined in the associated spec, not in the config.

| Field      | Description                                                |
| ---------- | ---------------------------------------------------------- |
| registry   | Registry URL (e.g., `https://registry.bencher.dev`)        |
| project    | Project UUID for OCI authentication scoping                |
| digest     | Immutable image digest (e.g., `sha256:abc123...`)          |
| entrypoint | Optional entrypoint override (like Docker ENTRYPOINT)      |
| cmd        | Optional command override (like Docker CMD)                |
| env        | Optional environment variables                             |
| timeout    | Maximum execution time in seconds                          |
| output     | List of file paths to read results from after execution    |

**Design principles:**
- **Minimal information**: Runner doesn't know repo URL, branch, commit, or benchmark commands directly
- **Immutable reference**: Digest (not tag) ensures the image can't change between job creation and execution
- **Isolated execution**: VM resources (cpu, memory, disk) and network access are defined in the spec table
- **OCI-based**: All benchmark code is packaged in an OCI image, pulled from the Bencher registry

## API Endpoints

### Runner Management (Server Scoped)

Requires server admin permissions.

| Method | Endpoint                     | Description                                    |
| ------ | ---------------------------- | ---------------------------------------------- |
| POST   | `/v0/runners`                | Create runner, returns token                   |
| GET    | `/v0/runners`                | List runners                                   |
| GET    | `/v0/runners/{runner}`       | Get runner details                             |
| PATCH  | `/v0/runners/{runner}`       | Update runner (name, archived)                 |
| POST   | `/v0/runners/{runner}/token` | Generate new token (invalidates old)           |

### Spec Management (Server Scoped)

Requires server admin permissions.

| Method | Endpoint                     | Description                                    |
| ------ | ---------------------------- | ---------------------------------------------- |
| GET    | `/v0/specs`                  | List specs                                     |
| POST   | `/v0/specs`                  | Create spec                                    |
| GET    | `/v0/specs/{spec}`           | Get spec details                               |
| PATCH  | `/v0/specs/{spec}`           | Update spec (archive)                          |
| DELETE | `/v0/specs/{spec}`           | Delete spec                                    |

### Runner-Spec Association (Server Scoped)

Requires server admin permissions.

| Method | Endpoint                              | Description                            |
| ------ | ------------------------------------- | -------------------------------------- |
| GET    | `/v0/runners/{runner}/specs`          | List specs for a runner                |
| POST   | `/v0/runners/{runner}/specs`          | Add spec to a runner                   |
| DELETE | `/v0/runners/{runner}/specs/{spec}`   | Remove spec from a runner              |

### Job Management (Project Scoped)

Jobs belong to projects, but can be executed by any runner on the server.

| Method | Endpoint                            | Description               |
| ------ | ----------------------------------- | ------------------------- |
| GET    | `/v0/projects/{project}/jobs`       | List jobs (filterable)    |
| GET    | `/v0/projects/{project}/jobs/{job}` | Get job details + results |

### Runner Agent Endpoints

Authenticated via runner token (`Authorization: Bearer bencher_runner_<token>`)

| Method    | Endpoint                                  | Description                                            |
| --------- | ----------------------------------------- | ------------------------------------------------------ |
| POST      | `/v0/runners/{runner}/jobs`               | Long-poll to claim a job (from any accessible project) |
| WebSocket | `/v0/runners/{runner}/jobs/{job}/channel` | Heartbeat and status updates during job execution      |

## Claim Endpoint Behavior

1. Applies per-runner rate limiting to prevent abuse of long-polling
2. Filters pending jobs to only those whose `spec_id` matches one of the runner's associated specs
3. Finds matching pending jobs ordered by `(priority DESC, created ASC)`
4. Atomically updates job status to `claimed`, sets `runner_id` and `claimed` timestamp
5. If no matching jobs, holds connection open until timeout or job arrives
6. Returns job with config or `None` on timeout

## WebSocket Job Execution Channel

WebSocket connection for heartbeat and job status updates. Established after claiming a job, before benchmark execution begins.

**Authentication:** Runner token passed via `Sec-WebSocket-Protocol: bearer.<token>` header.

### Runner to Server Messages

| Event       | Description                                 | Payload                            |
| ----------- | ------------------------------------------- | ---------------------------------- |
| `running`   | Job setup complete, benchmark starting      | —                                  |
| `heartbeat` | Periodic liveness signal (~1/sec)           | —                                  |
| `completed` | Benchmark completed successfully            | `exit_code`, `stdout`, `stderr`, optional `output` (file path → contents map) |
| `failed`    | Benchmark failed                            | optional `exit_code`, `error`      |
| `cancelled` | Acknowledge cancellation from server        | —                                  |

### Server to Runner Messages

| Event    | Description                              |
| -------- | ---------------------------------------- |
| `ack`    | Acknowledge received message             |
| `cancel` | Job was canceled, stop execution         |

### Connection Flow

```
Runner                              Server
  │                                    │
  ├──[WS] Connect with runner token ──►│  Validate token, verify job ownership
  │◄─────────────── Connected ─────────┤
  │                                    │
  ├──── { "event": "running" } ────────►│  Mark job running, start billing clock
  │◄──── { "event": "ack" } ────────────┤
  │                                    │
  │  ┌─── benchmark executes ───┐      │
  ├──┼─ { "event": "heartbeat" } ──────►│  Update last_heartbeat, bill if minute elapsed
  │◄─┼── { "event": "ack" } ───────────┤  (or { "event": "cancel" } if user canceled)
  │  └──────────────────────────┘      │
  │                                    │
  ├──── { "event": "completed", ... } ─►│  Mark job completed, stop billing
  │◄──── { "event": "ack" } ────────────┤
  │                                    │
  ├──[WS Close] ──────────────────────►│
```

### Cancellation Flow

```
Runner                              Server
  │                                    │
  ├──── { "event": "heartbeat" } ──────►│  Detects job was canceled by user
  │◄──── { "event": "cancel" } ─────────┤
  │                                    │
  │  (runner stops benchmark)          │
  │                                    │
  ├──── { "event": "cancelled" } ──────►│  Mark job canceled (if not already)
  │◄──── { "event": "ack" } ────────────┤
  │                                    │
  ├──[WS Close] ──────────────────────►│
```

**Advantages over REST polling:**
- ~20x less network overhead per heartbeat (~50 bytes vs ~700 bytes)
- Immediate cancellation notification (server push)
- Connection loss triggers per-job timeout recovery (no periodic reaper needed)
- Reconnection supported: runner can reconnect to a `Running` job after a transient disconnect
- Billing based on connection duration, not polling

## Timeout-Based Job Recovery

Instead of a periodic reaper, stale jobs are recovered via per-job timeout tasks. This provides faster, more precise recovery without polling overhead.

**Two complementary mechanisms:**

1. **Inline WS timeout** — While the WebSocket connection is open, a read timeout detects a "connected but silent" runner. On timeout, the job is marked `Failed` immediately within the WS loop.

2. **Spawned disconnect timeout** — When a WebSocket disconnects and the job is still in-flight (non-terminal), a background task sleeps for the heartbeat timeout. After waking, it checks:
   - If the job reached a terminal state: do nothing (finished normally).
   - If `last_heartbeat` is recent (within the timeout window): the runner reconnected — schedule another timeout for the remaining duration.
   - Otherwise: mark the job as `Failed`.

**Startup recovery:** On server startup, all `Claimed` or `Running` jobs are queried and a timeout task is spawned for each, recovering jobs that were in-flight when the server previously shut down.

**Heartbeat timeout is configurable** (default: 90 seconds in production, 5 seconds in tests).

### WebSocket Reconnection

If a runner disconnects and reconnects to a `Running` job, the WebSocket channel accepts the connection. Sending a `Running` message on a job that is already `Running` is idempotent — it updates `last_heartbeat` without changing the status or `started` timestamp. This cancels any pending disconnect-timeout task via the `last_heartbeat` freshness check.

## Job State Machine

```
pending ───▶ claimed ───▶ running ───▶ completed
   │            │            │
   │            │            ├────────▶ failed
   │            │            │
   └────────────┴────────────┴────────▶ canceled
```

**Transitions:**
| From    | To        | Trigger                           |
| ------- | --------- | --------------------------------- |
| pending | claimed   | Runner claims job                 |
| pending | canceled  | User cancels                      |
| claimed | running   | Runner sends `running` event      |
| claimed | failed    | Runner fails during setup         |
| claimed | canceled  | User cancels                      |
| running | completed | Runner sends `completed` event    |
| running | failed    | Runner sends `failed` event or timeout |
| running | canceled  | User cancels                      |

**Terminal states:** completed, failed, canceled (no transitions out)

## Job Submission Flow

```
User submits job via CLI or API
                │
                ▼
1. API creates Report (with project, branch, testbed)
                │
                ▼
2. API creates Job linked to Report
   - Sets priority based on org's plan
   - Status = pending
                │
                ▼
3. Job waits in queue for runner
```

## Runner Execution Flow

```
1. Idle: Long-poll POST /v0/runners/{runner}/jobs
                │
                ▼ (job received with JsonJobConfig)
2. Job "claimed" implicitly via claim response
                │
                ▼
3. Open WebSocket: /v0/runners/{runner}/jobs/{job}/channel
                │
                ▼
4. Authenticate to OCI registry using runner token + project UUID
   Pull image from registry.bencher.dev/{project}/images@{digest}
                │
                ▼
5. Create VM with spec constraints (cpu, memory, disk, network from spec)
   Load OCI image into VM
                │
                ▼
6. Send { "event": "running" } over WebSocket
   - Heartbeat thread starts (pinned to separate CPU core)
   - Main benchmark cores isolated
                │
                ▼
7. Execute image (with optional entrypoint/cmd/env overrides)
   - Heartbeat messages sent ~1/sec over WebSocket
   - Server may send { "event": "cancel" } at any time
   - On cancel: stop execution, send { "event": "cancelled" }, close
   - Timeout enforced per job config
                │
                ▼
8. Send { "event": "completed", ... } or { "event": "failed", ... }
   - Results attached to the Report
                │
                ▼
9. Destroy VM, close WebSocket, return to idle
```

## Job Scheduling & Priority

### Priority Tiers

Jobs are queued with priority based on the submitting organization's plan. Priority is set at job creation time and does not change if the org upgrades/downgrades.

| Plan Tier  | Priority | Concurrency Limit        | Description                        |
| ---------- | -------- | ------------------------ | ---------------------------------- |
| Enterprise | 300      | Unlimited                | Highest priority, no limits        |
| Team       | 200      | Unlimited                | High priority, no limits           |
| Free       | 100      | 1 per organization       | Lower priority, org-level limiting |
| Unclaimed  | 0        | 1 per source IP          | Lowest priority, IP-based limiting |

**Unclaimed** means an organization with no members (anonymous/demo usage). Source IP rate limiting prevents abuse.

### Claim Algorithm

The claim endpoint atomically finds the highest-priority eligible job while respecting concurrency limits:

1. **Enterprise/Team** (priority >= 200): No concurrency limit — always eligible.
2. **Free** (priority 100-199): Eligible only if no other `Running` job exists for the same organization.
3. **Unclaimed** (priority < 100): Eligible only if no other `Running` job exists for the same source IP.

Jobs are ordered by `(priority DESC, created ASC)` — highest priority first, FIFO within the same tier. SQLite's serialized write transactions ensure atomicity without explicit row locking.

### OTEL Metrics

Queue time is tracked per priority tier to monitor starvation. A `job.queue.duration` histogram (in seconds) is recorded when a job transitions to `Running`, with a `priority.tier` attribute indicating the tier (enterprise, team, free, unclaimed).

### Usage-Based Billing

Usage is tracked per-minute via Stripe's usage-based pricing. Heartbeats serve double duty:

1. **Liveness check** — Confirms runner is still executing the job
2. **Billing increment** — Reports usage to Stripe

The API tracks which minutes have been billed via `last_billed_minute` on the job to avoid double-counting if heartbeats arrive early.

On each heartbeat:
1. Update `last_heartbeat` on job and runner
2. Calculate `elapsed_minutes = (now - started) / 60`
3. If `elapsed_minutes > last_billed_minute`, bill the difference to Stripe
4. Update `last_billed_minute = elapsed_minutes`

## Authentication

### Token Format

Runner tokens use random bytes with a `bencher_runner_` prefix (not JWTs). The token is shown exactly once at creation and cannot be retrieved later. Only the SHA-256 hash is stored in the database, so a database breach does not expose usable tokens.

### Token Validation

1. Verify the `bencher_runner_` prefix
2. Hash the provided token with SHA-256
3. Look up the runner by hash (excluding archived runners)

### Token Rotation

If a token is compromised:
1. Rotate token (`POST /v0/runners/{runner}/token`) — old token is invalidated immediately
2. Update runner agent with new token

### Request Header

```
Authorization: Bearer bencher_runner_<token>
```

This token is scoped to:
- Only the runner agent endpoints (`/v0/runners/{runner}/jobs[/{job}[/channel]]`)
- Can claim jobs from any project on the server
- Can only perform operations on jobs claimed by this runner

## Open Questions

- **Result storage**: Store in job table or separate results table linked to existing perf tables?
- **Output storage**: How to persist the file path → contents map from `completed` messages? Options: inline in job table, separate table, or external blob storage.
- **Retry policy**: Auto-retry failed jobs? How many times?
- **OCI auth for runners**: How does runner authenticate to registry? Options: (a) runner token directly, (b) exchange runner token for short-lived OCI token via API, (c) job claim response includes OCI token.

## Implementation Phases

1. **Phase 1**: Runner registration & heartbeat — Runners can connect and stay alive
2. **Phase 2**: Job queue & claiming — Basic job distribution
3. **Phase 3**: Execution & result reporting — Actually run benchmarks
4. **Phase 4**: Console UI — Manage runners, view job history
