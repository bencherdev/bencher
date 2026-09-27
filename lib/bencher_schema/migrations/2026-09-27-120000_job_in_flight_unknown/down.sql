DROP INDEX IF EXISTS index_job_in_flight;
CREATE INDEX index_job_in_flight ON job(status)
WHERE status = 1 OR status = 2;
