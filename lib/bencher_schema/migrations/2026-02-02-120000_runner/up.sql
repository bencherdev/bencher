-- Runner: server-scoped, serves all projects
CREATE TABLE runner (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    token_hash TEXT NOT NULL,
    state INTEGER NOT NULL DEFAULT 0,

    archived BIGINT,
    last_heartbeat BIGINT,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL
);
-- Spec: server-scoped resource requirements for jobs
CREATE TABLE spec (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    cpu INTEGER NOT NULL,
    memory BIGINT NOT NULL,
    disk BIGINT NOT NULL,
    network BOOLEAN NOT NULL DEFAULT 0,
    archived BIGINT,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL
);
-- Runner-Spec: many-to-many association
CREATE TABLE runner_spec (
    id INTEGER PRIMARY KEY NOT NULL,
    runner_id INTEGER NOT NULL,
    spec_id INTEGER NOT NULL,
    FOREIGN KEY (runner_id) REFERENCES runner (id) ON DELETE CASCADE,
    FOREIGN KEY (spec_id) REFERENCES spec (id) ON DELETE RESTRICT,
    UNIQUE (runner_id, spec_id)
);
CREATE INDEX index_runner_spec_runner_id ON runner_spec(runner_id);
CREATE INDEX index_runner_spec_spec_id ON runner_spec(spec_id);
-- Job status enum (stored as integer)
-- 0 = pending
-- 1 = claimed
-- 2 = running
-- 3 = completed
-- 4 = failed
-- 5 = canceled
-- Job: tied to a report, executed by a runner
-- Priority tiers: 0=unclaimed, 100=free, 200=team, 300=enterprise
CREATE TABLE job (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_id INTEGER NOT NULL,
    organization_id INTEGER NOT NULL,
    source_ip TEXT NOT NULL,
    spec_id INTEGER NOT NULL,
    config TEXT NOT NULL,
    timeout INTEGER NOT NULL DEFAULT 3600,
    priority INTEGER NOT NULL DEFAULT 0,
    status INTEGER NOT NULL DEFAULT 0,
    runner_id INTEGER,
    claimed BIGINT,
    started BIGINT,
    completed BIGINT,
    last_heartbeat BIGINT,
    last_billed_minute INTEGER,
    exit_code INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE,
    FOREIGN KEY (organization_id) REFERENCES organization (id) ON DELETE CASCADE,
    FOREIGN KEY (spec_id) REFERENCES spec (id) ON DELETE RESTRICT,
    FOREIGN KEY (runner_id) REFERENCES runner (id) ON DELETE RESTRICT
);
-- Index for job claiming (ordered by priority, then FIFO)
CREATE INDEX index_job_pending ON job(status, priority DESC, created ASC)
WHERE status = 0;
-- Indexes for concurrency limit checks (used in claim subqueries)
-- Cover both Claimed (1) and Running (2) statuses for in-flight job checks
CREATE INDEX index_job_org_in_flight ON job(organization_id)
WHERE status = 1 OR status = 2;
CREATE INDEX index_job_source_ip_in_flight ON job(source_ip)
WHERE status = 1 OR status = 2;
-- Index for in-flight job recovery queries
CREATE INDEX index_job_in_flight ON job(status)
WHERE status = 1 OR status = 2;
CREATE INDEX index_job_runner_id ON job(runner_id)
WHERE runner_id IS NOT NULL;
CREATE INDEX index_job_spec_id ON job(spec_id);
CREATE UNIQUE INDEX index_runner_token_hash ON runner(token_hash);
CREATE INDEX index_job_report_id ON job(report_id);