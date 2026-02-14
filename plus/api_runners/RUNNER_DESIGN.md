# Bare Metal Benchmark Runner Design

This document outlines the design for Bencher's bare metal benchmark runner system.

## Overview

A pull-based runner agent architecture where runners claim jobs from the API, execute benchmarks on bare metal (with Firecracker microVM isolation), and report results back. Designed to work for both Bencher Cloud (SaaS) and self-hosted deployments.

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Bencher CLI    │     │  Bencher API    │     │  Runner Agent   │
│  (submits runs) │────▶│ api.bencher.dev │◀────│  (polls/claims) │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                      │                        │
        │               ┌──────┴──────┐                 │
        │               ▼             ▼                 │
        │        ┌───────────┐ ┌─────────────────┐      │
        │        │  SQLite   │ │  OCI Registry   │◀─────┤
        │        │ (jobs tbl)│ │  registry.      │(pulls│
        │        └───────────┘ │  bencher.dev    │images)│
        │                      └─────────────────┘      │
        │                             ▲                 ▼
        └─────────────────────────────┘          ┌─────────────┐
                (pushes images)                  │  Bare Metal │
                                                 │   Machine   │
                                                 │ (Firecracker│
                                                 │   microVM)  │
                                                 └─────────────┘
```

**OCI Registry:** Images are stored at `registry.bencher.dev`. The `{name}` in OCI paths is a `ProjectResourceId` (project UUID or slug). Job output is also stored in the same storage backend under `{project}/output/v0/jobs/{job}`.

**Flow:**
1. CLI pushes benchmark OCI image to `registry.bencher.dev/{project}:{tag}`
2. CLI submits run to API via `POST /v0/run` with `image` digest and `spec`
3. API creates Report (pending results) and Job linked to that Report
4. Runner claims job from API, receives `JsonJobConfig` with short-lived OCI token and spec details
5. Runner pulls image from registry using project-scoped OCI token
6. Runner executes image in Firecracker microVM, reports results via WebSocket
7. Server runs adapter on job output to parse results into the Report

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

| Field            | Type                | Description                                          |
| ---------------- | ------------------- | ---------------------------------------------------- |
| id               | `RunnerId`          | Internal primary key                                 |
| uuid             | `RunnerUuid`        | Runner's unique identifier                           |
| name             | `ResourceName`      | Human-readable name                                  |
| slug             | `RunnerSlug`        | URL-friendly slug (unique, auto-generated from name) |
| token_hash       | `TokenHash`         | SHA-256 hash of runner token (64-char hex, token itself never stored) |
| last_heartbeat   | `Option<DateTime>`  | Last heartbeat received from this runner             |
| created          | `DateTime`          | Creation timestamp                                   |
| modified         | `DateTime`          | Last modification timestamp                          |
| archived         | `Option<DateTime>`  | Soft delete timestamp                                |

### Spec

Represents a hardware specification that runners can be associated with and jobs can target.

| Field            | Type                | Description                                          |
| ---------------- | ------------------- | ---------------------------------------------------- |
| id               | `SpecId`            | Internal primary key                                 |
| uuid             | `SpecUuid`          | Unique identifier                                    |
| name             | `ResourceName`      | Human-readable name                                  |
| slug             | `SpecSlug`          | URL-friendly slug (unique)                           |
| architecture     | `Architecture`      | CPU architecture (`x86_64`, `aarch64`, `riscv64`)    |
| cpu              | `Cpu`               | Number of CPUs (u32, 1 to i32::MAX)                  |
| memory           | `Memory`            | Memory size in bytes (u64, 1 to i64::MAX)            |
| disk             | `Disk`              | Disk size in bytes (u64, 1 to i64::MAX)              |
| network          | `bool`              | Whether VM has network access (default: false)       |
| created          | `DateTime`          | Creation timestamp                                   |
| modified         | `DateTime`          | Last modification timestamp                          |
| archived         | `Option<DateTime>`  | Soft delete timestamp                                |

### Runner-Spec

Many-to-many association between runners and specs. A runner can support multiple specs, and a spec can be supported by multiple runners. When a job targets a spec, only runners associated with that spec are eligible to claim it.

| Field            | Type                | Description                                          |
| ---------------- | ------------------- | ---------------------------------------------------- |
| id               | `RunnerSpecId`      | Internal primary key                                 |
| runner_id        | `RunnerId`          | FK to runner (CASCADE on delete)                     |
| spec_id          | `SpecId`            | FK to spec (RESTRICT on delete)                      |

Unique constraint on `(runner_id, spec_id)`.

### Job

Represents a benchmark execution request linked to a report.

| Field              | Type                | Description                                          |
| ------------------ | ------------------- | ---------------------------------------------------- |
| id                 | `JobId`             | Internal primary key                                 |
| uuid               | `JobUuid`           | Unique identifier                                    |
| report_id          | `ReportId`          | FK to report (CASCADE on delete)                     |
| organization_id    | `OrganizationId`    | FK to organization (CASCADE), for concurrency limits |
| source_ip          | `SourceIp`          | Submitter IP (validated IpAddr, for unclaimed rate limiting) |
| spec_id            | `SpecId`            | FK to spec (RESTRICT on delete)                      |
| config             | `JsonJobConfig`     | Execution details (JSON serialized in TEXT column)   |
| timeout            | `Timeout`           | Maximum execution time in seconds (u32, default: 3600) |
| priority           | `JobPriority`       | Scheduling priority (0/100/200/300)                  |
| status             | `JobStatus`         | Job lifecycle state (integer enum 0-5)               |
| runner_id          | `Option<RunnerId>`  | FK to runner (RESTRICT), set on claim                |
| claimed            | `Option<DateTime>`  | When runner claimed the job                          |
| started            | `Option<DateTime>`  | When benchmark execution began                       |
| completed          | `Option<DateTime>`  | When job reached terminal state                      |
| last_heartbeat     | `Option<DateTime>`  | Last heartbeat for this job                          |
| last_billed_minute | `Option<i32>`       | Minutes billed so far (prevents double-counting)     |
| exit_code          | `Option<i32>`       | Process exit code from the benchmark                 |
| created            | `DateTime`          | Creation timestamp                                   |
| modified           | `DateTime`          | Last modification timestamp                          |

**Database indexes:**
- `index_job_pending` — on `(status, priority DESC, created ASC)` WHERE status = 0
- `index_job_org_in_flight` — on `(organization_id)` WHERE status IN (1, 2)
- `index_job_source_ip_in_flight` — on `(source_ip)` WHERE status IN (1, 2)
- `index_job_in_flight` — on `(status)` WHERE status IN (1, 2)
- `index_job_runner_id` — on `(runner_id)` WHERE runner_id IS NOT NULL
- `index_job_spec_id` — on `(spec_id)`
- `index_job_report_id` — on `(report_id)`

### Job Status

| Value | Name       | Description                                      |
| ----- | ---------- | ------------------------------------------------ |
| 0     | `Pending`  | Waiting for a runner to claim                    |
| 1     | `Claimed`  | Runner claimed but hasn't started execution      |
| 2     | `Running`  | Benchmark is executing                           |
| 3     | `Completed`| Finished successfully                            |
| 4     | `Failed`   | Failed (runner error, setup failure, or timeout)  |
| 5     | `Canceled` | Canceled by user or hard timeout exceeded        |

## Job Config Structure

The job config is designed to minimize data sent to runners, reducing leakage risk. Runners only receive what's necessary to pull and execute an OCI image. Hardware resource constraints (cpu, memory, disk, network) are defined in the associated spec, not in the config.

| Field       | Type                           | Description                                          |
| ----------- | ------------------------------ | ---------------------------------------------------- |
| registry    | `Url`                          | Registry URL (e.g., `https://registry.bencher.dev`)  |
| project     | `ProjectUuid`                  | Project UUID for OCI authentication scoping          |
| digest      | `ImageDigest`                  | Immutable image digest (e.g., `sha256:abc123...`)    |
| entrypoint  | `Option<Vec<String>>`          | Entrypoint override (like Docker ENTRYPOINT)         |
| cmd         | `Option<Vec<String>>`          | Command override (like Docker CMD)                   |
| env         | `Option<HashMap<String, String>>` | Environment variables passed to the container     |
| timeout     | `Timeout`                      | Maximum execution time in seconds                    |
| file_paths  | `Option<Vec<Utf8PathBuf>>`     | File paths to read from VM after execution           |

