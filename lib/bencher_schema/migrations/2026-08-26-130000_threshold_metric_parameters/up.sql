PRAGMA foreign_keys = off;
-- threshold
-- A threshold gains the name it gates and the grid points it gates.
--
-- `metric` is the `metric.name` this threshold gates. NULL is the conventional
-- `value` name, which is what every threshold that predates this migration gates,
-- so every existing row carries NULL and nothing about it moves.
--
-- `parameters` is the filter over grid points: the SQLite JSONB encoding of a JSON
-- array of parameter sets, OR across the array and subset match within each set.
-- NULL is match all, which again is what every existing row does. It is declared
-- `BLOB` to match the SQLite representation of the `Jsonb` SQL type, the same as
-- `parameter."set"`.
--
-- The table is recreated rather than altered because the identity it enforces
-- changes. `UNIQUE(branch_id, testbed_id, measure_id)` is backed by an automatic
-- index that no statement can drop, so the only way off it is a new table.
--
-- The table is declared without its unique keys. They are built below, once the
-- copy has landed, as named indexes.
CREATE TABLE up_threshold (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL,
    project_id INTEGER NOT NULL,
    branch_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    measure_id INTEGER NOT NULL,
    metric TEXT,
    parameters BLOB,
    model_id INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (branch_id) REFERENCES branch (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id),
    FOREIGN KEY (measure_id) REFERENCES measure (id),
    FOREIGN KEY (model_id) REFERENCES model (id)
);
INSERT INTO up_threshold(
        id,
        uuid,
        project_id,
        branch_id,
        testbed_id,
        measure_id,
        metric,
        parameters,
        model_id,
        created,
        modified
    )
SELECT id,
    uuid,
    project_id,
    branch_id,
    testbed_id,
    measure_id,
    NULL,
    NULL,
    model_id,
    created,
    modified
FROM threshold;
DROP TABLE threshold;
ALTER TABLE up_threshold
    RENAME TO threshold;
CREATE UNIQUE INDEX index_threshold_uuid ON threshold(uuid);
-- The identity of a threshold, under the null semantics the two new columns carry.
--
-- A SQLite unique index treats NULLs as distinct, so a plain unique key over the
-- five columns would let two bare thresholds sit on one (branch, testbed, measure)
-- and would let an explicit `value` sit beside an absent one. The index is declared
-- over the effective values instead: NULL metric reads as `value` and NULL
-- parameters reads as the empty blob, which is a value no stored filter can take
-- because a filter that matches everything is stored as NULL.
CREATE UNIQUE INDEX index_threshold_dimensions ON threshold(
    branch_id,
    testbed_id,
    measure_id,
    COALESCE(metric, 'value'),
    COALESCE(parameters, x'')
);
CREATE INDEX index_threshold_project_created ON threshold(project_id, created);
-- Ingest loads every threshold of one (branch, testbed) once per report, so the
-- branch is the leading column of the load.
CREATE INDEX index_threshold_branch ON threshold(branch_id);
-- boundary
-- Several thresholds may gate one metric row now, and each computes its own
-- boundary against its own sample, so `UNIQUE(metric_id)` becomes
-- `UNIQUE(metric_id, threshold_id)`.
--
-- `metric_boundary` is a view over `boundary`, so it is dropped before the swap and
-- recreated unchanged after it.
DROP VIEW IF EXISTS metric_boundary;
CREATE TABLE up_boundary (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL,
    metric_id INTEGER NOT NULL,
    threshold_id INTEGER NOT NULL,
    model_id INTEGER NOT NULL,
    baseline DOUBLE,
    lower_limit DOUBLE,
    upper_limit DOUBLE,
    FOREIGN KEY (metric_id) REFERENCES metric (id) ON DELETE CASCADE,
    FOREIGN KEY (threshold_id) REFERENCES threshold (id),
    FOREIGN KEY (model_id) REFERENCES model (id)
);
INSERT INTO up_boundary(
        id,
        uuid,
        metric_id,
        threshold_id,
        model_id,
        baseline,
        lower_limit,
        upper_limit
    )
SELECT id,
    uuid,
    metric_id,
    threshold_id,
    model_id,
    baseline,
    lower_limit,
    upper_limit
FROM boundary;
DROP TABLE boundary;
ALTER TABLE up_boundary
    RENAME TO boundary;
-- The two unique keys the table was declared without, built now that every row is
-- in place. This is the largest table this migration touches. Declared on the table
-- they would be maintained online across every insert of the copy, and the uuids
-- are v4, so each insert would land on a random page of the uuid index and those
-- random reads and writes, not the scan, would set the pace. Built here each is one
-- external sort, which is indifferent to where its keys fall, so the rebuild floors
-- at the scan that copies the rows.
CREATE UNIQUE INDEX index_boundary_uuid ON boundary(uuid);
CREATE UNIQUE INDEX index_boundary_metric_threshold ON boundary(metric_id, threshold_id);
-- metric_boundary
-- Recreated exactly as it stood. The view carries at most one boundary per metric
-- row, which is no longer the whole truth, and no reader reaches a boundary through
-- it any more. It stays for the migration that pins its column list.
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
