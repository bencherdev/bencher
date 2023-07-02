PRAGMA foreign_keys = off;
-- organization
CREATE TABLE up_organization (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    subscription TEXT UNIQUE,
    license TEXT UNIQUE,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    constraint zero_or_one_plan check (
        not (
            subscription is not null
            and license is not null
        )
    )
);
INSERT INTO up_organization(
        id,
        uuid,
        name,
        slug,
        subscription,
        license,
        created,
        modified
    )
SELECT id,
    uuid,
    name,
    slug,
    subscription,
    license,
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    ),
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    )
FROM organization;
DROP TABLE organization;
ALTER TABLE up_organization
    RENAME TO organization;
-- organization role
CREATE TABLE up_organization_role (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL,
    organization_id INTEGER NOT NULL,
    role TEXT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (organization_id) REFERENCES organization (id) ON UPDATE CASCADE ON DELETE CASCADE,
    UNIQUE(user_id, organization_id)
);
INSERT INTO up_organization_role(
        id,
        user_id,
        organization_id,
        role,
        created,
        modified
    )
SELECT id,
    user_id,
    organization_id,
    role,
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    ),
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    )
FROM organization_role;
DROP TABLE organization_role;
ALTER TABLE up_organization_role
    RENAME TO organization_role;
-- project
CREATE TABLE up_project (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    organization_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    url TEXT,
    visibility INTEGER NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization (id) ON UPDATE CASCADE ON DELETE CASCADE,
    UNIQUE(organization_id, name)
);
INSERT INTO up_project(
        id,
        uuid,
        organization_id,
        name,
        slug,
        url,
        visibility,
        created,
        modified
    )
SELECT id,
    uuid,
    organization_id,
    name,
    slug,
    url,
    visibility,
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    ),
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    )
FROM project;
DROP TABLE project;
ALTER TABLE up_project
    RENAME TO project;
-- project role
CREATE TABLE up_project_role (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL,
    project_id INTEGER NOT NULL,
    role TEXT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (project_id) REFERENCES project (id) ON UPDATE CASCADE ON DELETE CASCADE,
    UNIQUE(user_id, project_id)
);
INSERT INTO up_project_role(
        id,
        user_id,
        project_id,
        role,
        created,
        modified
    )
SELECT id,
    user_id,
    project_id,
    role,
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    ),
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    )
FROM project_role;
DROP TABLE project_role;
ALTER TABLE up_project_role
    RENAME TO project_role;
-- metric kind
CREATE TABLE up_metric_kind (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    units TEXT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON UPDATE CASCADE ON DELETE CASCADE,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO up_metric_kind(
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
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    ),
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    )
FROM metric_kind;
DROP TABLE metric_kind;
ALTER TABLE up_metric_kind
    RENAME TO metric_kind;
-- branch
CREATE TABLE up_branch (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON UPDATE CASCADE ON DELETE CASCADE,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO up_branch(
        id,
        uuid,
        project_id,
        name,
        slug,
        created,
        modified
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug,
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    ),
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    )
FROM branch;
DROP TABLE branch;
ALTER TABLE up_branch
    RENAME TO branch;
-- version
CREATE TABLE up_version (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    number INTEGER NOT NULL,
    hash TEXT
);
INSERT INTO up_version(
        id,
        uuid,
        project_id,
        number,
        hash
    )
SELECT id,
    uuid,
    (
        SELECT project_id
        FROM branch
        WHERE branch.id = (
                SELECT branch_id
                FROM branch_version
                WHERE version.id = branch_version.version_id
                LIMIT 1
            )
    ), number, hash
FROM version;
DROP TABLE version;
ALTER TABLE up_version
    RENAME TO version;
-- branch version
CREATE TABLE up_branch_version (
    id INTEGER PRIMARY KEY NOT NULL,
    branch_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    FOREIGN KEY (branch_id) REFERENCES branch (id) ON UPDATE CASCADE ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES version (id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    UNIQUE(branch_id, version_id)
);
INSERT INTO up_branch_version(
        id,
        branch_id,
        version_id
    )
SELECT id,
    branch_id,
    version_id
FROM branch_version;
DROP TABLE branch_version;
ALTER TABLE up_branch_version
    RENAME TO branch_version;
-- testbed
CREATE TABLE up_testbed (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON UPDATE CASCADE ON DELETE CASCADE,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO up_testbed(
        id,
        uuid,
        project_id,
        name,
        slug,
        created,
        modified
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug,
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    ),
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    )
FROM testbed;
DROP TABLE testbed;
ALTER TABLE up_testbed
    RENAME TO testbed;
-- benchmark
CREATE TABLE up_benchmark (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    created BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON UPDATE CASCADE ON DELETE CASCADE,
    UNIQUE(project_id, name)
);
INSERT INTO up_benchmark(
        id,
        uuid,
        project_id,
        name,
        created
    )
SELECT id,
    uuid,
    project_id,
    name,
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    )
FROM benchmark;
DROP TABLE benchmark;
ALTER TABLE up_benchmark
    RENAME TO benchmark;
