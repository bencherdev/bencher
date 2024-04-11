PRAGMA foreign_keys = off;
-- report_benchmark
CREATE TABLE up_report_benchmark (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_id INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id),
    UNIQUE(report_id, iteration, benchmark_id)
);
INSERT INTO up_report_benchmark(
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
FROM perf;
DROP TABLE perf;
ALTER TABLE up_report_benchmark
    RENAME TO report_benchmark;
-- metric
CREATE TABLE up_metric (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_benchmark_id INTEGER NOT NULL,
    measure_id INTEGER NOT NULL,
    value DOUBLE NOT NULL,
    lower_value DOUBLE,
    upper_value DOUBLE,
    FOREIGN KEY (report_benchmark_id) REFERENCES report_benchmark (id) ON DELETE CASCADE,
    FOREIGN KEY (measure_id) REFERENCES measure (id),
    UNIQUE(report_benchmark_id, measure_id)
);
INSERT INTO up_metric(
        id,
        uuid,
        report_benchmark_id,
        measure_id,
        value,
        lower_value,
        upper_value
    )
SELECT id,
    uuid,
    perf_id,
    measure_id,
    value,
    lower_value,
    lower_value
FROM metric;
DROP TABLE metric;
ALTER TABLE up_metric
    RENAME TO metric;
PRAGMA foreign_keys = on;