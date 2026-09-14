-- Drop the query planner statistics tables.
-- The 2026-08-17 migration ran a limited ANALYZE, and the sampled statistics
-- overstate how many rows share a key on indexes whose leading rows cluster on
-- one value. Trusting them, the planner scans the whole alert table for a status
-- filtered alerts list and walks every head version for a perf query.
-- A full ANALYZE does not help: it keeps the alert scan and adds scans elsewhere.
-- Without statistics the planner falls back on its structural heuristics, which
-- pick the indexes, as it did before the statistics were introduced.
-- IF EXISTS because a database that never ran ANALYZE has neither table, and a
-- build without ENABLE_STAT4 never creates sqlite_stat4.
DROP TABLE IF EXISTS sqlite_stat1;
DROP TABLE IF EXISTS sqlite_stat4;
