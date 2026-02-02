-- Runner: server-scoped, serves all projects
CREATE TABLE runner (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    token_hash TEXT NOT NULL,
    state INTEGER NOT NULL DEFAULT 0,
    locked BIGINT,
    archived BIGINT,
    last_heartbeat BIGINT,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL
);

-- Job status enum (stored as integer)
-- 0 = pending
-- 1 = claimed
-- 2 = running
-- 3 = completed
-- 4 = failed
-- 5 = canceled

-- Job: tied to a report, executed by a runner
CREATE TABLE job (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_id INTEGER NOT NULL,
    status INTEGER NOT NULL DEFAULT 0,
    spec TEXT NOT NULL,
    timeout INTEGER NOT NULL DEFAULT 3600,
    priority INTEGER NOT NULL DEFAULT 0,
    runner_id INTEGER,
    claimed BIGINT,
    started BIGINT,
    completed BIGINT,
    last_heartbeat BIGINT,
    last_billed_minute INTEGER DEFAULT 0,
    exit_code INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE,
    FOREIGN KEY (runner_id) REFERENCES runner (id) ON DELETE RESTRICT
);

-- Index for job claiming (ordered by priority, then FIFO)
CREATE INDEX index_job_pending ON job(status, priority DESC, created ASC) WHERE status = 0;
