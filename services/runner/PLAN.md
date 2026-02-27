# `bencher run --image` End-to-End Plan

Track the work needed to go from `bencher run --image` invocation through runner execution to returning results to the user.

## Current State

The CLI, runner daemon, WebSocket protocol, job viewing endpoints, job creation in `run_post` (Gap 1), and benchmark result processing (Gap 2) are all implemented. One gap remains before the flow works end-to-end.

## Flow Overview

```
bencher run --image ghcr.io/org/bench:v1 --adapter json
  │
  ├─1. CLI builds JsonNewRun with job: Some(JsonNewRunJob{...})
  ├─2. CLI sends POST /v0/run to API
  │
  ├─3. API creates report (empty results)
  ├─4. API creates job record linked to report
  │     └─ resolves image digest, spec, priority
  │
  ├─5. Runner daemon long-polls POST /v0/runners/{runner}/jobs
  ├─6. Runner claims job, opens WebSocket
  ├─7. Runner executes benchmark in Firecracker microVM
  ├─8. Runner sends Completed{results: Vec<JsonIterationOutput>}
  │
  ├─9. API receives Completed via WebSocket
  ├─10. API processes benchmark results via adapter
  │      └─ creates metrics, benchmarks, alerts
  │      └─ updates report with results and end_time
  │
  ├─11. CLI polls for job completion                     ← GAP 3
  └─12. CLI fetches updated report, displays results
```

## In-Code TODOs

1. **`lib/bencher_schema/src/model/runner/job.rs`** — `TODO: Check metered plan level to distinguish Team vs Enterprise`
   - Currently all `PlanKind::Metered` maps to `JobPriority::Team`; should distinguish Enterprise tier

2. **`plus/api_runners/src/jobs/websocket.rs`** — `TODO: Billing logic - check elapsed minutes and bill to Stripe`
   - Billing for runner usage not yet implemented

3. **`lib/bencher_schema/src/model/runner/job.rs` — `process_results()`** — `TODO: Refactor PlanKind to support auth_conn directly`
   - `PlanKind::new_for_project` requires a `PublicUser` for `public_conn!` routing. In the runner context we're already authenticated, so we use `PublicUser::Public(None)` as a workaround. Refactor `PlanKind` (and its callees like `QueryPlan::get_active_metered_plan`, `LicenseUsage::get`, `QueryOrganization::window_usage`) to accept a `&mut DbConnection` directly instead of requiring `public_conn!`.

## Gap 3: CLI Polling and Result Display

**Where:** `services/cli/src/bencher/sub/run/mod.rs` — `exec_inner()`

**Problem:** After sending the run, `exec_inner()` immediately expects a `JsonReport` back with results. For `--image` runs it receives an empty report (zero results) and exits.

**What needs to happen:**
- [ ] Detect that the returned report has an associated pending job
- [ ] Enter a polling loop: `GET /v0/projects/{project}/jobs/{job}` until terminal state
- [ ] Display progress/status updates while waiting (e.g., "Job pending...", "Job running...")
- [ ] On completion: fetch the updated `JsonReport` (now with processed results)
- [ ] Display stdout/stderr from the job output
- [ ] Display benchmark results and alerts (same as local runs)
- [ ] On failure/cancellation: display error details and exit non-zero
- [ ] Respect `--ci-only` and other display flags

**Key endpoints already implemented:**
- `GET /v0/projects/{project}/jobs/{job}` returns `JsonJob` with status and output
- `GET /v0/projects/{project}/reports/{report}` returns `JsonReport` with results

**Open questions:**
- What poll interval? (The runner daemon uses 55s long-poll; CLI should probably use shorter intervals like 5-10s)
- Should the CLI stream stdout/stderr in real-time via WebSocket, or just fetch at the end?
- Timeout behavior: should the CLI have its own timeout for waiting on job completion?

## Gap 4: `bencher noise` Subcommand

See [`services/cli/NOISE_PLAN.md`](../cli/NOISE_PLAN.md) for the design of the `bencher noise` subcommand, which will generate synthetic benchmark results for testing the full runner flow without needing real benchmarks or a real adapter.

## Gap 5: Claude Code Skill for Bencher Workflow

See [`services/cli/SKILL_PLAN.md`](../cli/SKILL_PLAN.md) for the design of a Claude Code skill that teaches AI agents the Bencher workflow: project setup, benchmark runs (local and bare metal), threshold configuration, CI integration, and result interpretation.

## Implementation Order

1. ~~**Gap 1 (job creation)**~~ — Complete. Jobs are created in `run_post` with spec resolution, image resolution, and priority determination.
2. ~~**Gap 2 (result processing)**~~ — Complete. `QueryJob::process_results()` parses benchmark output via adapter, creates metrics/alerts, and updates report timestamps. WebSocket protocol uses `Vec<JsonIterationOutput>` for per-iteration results.
3. **Gap 3 (CLI polling)** — Without this, the user sees empty results even after Gap 2 is fixed.
4. **Gap 4 (`bencher noise`)** — This is a separate feature that can be implemented after the main flow works end-to-end.
5. **Gap 5 (Claude Code skill)** — Agent skill for guiding users through Bencher workflows. Can be implemented independently of Gaps 2-4.

## SQLite Write Lock Contention

`spawn_heartbeat_timeout()` in `lib/bencher_schema/src/model/runner/job.rs` uses the same shared `Arc<Mutex<DbConnection>>` as API request handlers. Background heartbeat tasks contend with foreground writes. Create a notification mechanism for runners awaiting jobs so they don't poll the DB, and consider a dedicated background connection.