**Design principles:**
- **Minimal information**: Runner doesn't know repo URL, branch, commit, or benchmark commands directly
- **Immutable reference**: Digest (not tag) ensures the image can't change between job creation and execution
- **Isolated execution**: VM resources (cpu, memory, disk) and network access are defined in the spec table
- **OCI-based**: All benchmark code is packaged in an OCI image, pulled from the Bencher registry

## Job Submission via `/v0/run`

The existing `POST /v0/run` endpoint is extended with optional fields for runner-based execution:

| Field       | Type                           | Required | Description                                    |
| ----------- | ------------------------------ | -------- | ---------------------------------------------- |
| `image`     | `ImageDigest`                  | No       | OCI image digest for runner execution          |
| `spec`      | `SpecResourceId`               | No       | Target hardware spec (UUID or slug)            |
| `results`   | `Vec<String>`                  | No*      | BMF results (existing field, now optional)      |

\* `results` is required for direct submission (no runner). When `image` and `spec` are provided, `results` is omitted and the runner produces output that the server parses.

**Submission flow:**

```
CLI calls POST /v0/run with { image, spec, branch, testbed, ... }
                │
                ▼
1. API creates Report (project, branch, testbed, adapter)
   - Report has no results yet (pending runner output)
                │
                ▼
2. API creates Job linked to Report
   - Builds JsonJobConfig from image digest + registry URL
   - Sets priority based on org's plan tier
   - Status = Pending
                │
                ▼
3. Job waits in queue for a runner with matching spec
```

