-- variant
-- A benchmark with one set of parameters is a variant, and the map it carries is
-- its parameters: `variant.parameters`.
--
-- The table and column renames are metadata only. SQLite rewrites the stored DDL in
-- place, including the foreign keys that reference the table, the primary key of
-- `series_last_seen`, and `UNIQUE(benchmark_id, parameters)`.
-- SQLite cannot rename an index, so the three named indexes over `variant_id` are
-- rebuilt, which is the only step that reads rows.
ALTER TABLE parameter
    RENAME TO variant;
ALTER TABLE variant
    RENAME COLUMN "set" TO parameters;
ALTER TABLE report_benchmark
    RENAME COLUMN parameter_id TO variant_id;
ALTER TABLE series_last_seen
    RENAME COLUMN parameter_id TO variant_id;
DROP INDEX index_report_benchmark_report_iteration_benchmark_parameter;
CREATE UNIQUE INDEX index_report_benchmark_report_iteration_benchmark_variant ON report_benchmark(report_id, iteration, benchmark_id, variant_id);
DROP INDEX index_report_benchmark_parameter;
CREATE INDEX index_report_benchmark_variant ON report_benchmark(variant_id);
DROP INDEX index_series_last_seen_parameter;
CREATE INDEX index_series_last_seen_variant ON series_last_seen (variant_id);
