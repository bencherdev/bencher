-- parameter
-- The reverse of the renames, with the same three indexes rebuilt under their prior names.
ALTER TABLE series_last_seen
    RENAME COLUMN variant_id TO parameter_id;
ALTER TABLE report_benchmark
    RENAME COLUMN variant_id TO parameter_id;
ALTER TABLE variant
    RENAME COLUMN parameters TO "set";
ALTER TABLE variant
    RENAME TO parameter;
DROP INDEX index_report_benchmark_report_iteration_benchmark_variant;
CREATE UNIQUE INDEX index_report_benchmark_report_iteration_benchmark_parameter ON report_benchmark(report_id, iteration, benchmark_id, parameter_id);
DROP INDEX index_report_benchmark_variant;
CREATE INDEX index_report_benchmark_parameter ON report_benchmark(parameter_id);
DROP INDEX index_series_last_seen_variant;
CREATE INDEX index_series_last_seen_parameter ON series_last_seen (parameter_id);
