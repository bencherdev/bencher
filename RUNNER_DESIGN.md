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
3. Runner claims job from API, receives `JsonJobSpec`
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

## Database Schema

```sql
-- Runner registration and state (server-scoped, serves all projects)
CREATE TABLE runner (
    id UUID PRIMARY KEY,
    uuid UUID NOT NULL UNIQUE,        -- Runner's self-generated ID
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,        -- URL-friendly name
    token_hash TEXT NOT NULL,         -- SHA-256 hash of token (token itself never stored)
    labels JSONB NOT NULL DEFAULT '[]', -- ["arch:arm64", "os:linux"]
    state TEXT NOT NULL DEFAULT 'offline', -- offline, idle, running
    locked TIMESTAMP,              -- If set, runner is locked (token compromised)
    archived TIMESTAMP,               -- Soft delete
    last_heartbeat TIMESTAMP,
    created TIMESTAMP NOT NULL,
    modified TIMESTAMP NOT NULL
);

-- Job status enum (stored as integer)
-- 0 = pending
-- 1 = claimed
-- 2 = running
-- 3 = completed
-- 4 = failed
-- 5 = canceled

-- Job queue
CREATE TABLE job (
    id UUID PRIMARY KEY,
    report_id UUID NOT NULL REFERENCES report(id) ON DELETE CASCADE,

    -- Job specification (JsonJobSpec serialized as JSON)
    status INTEGER NOT NULL DEFAULT 0,  -- JobStatus enum
    spec JSONB NOT NULL,                -- JsonJobSpec
    priority INTEGER NOT NULL DEFAULT 0,  -- 0 = free, 100 = plus

    -- Execution tracking
    runner_id UUID REFERENCES runner(id) ON DELETE RESTRICT,
    claimed TIMESTAMP,
    started TIMESTAMP,                -- When benchmark actually began (after setup)
    completed TIMESTAMP,
    last_heartbeat TIMESTAMP,
    last_billed_minute INTEGER DEFAULT 0,  -- Minutes billed so far

    -- Results
    exit_code INTEGER,

    created TIMESTAMP NOT NULL,
    modified TIMESTAMP NOT NULL
);

-- Note: The `spec` column contains JsonJobSpec:
-- {
--   "registry": "https://registry.bencher.dev",
--   "project": "<project-uuid>",
--   "digest": "sha256:...",
--   "entrypoint": ["./run.sh"],      // optional
--   "cmd": ["--iterations", "100"],  // optional
--   "env": {"KEY": "value"},         // optional
--   "vcpu": 4,
--   "memory": 8589934592,            // 8 GB in bytes
--   "disk": 21474836480,             // 20 GB in bytes
--   "timeout": 3600,
--   "network": false
-- }

-- Index for job claiming (ordered by priority, then FIFO)
CREATE INDEX idx_job_pending
    ON job(status, priority DESC, created ASC)
    WHERE status = 0;  -- pending
```

```rust
#[repr(u8)]
pub enum JobStatus {
    Pending = 0,
    Claimed = 1,
    Running = 2,
    Completed = 3,
    Failed = 4,
    Canceled = 5,
}
```

## Job Spec Structure

The job spec is designed to minimize data sent to runners, reducing leakage risk. Runners only receive what's necessary to pull and execute an OCI image.

```rust
/// Job specification sent to runners when claiming a job.
/// Defined in `bencher_json`, with `ImageDigest` validated in `bencher_valid`.
pub struct JsonJobSpec {
    /// Registry URL for pulling the OCI image (e.g., "https://registry.bencher.dev")
    pub registry: Url,
    /// Project UUID for OCI authentication scoping
    pub project: ProjectUuid,
    /// Image digest - must be immutable (e.g., "sha256:abc123...")
    pub digest: ImageDigest,
    /// Entrypoint override (like Docker ENTRYPOINT)
    pub entrypoint: Option<Vec<String>>,
    /// Command override (like Docker CMD)
    pub cmd: Option<Vec<String>>,
    /// Environment variables passed to the container
    pub env: Option<HashMap<String, String>>,
    /// Number of virtual CPUs for the VM
    pub vcpu: u32,
    /// Memory size in bytes
    pub memory: u64,
    /// Disk size in bytes
    pub disk: u64,
    /// Maximum execution time in seconds
    pub timeout: u32,
    /// Whether the VM has network access
    pub network: bool,
}
```

**Design principles:**
- **Minimal information**: Runner doesn't know repo URL, branch, commit, or benchmark commands directly
- **Immutable reference**: Digest (not tag) ensures the image can't change between job creation and execution
- **Isolated execution**: VM resources (vcpu, memory, disk) and network access are explicit
- **OCI-based**: All benchmark code is packaged in an OCI image, pulled from the Bencher registry

## API Endpoints

### Runner Management (Server Scoped)

Requires server admin permissions.

