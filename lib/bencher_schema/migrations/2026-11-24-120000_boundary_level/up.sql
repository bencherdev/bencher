PRAGMA foreign_keys = off;
-- boundary
-- A boundary records the level each side was tested at, beside the limit it gives.
-- At BMF version 1, a t-test, z-score, or log-normal boundary covers every check its
-- model makes in one report on one side, so each check runs at a stricter level than
-- the boundary itself. NULL is a side that was not corrected, which is every existing row.
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
    lower_level DOUBLE,
    upper_limit DOUBLE,
    upper_level DOUBLE,
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
CREATE UNIQUE INDEX index_boundary_uuid ON boundary(uuid);
CREATE UNIQUE INDEX index_boundary_metric_threshold ON boundary(metric_id, threshold_id);
CREATE INDEX index_boundary_threshold ON boundary(threshold_id);
CREATE INDEX index_boundary_model ON boundary(model_id);
-- metric_boundary
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
