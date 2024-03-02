PRAGMA foreign_keys = off;
-- statistic
CREATE TABLE statistic (
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
    FOREIGN KEY (threshold_id) REFERENCES threshold (id) ON DELETE CASCADE
);
INSERT INTO statistic(
        id,
        uuid,
        threshold_id,
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
        created
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
    created
FROM model;
DROP TABLE model;
-- threshold
CREATE TABLE down_threshold (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    branch_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    measure_id INTEGER NOT NULL,
    statistic_id INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (branch_id) REFERENCES branch (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id),
    FOREIGN KEY (measure_id) REFERENCES measure (id),
    FOREIGN KEY (statistic_id) REFERENCES statistic (id),
    UNIQUE(branch_id, testbed_id, measure_id)
);
INSERT INTO down_threshold(
        id,
        uuid,
        project_id,
        branch_id,
        testbed_id,
        measure_id,
        statistic_id,
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
-- boundary
CREATE TABLE down_boundary (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    threshold_id INTEGER NOT NULL,
    statistic_id INTEGER NOT NULL,
    metric_id INTEGER NOT NULL UNIQUE,
    baseline DOUBLE,
    lower_limit DOUBLE,
    upper_limit DOUBLE,
    FOREIGN KEY (threshold_id) REFERENCES threshold (id),
    FOREIGN KEY (statistic_id) REFERENCES statistic (id),
    FOREIGN KEY (metric_id) REFERENCES metric (id) ON DELETE CASCADE
);
INSERT INTO down_boundary(
        id,
        uuid,
        threshold_id,
        statistic_id,
        metric_id,
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
DROP TABLE boundary;
ALTER TABLE down_boundary
    RENAME TO boundary;
PRAGMA foreign_keys = on;