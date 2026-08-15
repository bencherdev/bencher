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
-- STUB: only the `value` row is carried over, so every stored bound is dropped.
-- The explosion into the `lower_value` and `upper_value` rows lands with the
-- implementation.
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
DROP TABLE metric;
ALTER TABLE up_metric
    RENAME TO metric;
-- metric_boundary
-- STUB: the bound columns are held in the column list but always NULL, so the
-- view's three readers still compile. The pivot over the named rows lands with
-- the implementation.
CREATE VIEW metric_boundary AS
SELECT metric.id AS metric_id,
    metric.uuid AS metric_uuid,
    metric.report_benchmark_id,
    metric.measure_id,
    metric.value,
    NULL AS lower_value,
    NULL AS upper_value,
    boundary.id AS boundary_id,
    boundary.uuid AS boundary_uuid,
    boundary.threshold_id,
    boundary.model_id,
    boundary.baseline,
    boundary.lower_limit,
    boundary.upper_limit
FROM metric
    LEFT OUTER JOIN boundary ON (boundary.metric_id = metric.id)
WHERE metric.name = 'value';
PRAGMA foreign_keys = on;
