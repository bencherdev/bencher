-- Redundant with index_report_benchmark_benchmark_report(benchmark_id, report_id)
DROP INDEX IF EXISTS index_report_benchmark_benchmark;
-- Redundant with the UNIQUE(report_id, iteration, benchmark_id) autoindex
DROP INDEX IF EXISTS index_report_benchmark;
