DROP INDEX IF EXISTS index_job_in_flight;
-- Index for in-flight job recovery queries: Claimed (1), Running (2), and Unknown (7)
CREATE INDEX index_job_in_flight ON job(status)
WHERE status = 1 OR status = 2 OR status = 7;
