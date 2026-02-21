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

## Step 0: Add `spec_id` to Testbed (Current Child Pattern)

**Standalone prerequisite** — can be implemented and merged independently before Gap 1.

### Pattern: "Current Child" FK

This follows the same "current child" pattern used by two other entity pairs in the codebase:

| Parent | Child | FK on Parent | `replaced` on Child |
|--------|-------|-------------|---------------------|
| `Branch` | `Head` | `head_id: Option<HeadId>` | `replaced: Option<DateTime>` |
| `Threshold` | `Model` | `model_id: Option<ModelId>` | `replaced: Option<DateTime>` |
| **`Testbed`** | **`Spec`** | **`spec_id: Option<SpecId>`** | **N/A (specs are server-scoped)** |

Key difference: Specs are **server-scoped** (not owned by a testbed), so there is no `replaced` timestamp. Multiple testbeds can share the same spec. When a testbed's spec changes, the old spec remains valid.

### 0.1 Migration — Recreate testbed table with `spec_id` between `slug` and `created`

SQLite doesn't support `ALTER TABLE ADD COLUMN` in arbitrary positions. Use the standard table-recreation pattern (see `2024-07-07-133201_archived/up.sql` for the most recent example that touched testbed).

**`lib/bencher_schema/migrations/<timestamp>_testbed_spec/up.sql`:**
```sql
PRAGMA foreign_keys = off;
CREATE TABLE up_testbed (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    spec_id INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    archived BIGINT,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (spec_id) REFERENCES spec (id) ON DELETE SET NULL,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO up_testbed(
        id, uuid, project_id, name, slug, spec_id,
        created, modified, archived
    )
SELECT id, uuid, project_id, name, slug, null,
    created, modified, archived
FROM testbed;
DROP TABLE testbed;
ALTER TABLE up_testbed RENAME TO testbed;
CREATE INDEX index_testbed_project_created ON testbed(project_id, created);
PRAGMA foreign_keys = on;
```

**`lib/bencher_schema/migrations/<timestamp>_testbed_spec/down.sql`:**
```sql
PRAGMA foreign_keys = off;
CREATE TABLE down_testbed (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    archived BIGINT,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO down_testbed(
        id, uuid, project_id, name, slug,
        created, modified, archived
    )
SELECT id, uuid, project_id, name, slug,
    created, modified, archived
FROM testbed;
DROP TABLE testbed;
ALTER TABLE down_testbed RENAME TO testbed;
CREATE INDEX index_testbed_project_created ON testbed(project_id, created);
PRAGMA foreign_keys = on;
```

- [ ] Create migration up/down SQL
- [ ] Regenerate `lib/bencher_schema/src/schema.rs` (diesel print-schema)

### 0.2 JSON Types

**`lib/bencher_json/src/project/testbed.rs`:**
- [ ] `JsonTestbed` — add `pub spec: Option<JsonSpec>` (the current or report-specific spec)
- [ ] `JsonUpdateTestbed` — add `pub spec: Option<SpecResourceId>` (set/change/clear the assigned spec)

Note: `JsonNewTestbed` does NOT get a spec field — testbeds are created without a spec, and the spec is set later via update or `bencher run --spec`.

### 0.3 DB Model

**`lib/bencher_schema/src/model/project/testbed.rs`:**
- [ ] `QueryTestbed` — add `pub spec_id: Option<SpecId>` (between `slug` and `created` to match column order)
- [ ] `InsertTestbed` — add `pub spec_id: Option<SpecId>` (always `None` on creation)
- [ ] `UpdateTestbed` — add `pub spec_id: Option<Option<SpecId>>` (`Some(Some(id))` = set, `Some(None)` = clear)

#### JSON serialization — two methods (following the Branch/Head pattern)

