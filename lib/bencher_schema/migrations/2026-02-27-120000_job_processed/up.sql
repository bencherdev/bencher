-- Re-map existing statuses to make room for Processed (4).
-- Must update in reverse order to avoid collisions.
UPDATE job SET status = 6 WHERE status = 5;  -- Canceled: 5 -> 6
UPDATE job SET status = 5 WHERE status = 4;  -- Failed:   4 -> 5

-- Index for startup recovery: find jobs in Completed state that need reprocessing.
-- status = 3 matches JobStatus::Completed (see lib/bencher_json/src/runner/job_status.rs)
CREATE INDEX index_job_completed ON job(status) WHERE status = 3;
