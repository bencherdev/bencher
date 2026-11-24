-- One row per run of a job's image that the server sent to a runner.
-- `timeout` is the budget the execution was sent with.
-- `status`: 0 sent, 1 running, 2 completed, 3 failed, 4 canceled, 5 unknown.
CREATE TABLE job_execution (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    job_id INTEGER NOT NULL,
    report_id INTEGER NOT NULL,
    iteration INTEGER NOT NULL CHECK (iteration >= 0),
    runner_id INTEGER NOT NULL,
    timeout INTEGER NOT NULL CHECK (timeout > 0),
    status INTEGER NOT NULL CHECK (status >= 0),
    sent BIGINT NOT NULL,
    started BIGINT,
    ended BIGINT,
    last_heartbeat BIGINT NOT NULL,
    FOREIGN KEY (job_id) REFERENCES job (id) ON DELETE CASCADE,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE,
    FOREIGN KEY (runner_id) REFERENCES runner (id) ON DELETE RESTRICT,
    UNIQUE(job_id, report_id, iteration)
);
CREATE INDEX index_job_execution_report_id ON job_execution(report_id);
CREATE INDEX index_job_execution_runner_id ON job_execution(runner_id);
CREATE INDEX index_job_execution_in_flight ON job_execution(status)
WHERE status = 0 OR status = 1 OR status = 5;

-- A job in flight was sent whole, as 1 execution under the job's own UUID, which its runner
-- echoes in its result. That execution holds every iteration of its job.
INSERT INTO job_execution (
    uuid, job_id, report_id, iteration, runner_id, timeout, status, sent, started,
    last_heartbeat
)
SELECT uuid, id, report_id, 0, runner_id, timeout,
    CASE status WHEN 1 THEN 0 WHEN 2 THEN 1 ELSE 5 END,
    COALESCE(claimed, created), started, COALESCE(last_heartbeat, claimed, created)
FROM job
WHERE (status = 1 OR status = 2 OR status = 7) AND runner_id IS NOT NULL;
