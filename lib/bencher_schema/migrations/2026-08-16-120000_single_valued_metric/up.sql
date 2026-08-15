PRAGMA foreign_keys = off;
-- `metric_boundary` is a view over `metric`, so it has to be dropped before
-- `metric` can be recreated, and rebuilt from the new shape afterwards.
DROP VIEW IF EXISTS metric_boundary;
-- metric
-- `name` sits with the identity columns, above `value`: the name is part of what
-- the row is, not part of what it measured. `UNIQUE(report_benchmark_id,
-- measure_id, name)` extends the old `UNIQUE(report_benchmark_id, measure_id)`
-- and is also the index the pivot below rides.
CREATE TABLE up_metric (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_benchmark_id INTEGER NOT NULL,
    measure_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    value DOUBLE NOT NULL,
    FOREIGN KEY (report_benchmark_id) REFERENCES report_benchmark (id) ON DELETE CASCADE,
    FOREIGN KEY (measure_id) REFERENCES measure (id),
    UNIQUE(report_benchmark_id, measure_id, name)
);
-- Every old row yields a `value` row that keeps the original id and uuid, which
-- makes the boundary remap a no-op: `boundary.metric_id` already points at that
-- id, so no `boundary` row is rewritten and its unique constraint is undisturbed.
INSERT INTO up_metric(
        id,
        uuid,
        report_benchmark_id,
        measure_id,
        name,
        value
    )
SELECT id,
    uuid,
    report_benchmark_id,
    measure_id,
    'value',
    value
FROM metric;
-- Each stored bound becomes its own row under its conventional name. The id is
-- assigned rather than kept, because it is a new row.
-- Pure SQL has no UUIDv7 function, so the UUID is a v4 minted from `randomblob`:
-- 16 random bytes with the version nibble set to 4 and the variant nibble drawn
-- from `89ab`. `random() & 3` is used rather than `abs(random()) % 4` because
-- `abs(-9223372036854775808)` is an integer overflow error in SQLite.
INSERT INTO up_metric(
        uuid,
        report_benchmark_id,
        measure_id,
        name,
        value
    )
SELECT lower(
        hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)), 2) || '-' || substr('89ab', (random() & 3) + 1, 1) || substr(hex(randomblob(2)), 2) || '-' || hex(randomblob(6))
    ),
    report_benchmark_id,
    measure_id,
    'lower_value',
    lower_value
FROM metric
WHERE lower_value IS NOT NULL;
INSERT INTO up_metric(
        uuid,
        report_benchmark_id,
        measure_id,
        name,
        value
    )
SELECT lower(
        hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)), 2) || '-' || substr('89ab', (random() & 3) + 1, 1) || substr(hex(randomblob(2)), 2) || '-' || hex(randomblob(6))
    ),
    report_benchmark_id,
    measure_id,
    'upper_value',
    upper_value
FROM metric
WHERE upper_value IS NOT NULL;
DROP TABLE metric;
ALTER TABLE up_metric
    RENAME TO metric;
-- metric_boundary
-- The view pivots the named rows back into the columns its readers already know,
-- so its column list is unchanged and every response it feeds is unchanged with
-- it. The self joins ride `UNIQUE(report_benchmark_id, measure_id, name)`, and
-- because that index makes them provably at most one row, SQLite omits them
-- outright for any query that does not select a bound.
-- `WHERE metric.name = 'value'` keeps the view one row per measurement, which is
-- what makes it a drop-in for the table it replaced.
CREATE VIEW metric_boundary AS
SELECT metric.id AS metric_id,
    metric.uuid AS metric_uuid,
    metric.report_benchmark_id,
    metric.measure_id,
    metric.value,
    lower_metric.value AS lower_value,
    upper_metric.value AS upper_value,
    boundary.id AS boundary_id,
    boundary.uuid AS boundary_uuid,
    boundary.threshold_id,
    boundary.model_id,
    boundary.baseline,
    boundary.lower_limit,
    boundary.upper_limit
FROM metric
    LEFT OUTER JOIN metric AS lower_metric ON (
        lower_metric.report_benchmark_id = metric.report_benchmark_id
        AND lower_metric.measure_id = metric.measure_id
        AND lower_metric.name = 'lower_value'
    )
    LEFT OUTER JOIN metric AS upper_metric ON (
        upper_metric.report_benchmark_id = metric.report_benchmark_id
        AND upper_metric.measure_id = metric.measure_id
        AND upper_metric.name = 'upper_value'
    )
    LEFT OUTER JOIN boundary ON (boundary.metric_id = metric.id)
WHERE metric.name = 'value';
PRAGMA foreign_keys = on;
