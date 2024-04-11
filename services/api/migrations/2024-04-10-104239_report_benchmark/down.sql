PRAGMA foreign_keys = off;
-- perf
CREATE TABLE down_perf (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_id INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id),
    UNIQUE(report_id, iteration, benchmark_id)
);
INSERT INTO down_perf(
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
ALTER TABLE down_perf
    RENAME TO perf;
-- metric
CREATE TABLE down_metric (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    perf_id INTEGER NOT NULL,
    measure_id INTEGER NOT NULL,
    value DOUBLE NOT NULL,
    lower_value DOUBLE,
    upper_value DOUBLE,
    FOREIGN KEY (perf_id) REFERENCES perf (id) ON DELETE CASCADE,
    FOREIGN KEY (measure_id) REFERENCES measure (id),
    UNIQUE(perf_id, measure_id)
);
INSERT INTO down_metric(
        id,
        uuid,
        perf_id,
        measure_id,
        value,
        lower_value,
        upper_value
    )
SELECT id,
    uuid,
    report_benchmark_id,
    measure_id,
    value,
    lower_value,
    lower_value
FROM metric;
DROP TABLE metric;
ALTER TABLE down_metric
    RENAME TO metric;
PRAGMA foreign_keys = on;