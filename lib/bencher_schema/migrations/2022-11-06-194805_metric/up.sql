PRAGMA foreign_keys = off;
DROP TABLE alert;
DROP TABLE resource;
DROP TABLE throughput;
DROP TABLE latency;
DROP TABLE threshold;
CREATE TABLE up_perf (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_id INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    FOREIGN KEY (report_id) REFERENCES report (id),
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id),
    UNIQUE(report_id, iteration, benchmark_id)
);
INSERT INTO up_perf(
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
ALTER TABLE up_perf
    RENAME TO perf;
CREATE TABLE metric (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    perf_id INTEGER NOT NULL,
    metric_kind_id INTEGER NOT NULL,
    value DOUBLE NOT NULL,
    lower_bound DOUBLE,
    upper_bound DOUBLE,
    FOREIGN KEY (perf_id) REFERENCES perf (id),
    FOREIGN KEY (metric_kind_id) REFERENCES metric_kind (id),
    UNIQUE(perf_id, metric_kind_id)
);
CREATE TABLE threshold (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    branch_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    metric_kind_id INTEGER NOT NULL,
    statistic_id INTEGER NOT NULL,
    FOREIGN KEY (branch_id) REFERENCES branch (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id),
    FOREIGN KEY (metric_kind_id) REFERENCES metric_kind (id),
    FOREIGN KEY (statistic_id) REFERENCES statistic (id),
    UNIQUE(branch_id, testbed_id, metric_kind_id)
);
CREATE TABLE alert (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    perf_id INTEGER NOT NULL,
    threshold_id INTEGER NOT NULL,
    statistic_id INTEGER NOT NULL,
    side BOOLEAN NOT NULL,
    boundary REAL NOT NULL,
    outlier REAL NOT NULL,
    FOREIGN KEY (perf_id) REFERENCES perf (id),
    FOREIGN KEY (threshold_id) REFERENCES threshold (id),
    FOREIGN KEY (statistic_id) REFERENCES statistic (id)
);
PRAGMA foreign_keys = on;