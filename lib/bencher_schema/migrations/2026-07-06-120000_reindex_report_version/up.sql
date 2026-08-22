-- Rebuild all indexes on the report, version, and branch tables.
-- Long-lived databases can carry index corruption (wrong # of entries in index)
-- that silently drops rows from index-driven queries.
-- The corruption found in July reached the report indexes, the version unique
-- index, and index_branch_head, so all three tables are rebuilt. Naming the
-- table rather than the index rebuilds every index on it, including the
-- implicit UNIQUE autoindexes.
-- REINDEX is a no-op when the indexes are already consistent.
REINDEX report;
REINDEX version;
REINDEX branch;
-- Remove report_benchmark rows orphaned by deletes performed before
-- ON DELETE CASCADE was enforced (see 2023-01-15-185835_perf_cascade),
-- along with their dependent rows.
-- Migrations run with foreign_keys = OFF, so ON DELETE CASCADE does not
-- fire here and the delete chain must be explicit, bottom-up:
-- alert -> boundary -> metric -> report_benchmark.
-- The orphaned ids are collected into temp tables up front so the deletes all
-- work from one stable set, rather than each statement re-running the same
-- multi-table join over tables an earlier delete has already changed.
CREATE TEMPORARY TABLE orphan_report_benchmark AS
SELECT id
FROM report_benchmark
WHERE benchmark_id NOT IN (
        SELECT id
        FROM benchmark
    );
CREATE TEMPORARY TABLE orphan_metric AS
SELECT id
FROM metric
WHERE report_benchmark_id IN (
        SELECT id
        FROM orphan_report_benchmark
    );
CREATE TEMPORARY TABLE orphan_boundary AS
SELECT id
FROM boundary
WHERE metric_id IN (
        SELECT id
        FROM orphan_metric
    );
DELETE FROM alert
WHERE boundary_id IN (
        SELECT id
        FROM orphan_boundary
    );
DELETE FROM boundary
WHERE id IN (
        SELECT id
        FROM orphan_boundary
    );
DELETE FROM metric
WHERE id IN (
        SELECT id
        FROM orphan_metric
    );
DELETE FROM report_benchmark
WHERE id IN (
        SELECT id
        FROM orphan_report_benchmark
    );
DROP TABLE orphan_boundary;
DROP TABLE orphan_metric;
DROP TABLE orphan_report_benchmark;