| Method | Endpoint                     | Description                                    |
| ------ | ---------------------------- | ---------------------------------------------- |
| POST   | `/v0/runners`                | Create runner, returns token                   |
| GET    | `/v0/runners`                | List runners                                   |
| GET    | `/v0/runners/{runner}`       | Get runner details                             |
| PATCH  | `/v0/runners/{runner}`       | Update runner (name, labels, locked, archived) |
| POST   | `/v0/runners/{runner}/token` | Generate new token (invalidates old)           |

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

## Endpoint Details

### POST /v0/runners - Register Runner

```rust
// Request (by project admin)
pub struct CreateRunner {
    pub name: String,
    pub labels: Vec<String>,
}

// Response (token shown once, then hashed)
pub struct RunnerCreated {
    pub uuid: Uuid,
    pub token: String,  // "bencher_runner_xxxx" - only shown once
}
```

### POST /v0/runners/{runner}/jobs - Claim Job (Long-Poll)

```rust
// Request
pub struct JsonClaimJob {
    pub poll_timeout: Option<u32>,  // 1-60 seconds, default 30
}

// Response (after job available or timeout)
// Returns Option<JsonJob> - None if timeout with no jobs

/// Job returned to runner on claim (includes spec for execution)
pub struct JsonJob {
    pub uuid: JobUuid,
    pub status: JobStatus,
    pub spec: JsonJobSpec,          // What to execute (see Job Spec Structure)
    pub runner: Option<RunnerUuid>,
    pub claimed: Option<DateTime>,
    pub started: Option<DateTime>,
    pub completed: Option<DateTime>,
    pub exit_code: Option<i32>,
    pub created: DateTime,
    pub modified: DateTime,
}
```

The claim endpoint:
1. Applies per-runner rate limiting to prevent abuse of long-polling
2. Finds pending jobs ordered by `(priority DESC, created ASC)`
3. Atomically updates job status to `claimed`, sets `runner_id` and `claimed`
4. If no matching jobs, holds connection open until timeout or job arrives
5. Returns job with spec or `None` on timeout

### WebSocket /v0/runners/{runner}/jobs/{job}/channel - Job Execution Channel

WebSocket connection for heartbeat and job status updates. Established after claiming a job, before benchmark execution begins.

**Authentication:** Runner token passed via `Sec-WebSocket-Protocol: bearer.<token>` header.

```rust
// Runner → Server messages (JSON)
#[serde(tag = "event", rename_all = "snake_case")]
enum RunnerMessage {
    /// Job setup complete, benchmark execution starting
    Running,
    /// Periodic heartbeat (~1/sec), keeps job alive and triggers billing
    Heartbeat,
    /// Benchmark completed successfully
    Completed {
        exit_code: i32,
        output: Option<String>,
    },
    /// Benchmark failed
    Failed {
        exit_code: Option<i32>,
        error: String,
    },
    /// Acknowledge cancellation from server
    Cancelled,
}

// Server → Runner messages (JSON)
#[serde(tag = "event", rename_all = "snake_case")]
enum ServerMessage {
    /// Acknowledge received message
    Ack,
    /// Job was canceled, stop execution immediately
    Cancel,
}
```

**Connection flow:**
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

**Cancellation flow:**
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

1. **Inline WS timeout** — While the WebSocket connection is open, `tokio::time::timeout(heartbeat_timeout, rx.next())` detects a "connected but silent" runner. On timeout, the job is marked `Failed` immediately within the WS loop.

2. **Spawned disconnect timeout** — When a WebSocket disconnects and the job is still in-flight (non-terminal), a background `tokio::spawn` task sleeps for `heartbeat_timeout`. After waking, it checks:
   - If the job reached a terminal state: do nothing (finished normally).
   - If `last_heartbeat` is recent (within the timeout window): the runner reconnected — schedule another timeout for the remaining duration.
   - Otherwise: mark the job as `Failed`.

**Startup recovery:** On server startup, all `Claimed` or `Running` jobs are queried and a timeout task is spawned for each, recovering jobs that were in-flight when the server previously shut down.

**Heartbeat timeout is configurable** via `ApiContext.heartbeat_timeout` (default: 90 seconds in production, 5 seconds in tests).

### WebSocket Reconnection

If a runner disconnects and reconnects to a `Running` job, the WebSocket channel accepts the connection (status check allows `Claimed | Running`). Sending a `Running` message on a job that is already `Running` is idempotent — it updates `last_heartbeat` without changing the status or `started` timestamp. This cancels any pending disconnect-timeout task (via the `last_heartbeat` freshness check).

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
| claimed | running   | Runner calls `/started`           |
| claimed | failed    | Runner fails during setup         |
| claimed | canceled  | User cancels                      |
| running | completed | Runner calls `/completed`         |
| running | failed    | Runner calls `/failed` or timeout |
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
                ▼ (job received with JsonJobSpec)
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
5. Create VM with spec constraints (vcpu, memory, disk, network)
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
   - Timeout enforced per job spec
                │
                ▼
8. Send { "event": "completed", ... } or { "event": "failed", ... }
   - Results attached to the Report
                │
                ▼
