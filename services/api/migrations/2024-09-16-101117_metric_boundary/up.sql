PRAGMA foreign_keys = off;
-- boundary
CREATE TABLE up_boundary (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    metric_id INTEGER NOT NULL UNIQUE,
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
    threshold_id,
    model_id,
    metric_id,
    baseline,
    lower_limit,
    upper_limit
FROM boundary;
-- The view has to be dropped before the table can be dropped
DROP VIEW IF EXISTS metric_boundary;
DROP TABLE boundary;
ALTER TABLE up_boundary
    RENAME TO boundary;
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
-- index
DROP INDEX index_alert_boundary;
CREATE INDEX index_alert_boundary ON alert(boundary_id);
PRAGMA foreign_keys = on;