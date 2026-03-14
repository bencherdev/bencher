# `bencher run --image` Remaining Work

## In-Code TODOs

1. **`plus/api_runners/src/channel.rs`** — `TODO: Billing logic - check elapsed minutes and bill to Stripe`
   - Billing for runner usage not yet implemented

2. **`lib/bencher_schema/src/model/runner/job.rs` — `process_results()`** — `TODO: Refactor PlanKind to support auth_conn directly`
   - `PlanKind::new_for_project` requires a `PublicUser` for `public_conn!` routing. In the runner context we're already authenticated, so we use `PublicUser::Public(None)` as a workaround. Refactor `PlanKind` (and its callees like `QueryPlan::get_active_metered_plan`, `LicenseUsage::get`, `QueryOrganization::window_usage`) to accept a `&mut DbConnection` directly instead of requiring `public_conn!`.

## Claude Code Skill for Bencher Workflow

See [`services/cli/SKILL_PLAN.md`](../cli/SKILL_PLAN.md) for the design of a Claude Code skill that teaches AI agents the Bencher workflow: project setup, benchmark runs (local and bare metal), threshold configuration, CI integration, and result interpretation.

## CLI Polling → Server-Sent Events

The CLI currently polls `GET /v0/projects/{project}/jobs/{job}` on a fixed interval to track job progress. Replace this with server-sent events (SSE) so the API pushes status updates to the CLI in real time, eliminating polling latency and unnecessary requests.

## CLI Docker Image Install Docs

Add documentation for using the `bencher` CLI Docker image `ghcr.io/bencherdev/bencher` to the install CLI docs.

## SQLite Write Lock Contention

`spawn_heartbeat_timeout()` in `lib/bencher_schema/src/model/runner/job.rs` uses the same shared `Arc<Mutex<DbConnection>>` as API request handlers. Background heartbeat tasks contend with foreground writes. Create a notification mechanism for runners awaiting jobs so they don't poll the DB, and consider a dedicated background connection.
