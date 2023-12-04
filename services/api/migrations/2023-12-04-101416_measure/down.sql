PRAGMA foreign_keys = off;
-- metric kind
CREATE TABLE metric_kind (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    units TEXT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO metric_kind(
        id,
        uuid,
        project_id,
        name,
        slug,
        units,
        created,
        modified
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug,
    units,
    created,
    modified
FROM measure;
DROP TABLE measure;
-- metric
CREATE TABLE down_metric (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    perf_id INTEGER NOT NULL,
    metric_kind_id INTEGER NOT NULL,
    value DOUBLE NOT NULL,
    lower_value DOUBLE,
    upper_value DOUBLE,
    FOREIGN KEY (perf_id) REFERENCES perf (id) ON DELETE CASCADE,
    FOREIGN KEY (metric_kind_id) REFERENCES metric_kind (id),
    UNIQUE(perf_id, metric_kind_id)
);
INSERT INTO down_metric(
        id,
        uuid,
        perf_id,
        metric_kind_id,
        value,
        lower_value,
        upper_value
    )
SELECT id,
    uuid,
    perf_id,
    measure_id,
    value,
    lower_value,
    lower_value
FROM metric;
DROP TABLE metric;
ALTER TABLE down_metric
    RENAME TO metric;
-- threshold
CREATE TABLE down_threshold (
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
INSERT INTO down_threshold(
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
    measure_id,
    branch_id,
    testbed_id,
    statistic_id,
    created,
    modified
FROM threshold;
DROP TABLE threshold;
ALTER TABLE down_threshold
    RENAME TO threshold;
PRAGMA foreign_keys = on;