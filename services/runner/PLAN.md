# `bencher run --image` End-to-End Plan

Track the work needed to go from `bencher run --image` invocation through runner execution to returning results to the user.

## Current State

The end-to-end flow is implemented: CLI, runner daemon, WebSocket protocol, job viewing endpoints, job creation (`run_post`), benchmark result processing, and CLI polling/result display are all complete.

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
  ├─11. CLI polls for job completion
  └─12. CLI fetches updated report, displays results
```

## In-Code TODOs

1. **`lib/bencher_schema/src/model/runner/job.rs`** — `TODO: Check metered plan level to distinguish Team vs Enterprise`
   - Currently all `PlanKind::Metered` maps to `JobPriority::Team`; should distinguish Enterprise tier

2. **`plus/api_runners/src/jobs/websocket.rs`** — `TODO: Billing logic - check elapsed minutes and bill to Stripe`
   - Billing for runner usage not yet implemented

3. **`lib/bencher_schema/src/model/runner/job.rs` — `process_results()`** — `TODO: Refactor PlanKind to support auth_conn directly`
   - `PlanKind::new_for_project` requires a `PublicUser` for `public_conn!` routing. In the runner context we're already authenticated, so we use `PublicUser::Public(None)` as a workaround. Refactor `PlanKind` (and its callees like `QueryPlan::get_active_metered_plan`, `LicenseUsage::get`, `QueryOrganization::window_usage`) to accept a `&mut DbConnection` directly instead of requiring `public_conn!`.

## Gap 4: `bencher noise` Subcommand

See [`services/cli/NOISE_PLAN.md`](../cli/NOISE_PLAN.md) for the design of the `bencher noise` subcommand, which will generate synthetic benchmark results for testing the full runner flow without needing real benchmarks or a real adapter.

## Gap 5: Claude Code Skill for Bencher Workflow

See [`services/cli/SKILL_PLAN.md`](../cli/SKILL_PLAN.md) for the design of a Claude Code skill that teaches AI agents the Bencher workflow: project setup, benchmark runs (local and bare metal), threshold configuration, CI integration, and result interpretation.

## Implementation Order

1. ~~**Gap 1 (job creation)**~~ — Complete.
2. ~~**Gap 2 (result processing)**~~ — Complete.
3. ~~**Gap 3 (CLI polling)**~~ — Complete. CLI polls `GET /v0/projects/{project}/jobs/{job}` with configurable interval (default 5s), displays status updates, fetches updated report on completion, and handles failure/cancellation/timeout.
4. **Gap 4 (`bencher noise`)** — Design complete (`services/cli/NOISE_PLAN.md`), implementation pending.
5. **Gap 5 (Claude Code skill)** — Design complete (`services/cli/SKILL_PLAN.md`), implementation pending.

## SQLite Write Lock Contention

`spawn_heartbeat_timeout()` in `lib/bencher_schema/src/model/runner/job.rs` uses the same shared `Arc<Mutex<DbConnection>>` as API request handlers. Background heartbeat tasks contend with foreground writes. Create a notification mechanism for runners awaiting jobs so they don't poll the DB, and consider a dedicated background connection.
