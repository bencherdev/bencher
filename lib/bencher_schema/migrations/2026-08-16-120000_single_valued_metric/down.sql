PRAGMA foreign_keys = off;
-- The view has to be dropped before the table can be recreated.
DROP VIEW IF EXISTS metric_boundary;
-- metric
CREATE TABLE down_metric (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_benchmark_id INTEGER NOT NULL,
    measure_id INTEGER NOT NULL,
    value DOUBLE NOT NULL,
    lower_value DOUBLE,
    upper_value DOUBLE,
    FOREIGN KEY (report_benchmark_id) REFERENCES report_benchmark (id) ON DELETE CASCADE,
    FOREIGN KEY (measure_id) REFERENCES measure (id),
    UNIQUE(report_benchmark_id, measure_id)
);
-- The inverse of the explosion: the `value` row keeps its id and uuid, and the
-- bound rows collapse back into its columns. Metrics that are not part of
-- the metric triple have no column to land in and are dropped, which is the
-- price of going back to a shape that cannot hold them.
INSERT INTO down_metric(
        id,
        uuid,
        report_benchmark_id,
        measure_id,
        value,
        lower_value,
        upper_value
    )
SELECT metric.id,
    metric.uuid,
    metric.report_benchmark_id,
    metric.measure_id,
    metric.value,
    lower_metric.value,
    upper_metric.value
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
WHERE metric.name = 'value';
DROP TABLE metric;
ALTER TABLE down_metric
    RENAME TO metric;
-- metric_boundary
CREATE VIEW metric_boundary AS
SELECT metric.id AS metric_id,
    metric.uuid AS metric_uuid,
    metric.report_benchmark_id,
    metric.measure_id,
    metric.value,
    metric.lower_value,
    metric.upper_value,
    boundary.id AS boundary_id,
    boundary.uuid AS boundary_uuid,
    boundary.threshold_id,
    boundary.model_id,
    boundary.baseline,
    boundary.lower_limit,
    boundary.upper_limit
FROM metric
    LEFT OUTER JOIN boundary ON (boundary.metric_id = metric.id);
PRAGMA foreign_keys = on;