**Result processing (server-side):**

When a runner completes a job, the server runs the configured adapter on the job output (stdout/stderr/file contents) to parse benchmark results into the Report. This means:
- The adapter is specified in the run's `settings` (same as direct submission)
- Adapter parsing happens on the API server, not the runner
- The runner is a dumb executor — it doesn't know about adapters, metrics, or thresholds
- If output parsing fails, the Report records the error but the job itself is still `Completed`

## API Endpoints

### Runner Management (Server Scoped)

Requires server admin permissions.

| Method | Endpoint                     | Description                                    |
| ------ | ---------------------------- | ---------------------------------------------- |
| POST   | `/v0/runners`                | Create runner, returns token (shown once)       |
| GET    | `/v0/runners`                | List runners (filterable by name, search, archived) |
| GET    | `/v0/runners/{runner}`       | Get runner details (by UUID or slug)            |
| PATCH  | `/v0/runners/{runner}`       | Update runner (name, slug, archived)            |
| POST   | `/v0/runners/{runner}/token` | Rotate token (invalidates old immediately)      |

### Spec Management (Server Scoped)

Requires server admin permissions.

| Method | Endpoint                     | Description                                    |
| ------ | ---------------------------- | ---------------------------------------------- |
| GET    | `/v0/specs`                  | List specs                                     |
| POST   | `/v0/specs`                  | Create spec                                    |
| GET    | `/v0/specs/{spec}`           | Get spec details                               |
| PATCH  | `/v0/specs/{spec}`           | Update spec (name, slug, archive)              |
| DELETE | `/v0/specs/{spec}`           | Delete spec (RESTRICT if referenced by jobs)   |

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
| GET    | `/v0/projects/{project}/jobs/{job}` | Get job details + output  |

### Runner Agent Endpoints

Authenticated via runner token (`Authorization: Bearer bencher_runner_<token>`)

| Method    | Endpoint                                  | Description                                            |
| --------- | ----------------------------------------- | ------------------------------------------------------ |
| POST      | `/v0/runners/{runner}/jobs`               | Long-poll to claim a job (from any accessible project) |
| PATCH     | `/v0/runners/{runner}/jobs/{job}`         | Update job status (running, completed, failed)         |
| WebSocket | `/v0/runners/{runner}/jobs/{job}/channel` | Heartbeat and status updates during job execution      |

## Claim Endpoint Behavior

1. Applies per-runner rate limiting to prevent abuse of long-polling
2. Filters pending jobs to only those whose `spec_id` matches one of the runner's associated specs
3. Finds matching pending jobs ordered by `(priority DESC, created ASC, id ASC)`
4. Uses a single write lock (`write_conn!`) to atomically:
   - Update job status to `Claimed`
   - Set `runner_id`, `claimed` timestamp, and `last_heartbeat`
