PRAGMA foreign_keys = off;
-- model
CREATE TABLE model (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    threshold_id INTEGER NOT NULL,
    test INTEGER NOT NULL,
    min_sample_size BIGINT,
    max_sample_size BIGINT,
    window BIGINT,
    lower_boundary DOUBLE,
    upper_boundary DOUBLE,
    created BIGINT NOT NULL,
    replaced BIGINT,
    FOREIGN KEY (threshold_id) REFERENCES threshold (id) ON DELETE CASCADE
);
INSERT INTO model(
        id,
        uuid,
        threshold_id,
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
        created,
        replaced
    )
SELECT id,
    uuid,
    threshold_id,
    test,
    min_sample_size,
    max_sample_size,
    window,
    lower_boundary,
    upper_boundary,
    created,
    (
        SELECT strftime('%s', datetime('now', 'utc'))
        FROM threshold
        WHERE statistic.threshold_id = threshold.id
            AND threshold.statistic_id != statistic.id
    )
FROM statistic;
DROP TABLE statistic;
-- threshold
CREATE TABLE up_threshold (
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
INSERT INTO up_threshold(
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
    statistic_id,
    created,
    modified
FROM threshold;
DROP TABLE threshold;
ALTER TABLE up_threshold
    RENAME TO threshold;
-- boundary
CREATE TABLE up_boundary (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    threshold_id INTEGER NOT NULL,
    model_id INTEGER NOT NULL,
    metric_id INTEGER NOT NULL UNIQUE,
    baseline DOUBLE,
    lower_limit DOUBLE,
    upper_limit DOUBLE,
    FOREIGN KEY (threshold_id) REFERENCES threshold (id),
    FOREIGN KEY (model_id) REFERENCES model (id),
    FOREIGN KEY (metric_id) REFERENCES metric (id) ON DELETE CASCADE
);
INSERT INTO up_boundary(
        id,
        uuid,
        threshold_id,
        model_id,
        metric_id,
        baseline,
        lower_limit,
        upper_limit
    )
SELECT id,
    uuid,
    threshold_id,
    statistic_id,
    metric_id,
    baseline,
    lower_limit,
    upper_limit
FROM boundary;
DROP TABLE boundary;
ALTER TABLE up_boundary
    RENAME TO boundary;
PRAGMA foreign_keys = on;