-- parameter
-- The column holds the parameter set itself, so it is named for what it is:
-- `parameter.set`, not `parameter.parameters`.
--
-- `set` is an SQL keyword, so every reference to it is quoted.
--
-- This is a metadata only rename. SQLite rewrites the stored DDL in place,
-- including `UNIQUE(benchmark_id, "set")` and the index that backs it,
-- so no row is read and no table is recreated.
ALTER TABLE parameter
    RENAME COLUMN parameters TO "set";
