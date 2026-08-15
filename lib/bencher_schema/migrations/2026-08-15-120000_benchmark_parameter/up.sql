PRAGMA foreign_keys = off;
-- parameter
-- `parameters` holds the RFC 8785 (JCS) canonical form of the parameter set,
-- so `UNIQUE(benchmark_id, parameters)` is the enforcement point for canonical
-- equality. It is declared `TEXT` to match the SQLite representation of Diesel's
-- `Json` SQL type; SQLite's JSON functions read canonical JSON text directly.
CREATE TABLE parameter (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    benchmark_id INTEGER NOT NULL,
    parameters TEXT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    archived BIGINT,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id) ON DELETE CASCADE,
    UNIQUE(benchmark_id, parameters)
);
CREATE INDEX index_parameter_benchmark ON parameter(benchmark_id);
-- Every benchmark is born with its empty parameter set, so every benchmark that
-- predates this migration is backfilled with one.
-- Pure SQL has no UUIDv7 function, so the UUID is a v4 minted from `randomblob`:
-- 16 random bytes with the version nibble set to 4 and the variant nibble drawn
-- from `89ab`. `random() & 3` is used rather than `abs(random()) % 4` because
-- `abs(-9223372036854775808)` is an integer overflow error in SQLite.
INSERT INTO parameter(uuid, benchmark_id, parameters, created, modified)
SELECT lower(
        hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)), 2) || '-' || substr('89ab', (random() & 3) + 1, 1) || substr(hex(randomblob(2)), 2) || '-' || hex(randomblob(6))
    ),
    id,
    '{}',
    created,
    modified
FROM benchmark;
-- report_benchmark
-- `parameter_id` is NOT NULL: SQLite unique indexes treat NULLs as distinct, so a
-- nullable dimension would silently void `UNIQUE(report_id, iteration, benchmark_id, parameter_id)`.
CREATE TABLE up_report_benchmark (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_id INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    parameter_id INTEGER NOT NULL,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id),
    FOREIGN KEY (parameter_id) REFERENCES parameter (id),
    UNIQUE(report_id, iteration, benchmark_id, parameter_id)
);
INSERT INTO up_report_benchmark(
        id,
        uuid,
        report_id,
        iteration,
        benchmark_id,
        parameter_id
    )
SELECT report_benchmark.id,
    report_benchmark.uuid,
    report_benchmark.report_id,
    report_benchmark.iteration,
    report_benchmark.benchmark_id,
    parameter.id
FROM report_benchmark
    INNER JOIN parameter ON (
        parameter.benchmark_id = report_benchmark.benchmark_id
        AND parameter.parameters = '{}'
    );
DROP TABLE report_benchmark;
ALTER TABLE up_report_benchmark
    RENAME TO report_benchmark;
CREATE INDEX index_report_benchmark_benchmark_report ON report_benchmark(benchmark_id, report_id);
PRAGMA foreign_keys = on;