5. If no matching jobs, polls every 1 second until `poll_timeout` (default 30s, max 600s) or job arrives
6. Returns `Option<JsonJob>` — job with config and short-lived OCI token if claimed, `None` on timeout
7. Records OTel metrics: queue duration histogram and claim counter

## OCI Authentication for Runners

> **TODO**: The OCI token is not yet included in the claim response. `JsonJobConfig` currently has `registry`, `project`, and `digest` but no token field. This needs to be implemented.

The job claim response will include a **short-lived, project-scoped OCI token** (as a new `oci_token` field on `JsonJobConfig` or alongside it in the claim response). This token:
- Is scoped to `Pull` access for the specific project's images only
- Has a short TTL (minutes, not days)
- Uses the `Oci` JWT audience (separate from user/server audiences), which already exists in the OCI auth system

This minimizes the blast radius of a compromised runner: even with the token, the runner can only pull images from the one project for the claimed job, and only for a short window.

## WebSocket Job Execution Channel

WebSocket connection for heartbeat and job status updates. Established after claiming a job, before benchmark execution begins.

**Authentication:** Runner token via `Authorization: Bearer bencher_runner_<token>` header.

**WebSocket limits:** Both `max_message_size` and `max_frame_size` are configured from `request_body_max_bytes` on the server context. This bounds the size of any single message (including `completed` payloads with stdout/stderr/output). Messages exceeding this limit are rejected at the WebSocket protocol level.

### Runner to Server Messages

| Event       | Description                                 | Payload                            |
| ----------- | ------------------------------------------- | ---------------------------------- |
| `running`   | Job setup complete, benchmark starting      | —                                  |
| `heartbeat` | Periodic liveness signal (~1/sec)           | —                                  |
| `completed` | Benchmark completed successfully            | `exit_code`, optional `stdout`, `stderr`, `output` (file path → contents map) |
| `failed`    | Benchmark failed                            | `error` (required), optional `exit_code`, `stdout`, `stderr` |
| `canceled`  | Acknowledge cancellation from server        | —                                  |

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
  │◄─────────────── Connected ─────────┤  (only Claimed or Running jobs accepted)
  │                                    │
  ├──── { "event": "running" } ────────►│  Mark job running, start billing clock
  │◄──── { "event": "ack" } ────────────┤
  │                                    │
  │  ┌─── benchmark executes ───┐      │
  ├──┼─ { "event": "heartbeat" } ──────►│  Update last_heartbeat, check timeout, bill if minute elapsed
  │◄─┼── { "event": "ack" } ───────────┤  (or { "event": "cancel" } if user canceled or timeout exceeded)
  │  └──────────────────────────┘      │
  │                                    │
  ├──── { "event": "completed", ... } ─►│  Mark job completed, store output in OCI storage
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
  ├──── { "event": "canceled" } ───────►│  Mark job canceled (if not already)
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

## Timeout & Recovery

Three complementary timeout mechanisms ensure jobs never get stuck:

### 1. Heartbeat Timeout (WebSocket read timeout)

While the WebSocket connection is open, a read timeout detects a "connected but silent" runner. Only valid protocol messages (`Running`, `Heartbeat`, `Completed`, `Failed`, `Canceled`) reset the timer — invalid JSON, ping/pong frames, and binary messages do not.

On timeout, the server reads the job and decides:
- If `started` exists and `elapsed > timeout + job_timeout_grace_period`: mark as `Canceled` (ran too long)
- Otherwise: mark as `Failed` (lost contact with runner)

**Heartbeat timeout is configurable** (default: configurable on `ApiContext`, 5 seconds in tests).

### 2. Hard Job Timeout (wall-clock enforcement)

The server enforces a hard maximum execution duration server-side, independent of runner behavior. This prevents a compromised or buggy runner from running indefinitely by sending heartbeats.

**Enforced in two places:**
- **During heartbeat handling**: Each heartbeat checks `elapsed = now - started`. If `elapsed > timeout + job_timeout_grace_period`, the job is marked `Canceled` and the runner receives a `Cancel` message.
- **During spawned timeout tasks**: Background tasks spawned on disconnect also check the hard timeout using `check_job_timeout()`.

