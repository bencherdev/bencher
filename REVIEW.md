# Code Review: Runner System (changes since `devel`)

## Overview

This changeset adds a **Runner System** — a Plus feature for bare-metal benchmark runners that claim and execute jobs from the API. It spans ~8,800 lines across 51 files, including:

- New `plus/api_runners` crate: Runner CRUD, token auth, job claiming (long-poll), job updates, WebSocket channel
- New database migration: `runner` and `job` tables with indexes
- New models/types: `bencher_json`, `bencher_schema`, `bencher_valid` additions
- Public API: project-scoped job listing
- OpenTelemetry instrumentation, rate limiting, Dockerfile updates
- Comprehensive test suite (~3,100 lines)

**Overall assessment: Well-engineered, production-ready code with strong security foundations.** A few issues worth addressing below.

---

## Security

### Token Handling (Strong)
- Tokens use 32 bytes of `rand::random()` + hex encoding with `bencher_runner_` prefix
- Stored as SHA-256 hashes — plaintext never persisted (`runners.rs:303-308`)
- Auth flow: extract from `Authorization: Bearer` header, hash, look up hash in DB (`runner_token.rs:40-59`)
- Archived/locked runners properly rejected

### Observations
- **SHA-256 is fine for high-entropy random tokens** (unlike passwords, these don't benefit from argon2/bcrypt since they can't be brute-forced). The current approach is appropriate.
- **Token format validation** at `runner_token.rs:45` only checks `starts_with(RUNNER_TOKEN_PREFIX)`. Consider also validating length (prefix + 64 hex chars) so malformed tokens are rejected early before hashing.
- **Spec field correctly excluded** from public API responses — only revealed to the claiming runner (`job.rs:81` vs `job.rs:98`). Good data minimization.

---

## Concurrency & Race Conditions

### Job Claiming (Excellent)
The two-phase claim at `jobs.rs:162-227` is well-designed:
1. Read pending job with eligibility filters using `auth_conn!()` (read-only)
2. Atomically update only if status is still `Pending` using `write_conn!()`
3. Check `updated > 0` to detect if another runner won the race

This is textbook optimistic locking. The test at `test_concurrent_job_claim` confirms correctness.

### Heartbeat Timeout (Minor Concern)
`spawn_heartbeat_timeout` at `job.rs:183-245` recursively spawns tasks when a runner reconnects (line 223). In theory, if a runner repeatedly disconnects/reconnects, this could accumulate spawned tasks. In practice:
- Each recursion holds the connection lock briefly
- Terminal state check prevents duplicate writes
- The remaining timeout shrinks each iteration

**Low risk**, but consider adding a max-reschedule counter to be safe.

---

## Bugs & Issues

### 1. Down Migration Missing Index Drops (Low)
**`down.sql`** only drops `index_job_pending` explicitly but the up migration creates 3 indexes. Since `DROP TABLE` cascades to indexes in SQLite, this technically works. However, for clarity and defensive coding:

```sql
DROP INDEX IF EXISTS index_job_pending;
DROP INDEX IF EXISTS index_job_org_running;
DROP INDEX IF EXISTS index_job_source_ip_running;
DROP TABLE IF EXISTS job;
DROP TABLE IF EXISTS runner;
```

### 2. Missing `runner_id` Index on Job Table (Medium)
There's no index on `job(runner_id)` or `job(runner_id, status)`. Queries that look up jobs by runner (e.g., checking a runner's active jobs, or the `ON DELETE RESTRICT` FK check) will require a full table scan. Consider adding:

```sql
CREATE INDEX index_job_runner ON job(runner_id) WHERE runner_id IS NOT NULL;
```

### 3. FIFO Ordering Tiebreaker (Very Low)
`jobs.rs:165`: `.order((priority.desc(), created.asc()))` — if two jobs have identical priority AND created timestamp, ordering is undefined. Adding `id.asc()` as a final tiebreaker would make it deterministic:

```rust
.order((priority.desc(), created.asc(), id.asc()))
```

### 4. `output` Is Dropped in `handle_completed` (Intentional TODO)
`channel.rs:399-400`: The `output` field from completed jobs is explicitly `drop()`ed with a TODO comment. This is clearly tracked but worth flagging — runners sending output data will silently lose it until storage is implemented.