9. Destroy VM, close WebSocket, return to idle
```

## Billing & Priority

### Job Priority

Jobs are queued with priority based on the submitting organization's plan:

| Plan                | Priority | Behavior               |
| ------------------- | -------- | ---------------------- |
| Bencher Plus (paid) | High     | Front of queue         |
| Free                | Low      | Waits behind paid jobs |

The claim endpoint orders pending jobs by `(priority DESC, created ASC)` so paid customers always get served first, with FIFO within each tier.

### Usage-Based Billing

Usage is tracked per-minute via Stripe's usage-based pricing. Heartbeats (received out-of-band) serve double duty:

1. **Liveness check** - Confirms runner is still executing the job
2. **Billing increment** - Reports usage to Stripe

The API tracks which minutes have been billed via `last_billed_minute` on the job (to avoid double-counting if heartbeats arrive early).

On each heartbeat:
1. Update `last_heartbeat` on job and runner
2. Calculate `elapsed_minutes = (now - started) / 60`
3. If `elapsed_minutes > last_billed_minute`, bill the difference to Stripe
4. Update `last_billed_minute = elapsed_minutes`

## Authentication

### Token Format

Runner tokens use random bytes with a prefix (not JWTs):

```rust
// Generation (only done once, at runner creation)
let random_bytes: [u8; 32] = rand::random();
let token = format!("bencher_runner_{}", hex::encode(&random_bytes));
// Example: bencher_runner_a1b2c3d4e5f6...

// Storage (only the hash is stored, never the token itself)
let token_hash = sha256(token.as_bytes());
```

**Key properties:**
- Token shown exactly once at creation (cannot be retrieved later)
- Only SHA-256 hash stored in database
- DB breach doesn't expose usable tokens
- Prefix `bencher_runner_` makes token type obvious

### Token Validation

```rust
fn validate_runner_token(token: &str) -> Result<Runner, AuthError> {
    // 1. Check prefix
    let token = token.strip_prefix("bencher_runner_")
        .ok_or(AuthError::InvalidToken)?;

    // 2. Hash the provided token
    let token_hash = sha256(format!("bencher_runner_{}", token).as_bytes());

    // 3. Look up runner by hash (excluding archived)
    let runner = db.query(
        "SELECT * FROM runner WHERE token_hash = ? AND archived IS NULL",
        token_hash
    )?;

    // 4. Check if locked
    if runner.locked.is_some() {
        return Err(AuthError::RunnerLocked);
    }

    Ok(runner)
}
```

### Token Rotation

If a token is compromised:
1. Lock the runner: `PATCH /v0/runners/{runner}` with `locked: true`
2. Generate new token: `POST /v0/runners/{runner}/token`
3. Update runner agent with new token
4. Unlock the runner: `PATCH /v0/runners/{runner}` with `locked: false`

### Request Header

```
Authorization: Bearer bencher_runner_<token>
```

This token is scoped to:
- Only the runner agent endpoints (`/v0/runners/{runner}/jobs[/{job}[/channel]]`)
- Can claim jobs from any project on the server
- Can only perform operations on jobs claimed by this runner

## Open Questions

- [x] **Runner scope**: ~~Project-scoped or organization-scoped?~~ **Decided: Server-scoped** - Runners can execute jobs from any project on the server (both self-hosted and cloud)
- [x] **Job priority**: ~~FIFO or priority field?~~ **Decided: Priority + FIFO** - Bencher Plus customers get priority, FIFO within tiers
- [x] **Usage billing**: ~~How to track?~~ **Decided: Stripe usage-based pricing** - Heartbeats trigger per-minute billing to Stripe
- [x] **Heartbeat protocol**: ~~UDP? WebSocket? gRPC stream?~~ **Decided: WebSocket** - Low overhead, immediate cancellation, connection-based liveness detection
- [x] **Stale job recovery**: ~~Periodic reaper or per-job timeout?~~ **Decided: Per-job timeout** - Spawned on WS disconnect and at startup; no periodic polling
- [x] **Job spec design**: ~~What info does runner need?~~ **Decided: Minimal OCI-based spec** - Registry URL, project UUID, image digest, entrypoint/cmd/env overrides, VM resources (vcpu/memory/disk), timeout, network access
- [ ] **Result storage**: Store in job table or separate results table linked to existing perf tables?
- [ ] **Output storage**: Store benchmark output from WebSocket `completed` messages (currently dropped). Needed before the runner feature is usable end-to-end.
- [ ] **Retry policy**: Auto-retry failed jobs? How many times?
- [ ] **Job spec persistence**: Currently `spec` is stored as JSON string in job table. Need to implement `JsonJobSpec` type in `bencher_json` and `ImageDigest` validation in `bencher_valid`.
- [ ] **OCI auth for runners**: How does runner authenticate to registry? Options: (a) runner token directly, (b) exchange runner token for short-lived OCI token via API, (c) job claim response includes OCI token.

## Implementation Phases

1. **Phase 1**: Runner registration & heartbeat - Runners can connect and stay alive
2. **Phase 2**: Job queue & claiming - Basic job distribution
3. **Phase 3**: Execution & result reporting - Actually run benchmarks
4. **Phase 4**: Labels & affinity - Match jobs to appropriate hardware
5. **Phase 5**: Console UI - Manage runners, view job history
