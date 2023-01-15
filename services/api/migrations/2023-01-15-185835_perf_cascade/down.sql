PRAGMA foreign_keys = off;
CREATE TABLE down_perf (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_id INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    FOREIGN KEY (report_id) REFERENCES report (id),
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
FROM perf;
DROP TABLE perf;
ALTER TABLE down_perf
    RENAME TO perf;
PRAGMA foreign_keys = on;