1. **`into_json_for_project(self, conn, project)`** — current spec (for list/get/create/update endpoints)
   - [ ] Signature changes: currently takes only `&QueryProject`, needs `&mut DbConnection` added
   - [ ] If `self.spec_id` is `Some`, fetch `QuerySpec::get(conn, spec_id)?.into_json()` → include in `JsonTestbed { spec: Some(...) }`
   - [ ] If `None`, set `spec: None`
   - Analogous to `QueryBranch::into_json_for_project()` which fetches the current head
   - Reference: `lib/bencher_schema/src/model/project/branch/mod.rs:278-285`

2. **`get_json_for_report(conn, project, testbed_id, spec_id)`** — report-specific spec
   - [ ] Static method that takes an explicit `Option<SpecId>` (from the job record)
   - [ ] If `spec_id` is `Some`, fetch that specific spec (may differ from testbed's current spec)
   - [ ] If `None`, fall back to testbed's current spec
   - Analogous to `QueryBranch::get_json_for_report(conn, project, head_id, version_id)`
   - Reference: `lib/bencher_schema/src/model/project/branch/mod.rs:287-297`

#### `From<JsonUpdateTestbed> for UpdateTestbed`

Currently at `lib/bencher_schema/src/model/project/testbed.rs:187-203`. The `spec` field requires `conn` to resolve `SpecResourceId` → `SpecId`. Resolve in the endpoint handler (`patch_inner`) before building `UpdateTestbed`, rather than changing the `From` impl. This matches how Branch and Threshold handle child updates outside their `From` impls.

### 0.4 Testbed Endpoint Updates

**`lib/api_projects/src/testbeds.rs`:**

All 4 call sites of `into_json_for_project` need `conn`:
1. `get_ls_inner` (line 135): `testbed.into_json_for_project(conn, &query_project)` — used in list
2. `get_one_inner` (line 291): `.map(|testbed| testbed.into_json_for_project(conn, &query_project))`
3. `post_inner` (line 226): `.map(|testbed| testbed.into_json_for_project(conn, &query_project))`
4. `patch_inner` (line 355): `.map(|testbed| testbed.into_json_for_project(conn, &query_project))`

For `patch_inner`, also need to:
- [ ] If `json_testbed.spec` is `Some(spec_resource_id)`, resolve to `SpecId` via `QuerySpec::from_resource_id(conn, &spec_resource_id)`
- [ ] Set on `UpdateTestbed { spec_id: Some(Some(resolved_id)) }`
- The spec is server-scoped (not project-scoped), so `from_resource_id` uses the non-project variant

### 0.5 Update Other Callers of `into_json_for_project`

The signature change propagates to all callers. Besides testbeds.rs:

- [ ] **`lib/bencher_schema/src/model/project/threshold/mod.rs:231`**: already has `conn` in scope
- [ ] **`lib/bencher_schema/src/model/project/report/mod.rs:216`**: already has `conn` in scope
- [ ] **`lib/api_projects/src/perf/mod.rs:508`**: `testbed.into_json_for_project(project)` — needs `conn` added
- [ ] **`lib/api_projects/src/metrics.rs:208`**: `testbed.into_json_for_project(project)` — needs `conn` added

For the report case, this is where `get_json_for_report` will eventually be used (when job creation is implemented in Gap 1). For now, just pass `conn` to the existing method.

### 0.6 Test Helpers

**`lib/bencher_schema/src/test_util.rs`:**

Add a `create_spec` helper (no existing one):

```rust
pub fn create_spec(conn, spec_uuid, spec_name, spec_slug) -> i32
pub fn get_testbed_spec_id(conn, testbed_id) -> Option<i32>
pub fn set_testbed_spec(conn, testbed_id, spec_id)
pub fn clear_testbed_spec(conn, testbed_id)
```

### 0.7 Tests

Add a `#[cfg(test)] mod tests` to `lib/bencher_schema/src/model/project/testbed.rs`:

1. **`testbed_created_without_spec`** — create testbed, verify `spec_id` is `None`
2. **`testbed_assign_spec`** — create testbed + spec, set spec on testbed, verify `spec_id` matches
3. **`testbed_change_spec`** — assign spec A, then change to spec B, verify testbed points to B and spec A still exists
4. **`testbed_clear_spec`** — assign spec, clear it, verify `spec_id` is `None` again
5. **`multiple_testbeds_share_spec`** — two testbeds pointing to the same spec, verify both work
6. **`testbed_spec_on_delete_set_null`** — assign spec to testbed, delete spec, verify testbed's `spec_id` is `None` (ON DELETE SET NULL behavior)
7. **`testbed_update_preserves_spec`** — update testbed name/slug, verify spec_id unchanged
8. **`testbed_with_spec_in_threshold`** — create testbed with spec, use in threshold, verify threshold's testbed JSON includes spec
9. **`testbed_spec_query`** — verify can query testbeds filtered by spec_id (for future use)

### 0.8 Generate Types

- [ ] `cargo gen-types` (regenerate OpenAPI spec + TypeScript types)

### Step 0 Files to Modify

| File | Changes |
|------|---------|
| `lib/bencher_schema/migrations/<new>/up.sql` | Recreate testbed table with `spec_id` between `slug` and `created` |
| `lib/bencher_schema/migrations/<new>/down.sql` | Reverse migration (recreate without `spec_id`) |
| `lib/bencher_schema/src/schema.rs` | Regenerate diesel schema |
| `lib/bencher_json/src/project/testbed.rs` | Add `spec` to `JsonTestbed`, `JsonUpdateTestbed` |
| `lib/bencher_schema/src/model/project/testbed.rs` | Add `spec_id`, new `into_json_for_project(conn)` sig, `get_json_for_report()`, tests |
| `lib/bencher_schema/src/test_util.rs` | Add `create_spec`, `get_testbed_spec_id`, `set_testbed_spec`, `clear_testbed_spec` |
| `lib/api_projects/src/testbeds.rs` | Pass `conn` to `into_json_for_project`, resolve spec in `patch_inner` |
| `lib/bencher_schema/src/model/project/threshold/mod.rs` | Pass `conn` to `into_json_for_project` |
| `lib/bencher_schema/src/model/project/report/mod.rs` | Pass `conn` to `into_json_for_project` |
| `lib/api_projects/src/perf/mod.rs` | Pass `conn` to `into_json_for_project` |
| `lib/api_projects/src/metrics.rs` | Pass `conn` to `into_json_for_project` |

### Step 0 Verification

1. `cargo build` and `cargo check --no-default-features`
2. `cargo fmt && cargo clippy --no-deps --all-targets --all-features -- -Dwarnings`
3. `cargo nextest run` (existing tests pass + new testbed/spec tests pass)
4. `cargo gen-types` (API types changed)
5. Seed test: verify testbeds still serialize correctly with `spec: null`

---

## Gap 1: Job Creation in `run_post`

**Problem:** The `From<JsonNewRun> for JsonNewReport` conversion silently discards the `job` field (`lib/bencher_json/src/run.rs:88` — `job: _`). No job record is ever inserted into the database.

**Prerequisite:** Step 0 (testbed `spec_id`) must be completed first.

### Phase A: Schema & Model Foundation

#### A1. Migration: Add `default` to `spec`

New migration in `lib/bencher_schema/migrations/`:

```sql
-- spec: nullable `default` timestamp (only one spec should be default at a time)
ALTER TABLE spec ADD COLUMN "default" BIGINT;
```

Only one spec should be the default at a time (enforced in application logic, not DB constraint).

- [ ] Create migration up/down SQL
- [ ] Regenerate `lib/bencher_schema/src/schema.rs` (diesel print-schema)

#### A2. Add `url` field to `JsonRegistry`

Add `url: Url` as the **first** field in `JsonRegistry` (`lib/bencher_json/src/system/config/plus/registry.rs`). This is the API server's externally-reachable URL for OCI registry access (e.g., `https://api.bencher.dev`).

- [ ] Add `pub url: Url` to `JsonRegistry` (first field)
- [ ] Store `registry_url` on `ApiContext` (`lib/bencher_schema/src/context/mod.rs`) — `#[cfg(feature = "plus")] pub registry_url: Url`
- [ ] Wire it through `lib/bencher_config/src/config_tx.rs` / `plus.rs`
- [ ] Update documentation chunks in `services/console/src/chunks/docs-reference/server-config/` (all 9 languages)
- [ ] Update example config in `services/console/src/chunks/docs-reference/server-config/example.mdx`

#### A3. Spec model/types: add `default`

JSON types (`lib/bencher_json/src/spec/mod.rs`):
- [ ] `JsonSpec` — add `pub default: Option<DateTime>`
- [ ] `JsonNewSpec` — add `pub default: Option<bool>`
- [ ] `JsonUpdateSpec` — add `pub default: Option<bool>`

DB model (`lib/bencher_schema/src/model/spec.rs`):
- [ ] `QuerySpec` — add `pub default: Option<DateTime>`
- [ ] `InsertSpec` — add `pub default: Option<DateTime>`
- [ ] `UpdateSpec` — add `pub default: Option<Option<DateTime>>`
- [ ] `QuerySpec::into_json()` — include `default` field
- [ ] New: `QuerySpec::get_default(conn)` — query for spec where `default IS NOT NULL`
- [ ] New: `QuerySpec::clear_default(conn)` — set `default = NULL` on current default

Spec CRUD (`plus/api_specs/src/specs.rs`):
- [ ] Create: if `default == Some(true)`, clear existing default, set `default = Some(now)`
- [ ] Update: if `default == Some(true)`, clear + set; if `Some(false)`, clear this spec's default

#### A4. Wire report-specific spec in `QueryReport::into_json()`

Testbed `spec_id` and the two serialization methods (`into_json_for_project`, `get_json_for_report`) are already implemented in Step 0.

Update `QueryReport::into_json()` (`lib/bencher_schema/src/model/project/report/mod.rs`):
- [ ] Currently: `QueryTestbed::get(conn, testbed_id)?.into_json_for_project(conn, &query_project)`
- [ ] Change to: look up the job for this report (if any) → get `job.spec_id` → call `QueryTestbed::get_json_for_report(conn, &query_project, testbed_id, job_spec_id)`
- This ensures the report JSON includes the spec that was actually used, enabling the UI to navigate to the testbed page with the correct `?spec=` query param

### Phase B: Job Creation in `run_post`

#### B1. Extract job and source IP in `post_inner`

File: `lib/api_run/src/run.rs`

- [ ] Pass `headers` from `run_post` into `post_inner` (available via `rqctx.request.headers()`)
- [ ] Before `json_run.into()`, extract the job: `let new_run_job = json_run.job.take();`
- [ ] Extract source IP: `RateLimiting::remote_ip(log, headers)`, fallback to `127.0.0.1`, wrap in `SourceIp::new(ip)`
- [ ] Pass `new_run_job` and `source_ip` to `QueryReport::create`

The `job` field is already `pub`. After `.take()`, the `From` impl sees `None` and the existing `job: _` pattern match works unchanged.

#### B2. Modify `QueryReport::create` signature

File: `lib/bencher_schema/src/model/project/report/mod.rs`

- [ ] Add `#[cfg(feature = "plus")] new_run_job: Option<JsonNewRunJob>` parameter
- [ ] Add `#[cfg(feature = "plus")] source_ip: SourceIp` parameter

#### B3. Job creation logic inside `QueryReport::create`

After the report is inserted and queried back (~line 155), before results processing:

- [ ] If `new_run_job` is `Some`:
  1. Resolve spec via chain (B4)
  2. If explicit `--spec` was provided and differs from testbed, update testbed's `spec_id`
  3. Resolve image → digest (B5)
  4. Determine priority (B6)
  5. Build `JsonJobConfig` (registry_url, project UUID, digest, entrypoint, cmd, env, timeout, file_paths)
  6. `InsertJob::new(...)` and `diesel::insert_into(job::table)`
  7. Skip results processing (results array is empty for job runs)
- [ ] If `new_run_job` is `None`: process results as before

`organization_id` available via `query_project.organization_id`. `plan_kind` already computed earlier in the function.

#### B4. Spec resolution chain

- [ ] Implement `resolve_spec(conn, new_run_job, testbed_id)`:
  1. If `new_run_job.spec` is `Some` → `QuerySpec::from_resource_id(conn, id)` → return `spec.id`
  2. Else if testbed has `spec_id` → return that
  3. Else → `QuerySpec::get_default(conn)` → return `spec.id`
  4. If none found → error: "No spec provided and no default spec configured"

#### B5. Image resolution

- [ ] Implement `resolve_image(context, project, image_ref)`:
  1. If `image.registry() == "docker.io"` (unqualified name → local registry):
     - Get `registry_url` from `context.registry_url`
     - If `image.is_digest()`: parse `image.reference()` as `ImageDigest` directly
     - If tag: parse as `bencher_oci_storage::Tag`, call `context.oci_storage().resolve_tag(&project_uuid, &tag)`, convert `Digest` → `ImageDigest` via `digest.as_str().parse()`
  2. If external registry → error: "External registries not yet supported"
  3. Return `(registry_url, image_digest)`

#### B6. Priority determination

- [ ] Implement `determine_priority(plan_kind, is_claimed)`:
  - Unclaimed org → `JobPriority::Unclaimed`
  - `PlanKind::None` → `JobPriority::Free`
  - `PlanKind::Metered` → `JobPriority::Team`
  - `PlanKind::Licensed` → `JobPriority::Team`

### Key types already implemented

- `InsertJob::new()` in `lib/bencher_schema/src/model/runner/job.rs`
- `JsonNewRunJob` in `lib/bencher_json/src/runner/job.rs`
- `JsonJobConfig` in `lib/bencher_json/src/runner/job.rs`
- `ImageReference` in `lib/bencher_valid/src/plus/image_reference.rs`
- `ImageDigest` in `lib/bencher_valid/src/image_digest.rs`
- `OciStorage::resolve_tag()` in `plus/bencher_oci_storage/src/storage.rs`
- `QuerySpec::from_resource_id()` in `lib/bencher_schema/src/model/spec.rs`
- `PlanKind` in `lib/bencher_schema/src/model/organization/plan.rs`
- `SourceIp` in `lib/bencher_schema/src/model/runner/source_ip.rs`
- `RateLimiting::remote_ip()` in `lib/bencher_schema/src/context/rate_limiting.rs`

### Files to modify

| File | Changes |
|------|---------|
| `lib/bencher_schema/migrations/<new>/up.sql` | Migration: spec.default |
| `lib/bencher_schema/migrations/<new>/down.sql` | Reverse migration |
| `lib/bencher_schema/src/schema.rs` | Regenerate diesel schema |
| `lib/bencher_json/src/system/config/plus/registry.rs` | Add `url: Url` first field |
| `lib/bencher_json/src/spec/mod.rs` | Add `default` to all 3 spec types |
| `lib/bencher_schema/src/model/spec.rs` | Add `default`, get_default(), clear_default() |
| `plus/api_specs/src/specs.rs` | Handle default in create/update |
| `lib/api_run/src/run.rs` | Extract job + source_ip, pass headers |
| `lib/bencher_schema/src/model/project/report/mod.rs` | Job creation, conditional results processing, report-specific spec via `get_json_for_report()` |
| `lib/bencher_schema/src/context/mod.rs` | Add `registry_url: Url` to ApiContext |
| `lib/bencher_config/src/config_tx.rs` or `plus.rs` | Wire registry_url to ApiContext |
| `services/console/src/chunks/docs-reference/server-config/` | Registry URL docs (9 languages) |

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

0. **Step 0 (testbed spec_id)** — Standalone prerequisite. Adds `spec_id` FK to testbed, JSON types, endpoint updates, and tests.
1. **Gap 1 (job creation)** — Without this, no jobs enter the queue and the runner has nothing to claim. Depends on Step 0.
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
