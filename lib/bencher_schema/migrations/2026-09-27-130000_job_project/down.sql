PRAGMA foreign_keys = off;
DROP INDEX IF EXISTS index_job_project_created;
DROP INDEX IF EXISTS index_job_project_status_created;
CREATE TABLE down_job (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_id INTEGER NOT NULL,
    organization_id INTEGER NOT NULL,
    source_ip TEXT NOT NULL,
    spec_id INTEGER NOT NULL,
    config TEXT NOT NULL,
    timeout INTEGER NOT NULL DEFAULT 3600 CHECK (timeout > 0),
    priority INTEGER NOT NULL DEFAULT 0 CHECK (priority >= 0),
    status INTEGER NOT NULL DEFAULT 0 CHECK (status >= 0),
    runner_id INTEGER,
    claimed BIGINT,
    started BIGINT,
    completed BIGINT,
    last_heartbeat BIGINT,
    last_billed_minute INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE,
    FOREIGN KEY (organization_id) REFERENCES organization (id) ON DELETE CASCADE,
    FOREIGN KEY (spec_id) REFERENCES spec (id) ON DELETE RESTRICT,
    FOREIGN KEY (runner_id) REFERENCES runner (id) ON DELETE RESTRICT
);
INSERT INTO down_job(
        id,
        uuid,
        report_id,
        organization_id,
        source_ip,
        spec_id,
        config,
        timeout,
        priority,
        status,
        runner_id,
        claimed,
        started,
        completed,
        last_heartbeat,
        last_billed_minute,
        created,
        modified
    )
SELECT id,
    uuid,
    report_id,
    organization_id,
    source_ip,
    spec_id,
    config,
    timeout,
    priority,
    status,
    runner_id,
    claimed,
    started,
    completed,
    last_heartbeat,
    last_billed_minute,
    created,
    modified
FROM job;
DROP TABLE job;
ALTER TABLE down_job
    RENAME TO job;
CREATE INDEX index_job_pending ON job(status, priority DESC, created ASC)
WHERE status = 0;
CREATE INDEX index_job_org_in_flight ON job(organization_id)
WHERE status = 1
    OR status = 2;
CREATE INDEX index_job_source_ip_in_flight ON job(source_ip)
WHERE status = 1
    OR status = 2;
CREATE INDEX index_job_in_flight ON job(status)
WHERE status = 1
    OR status = 2
    OR status = 7;
CREATE INDEX index_job_runner_id ON job(runner_id)
WHERE runner_id IS NOT NULL;
CREATE INDEX index_job_spec_id ON job(spec_id);
CREATE INDEX index_job_report_id ON job(report_id);
CREATE INDEX index_job_completed ON job(status)
WHERE status = 3;
PRAGMA foreign_keys = on;
