PRAGMA foreign_keys = off;
-- `metric_boundary` is a view over `metric`, so it has to be dropped before
-- `metric` can be recreated, and rebuilt from the new shape afterwards.
DROP VIEW IF EXISTS metric_boundary;
-- metric
-- `name` sits with the identity columns, above `value`: the name is part of what
-- the row is, not part of what it measured. The unique key over
-- (report_benchmark_id, measure_id, name) extends the old
-- `UNIQUE(report_benchmark_id, measure_id)` and is also the index the pivot below
-- rides.
-- The table is declared without its two unique keys. They are built below, once
-- every row is in place, as named unique indexes that enforce exactly what the
-- table constraints enforced.
CREATE TABLE up_metric (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL,
    report_benchmark_id INTEGER NOT NULL,
    measure_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    value DOUBLE NOT NULL,
    FOREIGN KEY (report_benchmark_id) REFERENCES report_benchmark (id) ON DELETE CASCADE,
    FOREIGN KEY (measure_id) REFERENCES measure (id)
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
-- The 48 bit millisecond prefix that every minted uuid shares, computed once for
-- the whole migration rather than per row, so that every minted uuid is a v7 of
-- the one instant the migration ran at. That is worth the temporary table at this
-- volume, and it costs the uuid index nothing either way: the index is built below
-- by an external sort, which is indifferent to where its keys fall.
-- `julianday` rather than `unixepoch('now', 'subsec')`, which is newer than the
-- oldest SQLite that can otherwise run this migration.
CREATE TEMP TABLE metric_uuid_prefix AS
SELECT printf(
        '%012x',
        CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
    ) AS prefix;
-- Each stored bound becomes its own row under its conventional name. The id is
-- assigned rather than kept, because it is a new row.
-- The uuid is v7 shaped: the shared millisecond prefix, the version nibble 7, and
-- `randomblob` for the rest. The variant nibble is drawn from `89ab` by
-- `random() & 3` rather than `abs(random()) % 4`, because
-- `abs(-9223372036854775808)` is an integer overflow error in SQLite.
INSERT INTO up_metric(
        uuid,
        report_benchmark_id,
        measure_id,
        name,
        value
    )
SELECT lower(
        substr(metric_uuid_prefix.prefix, 1, 8) || '-' || substr(metric_uuid_prefix.prefix, 9, 4) || '-7' || substr(hex(randomblob(2)), 2) || '-' || substr('89ab', (random() & 3) + 1, 1) || substr(hex(randomblob(2)), 2) || '-' || hex(randomblob(6))
    ),
    metric.report_benchmark_id,
    metric.measure_id,
    'lower_value',
    metric.lower_value
FROM metric,
    metric_uuid_prefix
WHERE metric.lower_value IS NOT NULL;
INSERT INTO up_metric(
        uuid,
        report_benchmark_id,
        measure_id,
        name,
        value
    )
SELECT lower(
        substr(metric_uuid_prefix.prefix, 1, 8) || '-' || substr(metric_uuid_prefix.prefix, 9, 4) || '-7' || substr(hex(randomblob(2)), 2) || '-' || substr('89ab', (random() & 3) + 1, 1) || substr(hex(randomblob(2)), 2) || '-' || hex(randomblob(6))
    ),
    metric.report_benchmark_id,
    metric.measure_id,
    'upper_value',
    metric.upper_value
FROM metric,
    metric_uuid_prefix
WHERE metric.upper_value IS NOT NULL;
-- The two unique keys the table was declared without, built now that every row the
-- explosion produces is in place. Declared on the table they would be maintained
-- online across every one of those inserts, two unique B-trees kept dirty a page
-- split at a time; built here each is one external sort. Measured over the whole
-- rebuild, deferring them reads 16 times fewer pages and writes 29 times fewer. The
-- sort is also indifferent to where its keys fall, so the v4 uuids that the
-- preserved rows carry over cost it nothing.
CREATE UNIQUE INDEX index_metric_uuid ON up_metric(uuid);
CREATE UNIQUE INDEX index_metric_report_benchmark_measure_name ON up_metric(report_benchmark_id, measure_id, name);
DROP TABLE temp.metric_uuid_prefix;
DROP TABLE metric;
ALTER TABLE up_metric
    RENAME TO metric;
-- metric_boundary
-- The view pivots the named rows back into the columns its readers already know,
-- so its column list is unchanged and every response it feeds is unchanged with
-- it. The self joins ride `index_metric_report_benchmark_measure_name`, and
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