The `job_timeout_grace_period` is a server-level `Duration` configured on `ApiContext`, allowing a buffer for VM teardown and result transmission.

### 3. Spawned Disconnect Timeout

When a WebSocket disconnects and the job is still in-flight (non-terminal), a background task is spawned via `spawn_heartbeat_timeout()`:
1. Sleep for the heartbeat timeout duration.
2. On wake, read the job state:
   - If terminal: do nothing (finished normally).
   - If `last_heartbeat` is recent (within the timeout window): the runner reconnected — schedule another timeout for the remaining duration.
   - If job timeout exceeded: mark as `Canceled`.
   - Otherwise: mark as `Failed`.
3. The task is tracked via `HeartbeatTasks` on `ApiContext` and can be canceled when a job reaches a terminal state.

### Startup Recovery

On server startup (`spawn_job_recovery`):
1. `recover_orphaned_claimed_jobs()` finds all `Claimed` jobs where `claimed` (or `created`) is older than the heartbeat timeout and marks them `Failed`.
2. All remaining in-flight (`Claimed` or `Running`) jobs get a `spawn_heartbeat_timeout()` task, recovering jobs that were active when the server previously shut down.

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
| From    | To        | Trigger                                              |
| ------- | --------- | ---------------------------------------------------- |
| pending | claimed   | Runner claims job                                    |
| pending | canceled  | User cancels                                         |
| claimed | running   | Runner sends `running` event                         |
| claimed | failed    | Runner sends `failed` event, or heartbeat timeout    |
| claimed | canceled  | User cancels                                         |
| running | completed | Runner sends `completed` event                       |
| running | failed    | Runner sends `failed` event, or heartbeat timeout    |
| running | canceled  | User cancels, or hard job timeout exceeded           |

**Terminal states:** completed, failed, canceled (no transitions out)

**Race condition prevention:** All state transitions use a status filter on the UPDATE query (e.g., `WHERE status = Claimed OR status = Running`). If the UPDATE matches 0 rows, the job was concurrently modified — the handler re-reads the job to determine the current state and responds appropriately.

## Job Output Storage

Job output (stdout, stderr, file contents) is stored in the **same OCI storage backend** (S3 or local filesystem) used for container images, at the path:

```
{project_uuid}/output/v0/jobs/{job_uuid}
```

### Output Types

**Completed (`JsonJobOutputCompleted`):**
- `exit_code: i32`
- `stdout: Option<String>`
- `stderr: Option<String>`
- `output: Option<HashMap<Utf8PathBuf, String>>` — file path to contents map

**Failed (`JsonJobOutputFailed`):**
- `error: String` (required, serves as discriminator)
- `exit_code: Option<i32>`
- `stdout: Option<String>`
- `stderr: Option<String>`

### Storage Flow

