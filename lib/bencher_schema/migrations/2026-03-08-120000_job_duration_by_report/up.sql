CREATE TABLE job_duration_by_report (
    report_id INTEGER PRIMARY KEY NOT NULL,
    job_duration INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE
);

-- Backfill from existing job data (Completed=3, Processed=4)
INSERT INTO job_duration_by_report (report_id, job_duration)
SELECT j.report_id, (j.completed - j.started)
FROM job j
WHERE j.started IS NOT NULL
  AND j.completed IS NOT NULL
  AND j.status IN (3, 4);
