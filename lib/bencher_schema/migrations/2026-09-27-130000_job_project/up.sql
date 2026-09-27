PRAGMA foreign_keys = off;
-- job
-- A job gains its report's project, so a project's jobs are read by project. A job without
-- a report reads a NULL project, which fails the copy rather than dropping the job.
CREATE TABLE up_job (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL,
    organization_id INTEGER NOT NULL,
    project_id INTEGER NOT NULL,
    report_id INTEGER NOT NULL,
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
    FOREIGN KEY (organization_id) REFERENCES organization (id) ON DELETE CASCADE,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE,
    FOREIGN KEY (spec_id) REFERENCES spec (id) ON DELETE RESTRICT,
    FOREIGN KEY (runner_id) REFERENCES runner (id) ON DELETE RESTRICT
);
INSERT INTO up_job(
        id,
        uuid,
        organization_id,
        project_id,
        report_id,
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
SELECT job.id,
    job.uuid,
    job.organization_id,
    (
        SELECT report.project_id
        FROM report
        WHERE report.id = job.report_id
    ),
    job.report_id,
    job.source_ip,
    job.spec_id,
    job.config,
    job.timeout,
    job.priority,
    job.status,
    job.runner_id,
    job.claimed,
    job.started,
    job.completed,
    job.last_heartbeat,
    job.last_billed_minute,
    job.created,
    job.modified
FROM job
ORDER BY job.id;
DROP TABLE job;
ALTER TABLE up_job
    RENAME TO job;
CREATE UNIQUE INDEX index_job_uuid ON job(uuid);
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
-- A project's jobs a page at a time in either direction, ties in creation time broken by id.
CREATE INDEX index_job_project_created ON job(project_id, created, id);
-- The same for one status, so a status filter walks only its own jobs and its count reads
-- only those; the status lives here alone, so a status change moves one entry.
CREATE INDEX index_job_project_status_created ON job(project_id, status, created, id);
PRAGMA foreign_keys = on;