1. Runner sends `completed` or `failed` message over WebSocket
2. WebSocket `max_message_size` enforces an upper bound on ingress payload size
3. Server transitions the job to terminal state in the database
4. Server serializes `JsonJobOutput` to JSON and stores it via `oci_storage().job_output().put(project, job, output)`
5. Storage is best-effort (errors logged but don't fail the state transition)
6. When job details are queried via `GET /v0/projects/{project}/jobs/{job}`, output is fetched from storage and included in the `JsonJob` response

### Size Limits

- **WebSocket ingress**: Bounded by `request_body_max_bytes` on the server context (applies to both `max_message_size` and `max_frame_size`)
- **OCI blob uploads**: Bounded by `max_body_size` (default: 1 GiB, configurable via `plus.registry.max_body_size`)

## Runner Execution Flow

```
1. Idle: Long-poll POST /v0/runners/{runner}/jobs
                │
                ▼ (job received with JsonJobConfig + OCI token)
2. Job "claimed" implicitly via claim response
                │
                ▼
3. Open WebSocket: /v0/runners/{runner}/jobs/{job}/channel
                │
                ▼
4. Authenticate to OCI registry using short-lived project-scoped OCI token
   Pull image from registry at digest specified in config
                │
                ▼
5. Create Firecracker microVM with spec constraints
   (architecture, cpu, memory, disk, network from spec)
   Load OCI image rootfs into VM
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
   - On cancel: stop execution, send { "event": "canceled" }, close
   - Read file_paths from VM after execution completes
                │
                ▼
8. Send { "event": "completed", ... } or { "event": "failed", ... }
   - Server stores output in OCI storage
   - Server runs adapter to parse results into the Report
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
2. **Free** (priority 100-199): Eligible only if no other in-flight (Claimed or Running) job exists for the same organization.
3. **Unclaimed** (priority < 100): Eligible only if no other in-flight job exists for the same source IP.

Jobs are ordered by `(priority DESC, created ASC, id ASC)` — highest priority first, FIFO within the same tier. SQLite's serialized write transactions ensure atomicity without explicit row locking.

### OTEL Metrics

Queue time is tracked per priority tier to monitor starvation. A `job.queue.duration` histogram (in seconds) is recorded when a job transitions to `Running`, with a `priority.tier` attribute indicating the tier (enterprise, team, free, unclaimed).

Additional counters: `RunnerJobClaim` (job claimed), `RunnerJobUpdate` (state transitions by kind).

### Usage-Based Billing

Usage is tracked per-minute via Stripe's usage-based pricing. Heartbeats serve double duty:

1. **Liveness check** — Confirms runner is still executing the job
2. **Billing increment** — Reports usage to Stripe (TODO: billing integration)

The API tracks which minutes have been billed via `last_billed_minute` on the job to avoid double-counting if heartbeats arrive early.

On each heartbeat:
1. Update `last_heartbeat` on job
2. Calculate `elapsed_minutes = (now - started) / 60`
3. If `elapsed_minutes > last_billed_minute`, bill the difference to Stripe
4. Update `last_billed_minute = elapsed_minutes`

## Retry Policy

There is no automatic retry. Failed jobs stay in `Failed` status permanently. Users must explicitly re-submit a new run if they want to retry.

This is intentional: for a benchmarking platform, a failed benchmark is signal (flaky test, environment issue, resource exhaustion). Automatic retry would hide this signal and complicate the state machine.

## Authentication

### Token Format

Runner tokens use 64 random hex characters with a `bencher_runner_` prefix (79 chars total). The token is shown exactly once at creation and cannot be retrieved later. Only the SHA-256 hash is stored in the database, so a database breach does not expose usable tokens.

### Token Validation

1. Verify the `bencher_runner_` prefix and total length (79 chars)
2. Hash the provided token with SHA-256
3. Look up the runner by hash AND path parameter (UUID or slug), excluding archived runners
4. Single combined query prevents token enumeration attacks

### Token Rotation

If a token is compromised:
1. Rotate token (`POST /v0/runners/{runner}/token`) — old token is invalidated immediately
2. Update runner agent with new token
3. Archived runners cannot have their tokens rotated

### Request Header

```
Authorization: Bearer bencher_runner_<token>
```

This token is scoped to:
- Only the runner agent endpoints (`/v0/runners/{runner}/jobs[/{job}[/channel]]`)
- Can claim jobs from any project on the server
- Can only perform operations on jobs claimed by this runner

## Design Decisions

- **One job per report**: A Report has at most one Job. Benchmark suites cannot be split across multiple specs within a single report. Users submit separate runs for different specs.
- **No automatic retry**: Failed jobs stay failed. Users re-submit explicitly. See [Retry Policy](#retry-policy).
- **OCI token delivery**: Short-lived, project-scoped token included in claim response (option c). See [OCI Authentication for Runners](#oci-authentication-for-runners).

## Open Questions

- **Report lifecycle**: How should the UI display a Report that has a pending/in-progress job with no results yet? What states does a Report need?
- **Adapter error handling**: If a job completes but the adapter can't parse the output, what is the Report's state?

## Implementation Phases

1. **Phase 1**: Runner registration & heartbeat — Runners can connect and stay alive
2. **Phase 2**: Job queue & claiming — Basic job distribution
3. **Phase 3**: Execution & result reporting — Actually run benchmarks, store output, run adapters
4. **Phase 4**: `/v0/run` integration — Extend run endpoint with `image` and `spec` fields
5. **Phase 5**: Console UI — Manage runners, view job history
