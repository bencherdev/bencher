# `bencher run --image` End-to-End Plan

Track the work needed to go from `bencher run --image` invocation through runner execution to returning results to the user.

## Current State

The CLI, runner daemon, WebSocket protocol, and job viewing endpoints are all implemented. Three critical gaps prevent the flow from working end-to-end.

## Flow Overview

```
bencher run --image ghcr.io/org/bench:v1 --adapter json
  │
  ├─1. CLI builds JsonNewRun with job: Some(JsonNewRunJob{...})
  ├─2. CLI sends POST /v0/run to API
  │
  ├─3. API creates report (empty results)
  ├─4. API creates job record linked to report          ← GAP 1
  │     └─ resolves image digest, spec, priority
  │
  ├─5. Runner daemon long-polls POST /v0/runners/{runner}/jobs
  ├─6. Runner claims job, opens WebSocket
  ├─7. Runner executes benchmark in Firecracker microVM
  ├─8. Runner sends Completed{exit_code, stdout, stderr, output}
  │
  ├─9. API receives Completed via WebSocket
  ├─10. API processes benchmark output through adapter   ← GAP 2
  │      └─ creates metrics, benchmarks, alerts
  │      └─ updates report with results and end_time
  │
  ├─11. CLI polls for job completion                     ← GAP 3
  └─12. CLI fetches updated report, displays results
```

## Gap 1: Job Creation in `run_post`

**Where:** `lib/api_run/src/run.rs` — `run_post` handler

**Problem:** The `From<JsonNewRun> for JsonNewReport` conversion silently discards the `job` field (`job: _`). No job record is ever inserted into the database.

**What needs to happen:**
- [ ] When `JsonNewRun.job` is `Some(...)`, create a job record in the database
- [ ] Resolve the image reference to an OCI digest (via the registry or pass-through)
- [ ] Resolve the hardware spec (from `job.spec` or a default)
- [ ] Call `InsertJob::new()` to insert a job linked to the report
- [ ] Determine priority from the organization's plan tier
- [ ] Set the report's start/end time appropriately for deferred execution
- [ ] Return the report (with job UUID) so the CLI knows what to poll

**Key types already implemented:**
- `InsertJob::new()` in `lib/bencher_schema/src/model/runner/job.rs` (only used in tests today)
- `JsonNewRunJob` in `lib/bencher_json/src/run.rs`
- `JobStatus::Pending` initial state

## Gap 2: Benchmark Result Processing After Job Completion

**Where:** `plus/api_runners/src/jobs/websocket.rs` — `handle_completed()`

**Problem:** When the runner sends `Completed{exit_code, stdout, stderr, output}`, the handler only stores raw output to blob storage. It does not parse benchmark results or create metrics/alerts.

**What needs to happen:**
- [ ] After storing raw output, determine which adapter to use (from the report's settings)
- [ ] Parse stdout (and/or output files) through `bencher_adapter` to extract benchmark results
- [ ] Call `ReportResults::process()` (or equivalent) to create metrics, benchmarks, and report_benchmarks
- [ ] Run threshold detection to generate alerts
- [ ] Update the report with real `end_time` and processed results
- [ ] Handle the case where the adapter fails to parse (mark report as failed? store raw output anyway?)

**Key types already implemented:**
- `bencher_adapter` crate for parsing benchmark output
- `ReportResults` in `lib/api_run/` for processing results into metrics
- Blob storage for raw output already works

**Open questions:**
- Which field contains the benchmark output — stdout, a specific output file, or configurable?
- Should the adapter be stored on the report/job, or inferred from the run settings?
- What happens if the benchmark exits non-zero but produces partial output?

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

## Implementation Order

1. **Gap 1 (job creation)** — Without this, no jobs enter the queue and the runner has nothing to claim.
2. **Gap 2 (result processing)** — Without this, completed jobs produce no metrics/alerts even if the runner executes successfully.
3. **Gap 3 (CLI polling)** — Without this, the user sees empty results even after gaps 1 and 2 are fixed.

## What Already Works

- [x] CLI parsing of `--image`, `--entrypoint`, `--cmd`, `--env`, `--timeout`, `--spec`
- [x] `JsonNewRun` and `JsonNewRunJob` types
- [x] Runner daemon polling, job claiming, WebSocket lifecycle
- [x] Runner job execution in Firecracker microVM (with entrypoint/cmd/env overrides)
- [x] WebSocket protocol: Running, Heartbeat, Completed, Failed, Canceled
- [x] Server-side WebSocket handler: status updates, blob storage, heartbeat timeout
- [x] Job viewing endpoints (`GET /v0/projects/{project}/jobs[/{job}]`)
- [x] CLI `bencher job list` and `bencher job view` commands
- [x] `InsertJob::new()` constructor (used in tests)
- [x] Tier-based concurrency limits for job claiming