-- threshold
CREATE TABLE up_threshold (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    metric_kind_id INTEGER NOT NULL,
    branch_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    statistic_id INTEGER NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (metric_kind_id) REFERENCES metric_kind (id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (branch_id) REFERENCES branch (id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (testbed_id) REFERENCES testbed (id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (statistic_id) REFERENCES statistic (id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    UNIQUE(metric_kind_id, branch_id, testbed_id)
);
INSERT INTO up_threshold(
        id,
        uuid,
        metric_kind_id,
        branch_id,
        testbed_id,
        statistic_id,
        created,
        modified
    )
SELECT id,
    uuid,
    metric_kind_id,
    branch_id,
    testbed_id,
    statistic_id,
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    ),
    (
        SELECT strftime('%s', datetime('now', 'utc'))
    )
FROM threshold;
DROP TABLE threshold;
ALTER TABLE up_threshold
    RENAME TO threshold;
-- statistic
CREATE TABLE up_statistic (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    test INTEGER NOT NULL,
    min_sample_size BIGINT,
    max_sample_size BIGINT,
    window BIGINT,
    lower_boundary DOUBLE,
    upper_boundary DOUBLE,
    created BIGINT NOT NULL
);
INSERT INTO up_statistic(
        id,
        uuid,
        project_id,
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
        SELECT project_id
        FROM branch
        WHERE branch.id = (
                SELECT branch_id
                FROM threshold
                WHERE statistic.id = threshold.statistic_id
                LIMIT 1
            )
    ), test, min_sample_size, max_sample_size, window, lower_boundary, upper_boundary, (
        SELECT strftime('%s', datetime('now', 'utc'))
    )
FROM statistic;
DROP TABLE statistic;
ALTER TABLE up_statistic
    RENAME TO statistic;
-- report
CREATE TABLE up_report (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    branch_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    adapter INTEGER NOT NULL,
    start_time BIGINT NOT NULL,
    end_time BIGINT NOT NULL,
    created BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (branch_id) REFERENCES branch (id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (version_id) REFERENCES version (id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (testbed_id) REFERENCES testbed (id) ON UPDATE RESTRICT ON DELETE RESTRICT
);
INSERT INTO up_report(
        id,
        uuid,
        user_id,
        branch_id,
        version_id,
        testbed_id,
        adapter,
        start_time,
        end_time,
        created
    )
SELECT id,
    uuid,
    user_id,
    branch_id,
    version_id,
    testbed_id,
    adapter,
    (start_time / 1000000000),
    (end_time / 1000000000),
    (end_time / 1000000000)
FROM report;
DROP TABLE report;
ALTER TABLE up_report
    RENAME TO report;
-- perf
CREATE TABLE up_perf (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_id INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    FOREIGN KEY (report_id) REFERENCES report (id) ON UPDATE CASCADE ON DELETE CASCADE,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    UNIQUE(report_id, iteration, benchmark_id)
);
INSERT INTO up_perf(
        id,
        uuid,
        report_id,
        iteration,
        benchmark_id
    )
SELECT id,
    uuid,
    report_id,
    iteration,
    benchmark_id
FROM perf;
DROP TABLE perf;
ALTER TABLE up_perf
    RENAME TO perf;
-- metric
CREATE TABLE up_metric (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    perf_id INTEGER NOT NULL,
    metric_kind_id INTEGER NOT NULL,
    value DOUBLE NOT NULL,
    lower_bound DOUBLE,
    upper_bound DOUBLE,
    FOREIGN KEY (perf_id) REFERENCES perf (id) ON UPDATE CASCADE ON DELETE CASCADE,
    FOREIGN KEY (metric_kind_id) REFERENCES metric_kind (id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    UNIQUE(perf_id, metric_kind_id)
);
INSERT INTO up_metric(
        id,
        uuid,
        perf_id,
        metric_kind_id,
        value,
        lower_bound,
        upper_bound
    )
SELECT id,
    uuid,
    perf_id,
    metric_kind_id,
    value,
    lower_bound,
    upper_bound
FROM metric;
DROP TABLE metric;
ALTER TABLE up_metric
    RENAME TO metric;
-- boundary
CREATE TABLE up_boundary (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    threshold_id INTEGER NOT NULL,
    statistic_id INTEGER NOT NULL,
    metric_id INTEGER NOT NULL UNIQUE,
    lower_limit DOUBLE,
    upper_limit DOUBLE,
    FOREIGN KEY (metric_id) REFERENCES metric (id) ON UPDATE CASCADE ON DELETE CASCADE,
    FOREIGN KEY (threshold_id) REFERENCES threshold (id) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (statistic_id) REFERENCES statistic (id) ON UPDATE RESTRICT ON DELETE RESTRICT
);
INSERT INTO up_boundary(
        id,
        uuid,
        threshold_id,
        statistic_id,
        metric_id,
        lower_limit,
        upper_limit
    )
SELECT id,
    uuid,
    threshold_id,
    statistic_id,
    metric_id,
    lower_limit,
    upper_limit
FROM boundary;
DROP TABLE boundary;
ALTER TABLE up_boundary
    RENAME TO boundary;
-- alert
CREATE TABLE up_alert (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    boundary_id INTEGER NOT NULL,
    boundary_limit BOOLEAN NOT NULL,
    status INTEGER NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (boundary_id) REFERENCES boundary (id) ON UPDATE CASCADE ON DELETE CASCADE
);
INSERT INTO up_alert(
        id,
        uuid,
        boundary_id,
        boundary_limit,
        status,
        modified
    )
SELECT id,
    uuid,
    boundary_id,
    boundary_limit,
    status,
    modified
FROM alert;
DROP TABLE alert;
ALTER TABLE up_alert
    RENAME TO alert;
PRAGMA foreign_keys = on;