### 5. `handle_running` Uses `write_conn!()` for Initial Read (`channel.rs:274-276`)
The job state is read via `write_conn!()` before potentially updating it. This is correct (avoids TOCTOU by holding the write lock) but means read-only reconnection checks also acquire the write lock. Acceptable for correctness, but could be a bottleneck under high reconnection rates.

---

## State Machine

The job state machine is well-enforced across both REST and WebSocket paths:

```
Pending → Claimed     (job claim endpoint)
Claimed → Running     (WS Running message)
Claimed → Failed      (WS Failed message, or REST PATCH)
Running → Completed   (WS Completed message, or REST PATCH)
Running → Failed      (WS Failed / heartbeat timeout / REST PATCH)
Running → Canceled    (WS Cancelled message)
```

Transition validation at `jobs.rs:293-304` and throughout `channel.rs` handlers is correct and consistent. `is_terminal()` properly identifies `Completed | Failed | Canceled`.

---

## API Design

- RESTful resource paths (`/v0/runners`, `/v0/runners/{runner}/jobs`, etc.)
- Clean separation: admin endpoints (user bearer token) vs runner endpoints (runner token)
- Long-poll with configurable timeout clamped to 1-60s (`jobs.rs:97-101`)
- WebSocket channel for persistent heartbeat/status communication
- CORS endpoints for all paths
- Pagination + filtering on list endpoints with `X-Total-Count` header
- OpenAPI spec generated and matches implementation

---

## Performance

### Long-Polling Efficiency
`claim_job_inner` polls every 1 second for up to 60 seconds (`jobs.rs:104-117`). This means up to 60 DB queries per runner per poll cycle. For a small number of runners this is fine. For scale:
- Consider a notification/wake mechanism (e.g., `tokio::sync::Notify`) to avoid empty polls
- Or increase the poll interval with exponential backoff

### Index Usage for Concurrency Checks
The correlated subqueries at `jobs.rs:142-154` use `NOT EXISTS` with `job_org` and `job_ip` table aliases. The partial indexes (`WHERE status = 2`) should optimize these, but it's worth verifying with `EXPLAIN QUERY PLAN` in SQLite to confirm the partial indexes are actually used for these subqueries.

---

## Code Quality

- Follows project conventions (Dropshot endpoints, Diesel macros, `auth_conn!/write_conn!`)
- No `#[allow(...)]` suppressions
- Clean error handling with project-standard macros (`resource_not_found_err!`, `resource_conflict_err!`)
- Good OpenTelemetry instrumentation behind feature flags
- Rate limiting properly integrated
- Dockerfiles updated for the new crate

---

## Test Coverage (Strong)

~3,100 lines covering:
- **Authentication**: Invalid tokens, wrong runner, locked/archived runners, missing headers
- **Job claiming**: No jobs available, concurrent claiming, priority ordering, FIFO within priority
- **Tier-based concurrency**: Enterprise unlimited, Free per-org limit, Unclaimed per-IP limit, tier boundary values
- **State machine**: All valid transitions + key invalid transitions
- **WebSocket**: Full lifecycle, reconnection, heartbeat timeout, cancellation, invalid messages, ping/pong

### Missing test scenarios (for future work):
- All invalid state transitions (e.g., Pending→Running, Completed→Running)
- Concurrent token rotation
- Poll timeout boundary values (0, 61)
- Runner deletion with active jobs (FK constraint)
- Very large WebSocket messages

---

## Summary of Recommendations

| Priority | Issue | Location |
|----------|-------|----------|
| **Medium** | Add `job(runner_id)` index | `up.sql` |
| **Low** | Add missing index drops to down migration | `down.sql` |
| **Low** | Validate token length in format check | `runner_token.rs:45` |
| **Low** | Add FIFO tiebreaker (`id.asc()`) | `jobs.rs:165` |
| **Low** | Add max-reschedule counter to heartbeat timeout | `job.rs:223` |
| **Future** | Implement output storage (TODO exists) | `channel.rs:399` |
| **Future** | Consider notification-based wakeup for long-poll | `jobs.rs:104-117` |
| **Future** | Verify SQLite query plans for claim subqueries | `jobs.rs:142-154` |