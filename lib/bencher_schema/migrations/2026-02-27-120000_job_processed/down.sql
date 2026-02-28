DROP INDEX IF EXISTS index_job_completed;

-- Remap Processed back to Completed (no old equivalent).
UPDATE job SET status = 3 WHERE status = 4;  -- Processed: 4 -> 3 (Completed)

-- Reverse the status re-mapping (ascending order to avoid collisions).
UPDATE job SET status = 4 WHERE status = 5;  -- Failed:   5 -> 4
UPDATE job SET status = 5 WHERE status = 6;  -- Canceled: 6 -> 5
