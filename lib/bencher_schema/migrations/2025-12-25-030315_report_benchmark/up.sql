DROP INDEX IF EXISTS index_report_benchmark_benchmark;
DROP INDEX IF EXISTS index_report_benchmark_benchmark_report;
DROP INDEX IF EXISTS index_report_project_end_time;
CREATE INDEX index_report_benchmark_benchmark ON report_benchmark(benchmark_id);
CREATE INDEX index_report_benchmark_benchmark_report ON report_benchmark(benchmark_id, report_id);
CREATE INDEX index_report_project_end_time ON report(project_id, end_time);