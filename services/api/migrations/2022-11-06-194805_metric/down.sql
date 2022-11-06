PRAGMA foreign_keys = off;
DROP TABLE metric;
DROP TABLE perf;
CREATE TABLE latency (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    lower_variance BIGINT NOT NULL,
    upper_variance BIGINT NOT NULL,
    duration BIGINT NOT NULL
);
CREATE TABLE throughput (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    lower_variance DOUBLE NOT NULL,
    upper_variance DOUBLE NOT NULL,
    events DOUBLE NOT NULL,
    unit_time BIGINT NOT NULL
);
CREATE TABLE resource (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    min DOUBLE NOT NULL,
    max DOUBLE NOT NULL,
    avg DOUBLE NOT NULL
);
CREATE TABLE perf (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_id INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    -- at least one should not be null
    latency_id INTEGER,
    throughput_id INTEGER,
    compute_id INTEGER,
    memory_id INTEGER,
    storage_id INTEGER,
    --
    FOREIGN KEY (report_id) REFERENCES report (id),
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id),
    FOREIGN KEY (latency_id) REFERENCES latency (id),
    FOREIGN KEY (throughput_id) REFERENCES throughput (id),
    FOREIGN KEY (compute_id) REFERENCES resource (id),
    FOREIGN KEY (memory_id) REFERENCES resource (id),
    FOREIGN KEY (storage_id) REFERENCES resource (id),
    UNIQUE(report_id, iteration, benchmark_id)
);
PRAGMA foreign_keys = on;