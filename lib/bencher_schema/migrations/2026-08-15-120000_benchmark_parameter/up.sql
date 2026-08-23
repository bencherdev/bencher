PRAGMA foreign_keys = off;
-- parameter
-- `parameters` holds the SQLite JSONB encoding of the RFC 8785 (JCS) canonical
-- form of the parameter set, so `UNIQUE(benchmark_id, parameters)` is the
-- enforcement point for canonical equality. It is declared `BLOB` to match the
-- SQLite representation of the `Jsonb` SQL type, and SQLite's JSON functions
-- read it without a parse step.
CREATE TABLE parameter (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    benchmark_id INTEGER NOT NULL,
    parameters BLOB NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    archived BIGINT,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id) ON DELETE CASCADE,
    UNIQUE(benchmark_id, parameters)
);
-- Every benchmark is born with its empty parameter set, so every benchmark that
-- predates this migration is backfilled with one.
-- Pure SQL has no UUIDv7 function, so the UUID is a v4 minted from `randomblob`:
-- 16 random bytes with the version nibble set to 4 and the variant nibble drawn
-- from `89ab`. `random() & 3` is used rather than `abs(random()) % 4` because
-- `abs(-9223372036854775808)` is an integer overflow error in SQLite.
-- The empty set is minted with `jsonb()` so that SQLite itself defines the bytes
-- the encoder in `bencher_json` has to reproduce.
INSERT INTO parameter(uuid, benchmark_id, parameters, created, modified)
SELECT lower(
        hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)), 2) || '-' || substr('89ab', (random() & 3) + 1, 1) || substr(hex(randomblob(2)), 2) || '-' || hex(randomblob(6))
    ),
    id,
    jsonb('{}'),
    created,
    modified
FROM benchmark;
-- report_benchmark
-- `parameter_id` is NOT NULL: SQLite unique indexes treat NULLs as distinct, so a
-- nullable dimension would silently void the unique key over
-- (report_id, iteration, benchmark_id, parameter_id).
-- The table is declared without its two unique keys. They are built below, once the
-- backfill has landed, as named unique indexes that enforce exactly what the table
-- constraints enforced.
CREATE TABLE up_report_benchmark (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL,
    report_id INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    parameter_id INTEGER NOT NULL,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id),
    FOREIGN KEY (parameter_id) REFERENCES parameter (id)
);
-- The join is a `LEFT JOIN` so that a `report_benchmark` row whose benchmark has
-- no empty parameter set trips the `NOT NULL` on `parameter_id` and fails the
-- migration. Every benchmark is backfilled above, so this cannot fire on valid
-- data; an `INNER JOIN` would drop such a row silently instead.
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
    LEFT JOIN parameter ON (
        parameter.benchmark_id = report_benchmark.benchmark_id
        AND parameter.parameters = jsonb('{}')
    );
DROP TABLE report_benchmark;
ALTER TABLE up_report_benchmark
    RENAME TO report_benchmark;
-- The two unique keys the table was declared without, built now that every row is
-- in place. Declared on the table they would be maintained online, one page split
-- at a time, across every insert the backfill makes, and the preserved uuids are
-- v4, so each insert lands on a random page of the uuid index: at the 64 MiB cache
-- the connection serves from, those random reads and writes rather than the scan
-- set the pace, and they are the dominant cost of the rebuild. Built here they are
-- two external sorts, which are indifferent to where the keys fall, so the rebuild
-- floors at the scan that copies the rows.
CREATE UNIQUE INDEX index_report_benchmark_uuid ON report_benchmark(uuid);
CREATE UNIQUE INDEX index_report_benchmark_report_iteration_benchmark_parameter ON report_benchmark(report_id, iteration, benchmark_id, parameter_id);
CREATE INDEX index_report_benchmark_benchmark_report ON report_benchmark(benchmark_id, report_id);
-- `benchmark` cascades to `parameter`, so deleting a benchmark (or a project)
-- deletes parameter sets, and SQLite then verifies that no `report_benchmark`
-- row still references them. Without this index that check is a full table scan
-- of `report_benchmark` per deleted parameter set.
CREATE INDEX index_report_benchmark_parameter ON report_benchmark(parameter_id);
PRAGMA foreign_keys = on;
