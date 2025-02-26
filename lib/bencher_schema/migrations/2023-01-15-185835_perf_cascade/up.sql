PRAGMA foreign_keys = off;
CREATE TABLE up_perf (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_id INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id) ON DELETE CASCADE,
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
PRAGMA foreign_keys = on;
-- Delete the benchmarks with an empty name as this is no longer going to serialize
DELETE FROM benchmark
WHERE TRIM(name) = '';