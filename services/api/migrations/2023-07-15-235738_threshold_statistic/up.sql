PRAGMA foreign_keys = off;
-- threshold
CREATE TABLE up_threshold (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    metric_kind_id INTEGER NOT NULL,
    branch_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    statistic_id INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (metric_kind_id) REFERENCES metric_kind (id),
    FOREIGN KEY (branch_id) REFERENCES branch (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id),
    FOREIGN KEY (statistic_id) REFERENCES statistic (id),
    UNIQUE(metric_kind_id, branch_id, testbed_id)
);
INSERT INTO up_threshold(
        id,
        uuid,
        project_id,
        metric_kind_id,
        branch_id,
        testbed_id,
        statistic_id,
        created,
        modified
    )
SELECT id,
    uuid,
    project_id,
    metric_kind_id,
    branch_id,
    testbed_id,
    statistic_id,
    created,
    modified
FROM threshold;
DROP TABLE threshold;
ALTER TABLE up_threshold
    RENAME TO threshold;
-- statistic
CREATE TABLE up_statistic (
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
INSERT INTO up_statistic(
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
    (
        SELECT id
        FROM threshold
        WHERE threshold.statistic_id = statistic.id
    ),
    test,
    min_sample_size,
    max_sample_size,
    window,
    lower_boundary,
    upper_boundary,
    created
FROM statistic
WHERE EXISTS(
        SELECT id
        FROM threshold
        WHERE threshold.statistic_id = statistic.id
    );
DROP TABLE statistic;
ALTER TABLE up_statistic
    RENAME TO statistic;
PRAGMA foreign_keys = on;