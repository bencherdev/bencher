PRAGMA foreign_keys = off;
-- threshold
-- Back to the table level unique key over the three dimensions, and without the
-- two columns. A threshold that names a metric or a filter has no shape here, so
-- if any exists the restored unique key is what says so.
DROP INDEX IF EXISTS index_threshold_uuid;

DROP INDEX IF EXISTS index_threshold_dimensions;

DROP INDEX IF EXISTS index_threshold_project_created;

DROP INDEX IF EXISTS index_threshold_branch;

CREATE TABLE down_threshold (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    branch_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    measure_id INTEGER NOT NULL,
    model_id INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (branch_id) REFERENCES branch (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id),
    FOREIGN KEY (measure_id) REFERENCES measure (id),
    FOREIGN KEY (model_id) REFERENCES model (id),
    UNIQUE(branch_id, testbed_id, measure_id)
);

INSERT INTO down_threshold(
        id,
        uuid,
        project_id,
        branch_id,
        testbed_id,
        measure_id,
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
    model_id,
    created,
    modified
FROM threshold;

DROP TABLE threshold;

ALTER TABLE down_threshold
    RENAME TO threshold;

CREATE INDEX index_threshold_project_created ON threshold(project_id, created);

CREATE INDEX index_threshold_branch ON threshold(branch_id);

-- boundary
-- Back to `UNIQUE(metric_id)`. A metric row that carries more than one boundary
-- has no shape here either, and the restored unique key is what says so.
DROP VIEW IF EXISTS metric_boundary;

DROP INDEX IF EXISTS index_boundary_uuid;

DROP INDEX IF EXISTS index_boundary_metric_threshold;

CREATE TABLE down_boundary (
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

INSERT INTO down_boundary(
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

ALTER TABLE down_boundary
    RENAME TO boundary;

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
