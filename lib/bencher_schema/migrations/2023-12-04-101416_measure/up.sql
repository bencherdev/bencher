PRAGMA foreign_keys = off;
-- metric kind
CREATE TABLE measure (
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
INSERT INTO measure(
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
FROM metric_kind;
DROP TABLE metric_kind;
-- metric
CREATE TABLE up_metric (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    perf_id INTEGER NOT NULL,
    measure_id INTEGER NOT NULL,
    value DOUBLE NOT NULL,
    lower_value DOUBLE,
    upper_value DOUBLE,
    FOREIGN KEY (perf_id) REFERENCES perf (id) ON DELETE CASCADE,
    FOREIGN KEY (measure_id) REFERENCES measure (id),
    UNIQUE(perf_id, measure_id)
);
INSERT INTO up_metric(
        id,
        uuid,
        perf_id,
        measure_id,
        value,
        lower_value,
        upper_value
    )
SELECT id,
    uuid,
    perf_id,
    metric_kind_id,
    value,
    lower_value,
    lower_value
FROM metric;
DROP TABLE metric;
ALTER TABLE up_metric
    RENAME TO metric;
-- threshold
CREATE TABLE up_threshold (
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
INSERT INTO up_threshold(
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
    metric_kind_id,
    statistic_id,
    created,
    modified
FROM threshold;
DROP TABLE threshold;
ALTER TABLE up_threshold
    RENAME TO threshold;
PRAGMA foreign_keys = on;