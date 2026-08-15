PRAGMA foreign_keys = off;
-- report_benchmark
CREATE TABLE down_report_benchmark (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_id INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id),
    UNIQUE(report_id, iteration, benchmark_id)
);
INSERT INTO down_report_benchmark(
        id,
        uuid,
        report_id,
        iteration,
        benchmark_id
    )
SELECT id,
    uuid,
    report_id,
    iteration,
    benchmark_id
FROM report_benchmark;
DROP TABLE report_benchmark;
ALTER TABLE down_report_benchmark
    RENAME TO report_benchmark;
CREATE INDEX index_report_benchmark_benchmark_report ON report_benchmark(benchmark_id, report_id);
-- parameter
DROP TABLE parameter;
PRAGMA foreign_keys = on;
