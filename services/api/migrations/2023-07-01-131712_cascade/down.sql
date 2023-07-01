PRAGMA foreign_keys = off;
-- organization
CREATE TABLE down_organization (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    subscription TEXT UNIQUE,
    license TEXT UNIQUE,
    constraint zero_or_one_plan check (
        not (
            subscription is not null
            and license is not null
        )
    )
);
INSERT INTO down_organization(
        id,
        uuid,
        name,
        slug,
        subscription,
        license
    )
SELECT id,
    uuid,
    name,
    slug,
    subscription,
    license
FROM organization;
DROP TABLE organization;
ALTER TABLE down_organization
    RENAME TO organization;
-- project
CREATE TABLE down_project (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    organization_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    url TEXT,
    visibility INTEGER NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization (id) ON DELETE CASCADE,
    UNIQUE(organization_id, name)
);
INSERT INTO down_project(
        id,
        uuid,
        organization_id,
        name,
        slug,
        url,
        visibility
    )
SELECT id,
    uuid,
    organization_id,
    name,
    slug,
    url,
    visibility
FROM project;
DROP TABLE project;
ALTER TABLE down_project
    RENAME TO project;
-- metric kind
CREATE TABLE down_metric_kind (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    units TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO down_metric_kind(
        id,
        uuid,
        project_id,
        name,
        slug,
        units
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug,
    units
FROM metric_kind;
DROP TABLE metric_kind;
ALTER TABLE down_metric_kind
    RENAME TO metric_kind;
-- branch
CREATE TABLE down_branch (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO down_branch(
        id,
        uuid,
        project_id,
        name,
        slug
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug
FROM branch;
DROP TABLE branch;
ALTER TABLE down_branch
    RENAME TO branch;
-- branch version
CREATE TABLE down_branch_version (
    id INTEGER PRIMARY KEY NOT NULL,
    branch_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    FOREIGN KEY (branch_id) REFERENCES branch (id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES version (id) ON DELETE CASCADE,
    UNIQUE(branch_id, version_id)
);
INSERT INTO down_branch_version(
        id,
        branch_id,
        version_id
    )
SELECT id,
    branch_id,
    version_id
FROM branch_version;
DROP TABLE branch_version;
ALTER TABLE down_branch_version
    RENAME TO branch_version;
-- testbed
CREATE TABLE down_testbed (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO down_testbed(
        id,
        uuid,
        project_id,
        name,
        slug
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug
FROM testbed;
DROP TABLE testbed;
ALTER TABLE down_testbed
    RENAME TO testbed;
-- benchmark
CREATE TABLE down_benchmark (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id),
    UNIQUE(project_id, name)
);
INSERT INTO down_benchmark(
        id,
        uuid,
        project_id,
        name
    )
SELECT id,
    uuid,
    project_id,
    name
FROM benchmark;
DROP TABLE benchmark;
ALTER TABLE down_benchmark
    RENAME TO benchmark;
-- threshold
CREATE TABLE down_threshold (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    branch_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    metric_kind_id INTEGER NOT NULL,
    statistic_id INTEGER NOT NULL,
    FOREIGN KEY (branch_id) REFERENCES branch (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id),
    FOREIGN KEY (metric_kind_id) REFERENCES metric_kind (id),
    FOREIGN KEY (statistic_id) REFERENCES statistic (id),
    UNIQUE(branch_id, testbed_id, metric_kind_id)
);
INSERT INTO down_threshold(
        id,
        uuid,
        branch_id,
        testbed_id,
        metric_kind_id,
        statistic_id
    )
SELECT id,
    uuid,
    branch_id,
    testbed_id,
    metric_kind_id,
    statistic_id
FROM threshold;
DROP TABLE threshold;
ALTER TABLE down_threshold
    RENAME TO threshold;
-- statistic
CREATE TABLE down_statistic (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    test INTEGER NOT NULL,
    min_sample_size BIGINT,
    max_sample_size BIGINT,
    window BIGINT,
    lower_boundary DOUBLE,
    upper_boundary DOUBLE
);
INSERT INTO down_statistic(
        id,
        uuid,
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary
    )
SELECT id,
    uuid,
    test,
    min_sample_size,
    max_sample_size,
    window,
    lower_boundary,
    upper_boundary
FROM statistic;
DROP TABLE statistic;
ALTER TABLE down_statistic
    RENAME TO statistic;
-- report
CREATE TABLE down_report (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    branch_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    adapter INTEGER NOT NULL,
    start_time BIGINT NOT NULL,
    end_time BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id),
    FOREIGN KEY (version_id) REFERENCES version (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id)
);
INSERT INTO down_report(
        id,
        uuid,
        user_id,
        branch_id,
        version_id,
        testbed_id,
        adapter,
        start_time,
        end_time
    )
SELECT id,
    uuid,
    user_id,
    branch_id,
    version_id,
    testbed_id,
    adapter,
    (start_time * 1000000000),
    (end_time * 1000000000)
FROM report;
DROP TABLE report;
ALTER TABLE down_report
    RENAME TO report;
-- perf
CREATE TABLE down_perf (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    report_id INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id) ON DELETE CASCADE,
    UNIQUE(report_id, iteration, benchmark_id)
);
INSERT INTO down_perf(
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
ALTER TABLE down_perf
    RENAME TO perf;
-- metric
CREATE TABLE down_metric (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    perf_id INTEGER NOT NULL,
    metric_kind_id INTEGER NOT NULL,
    value DOUBLE NOT NULL,
    lower_bound DOUBLE,
    upper_bound DOUBLE,
    FOREIGN KEY (perf_id) REFERENCES perf (id),
    FOREIGN KEY (metric_kind_id) REFERENCES metric_kind (id),
    UNIQUE(perf_id, metric_kind_id)
);
INSERT INTO down_metric(
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
ALTER TABLE down_metric
    RENAME TO metric;
-- boundary
CREATE TABLE down_boundary (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    threshold_id INTEGER NOT NULL,
    statistic_id INTEGER NOT NULL,
    metric_id INTEGER NOT NULL UNIQUE,
    lower_limit DOUBLE,
    upper_limit DOUBLE,
    FOREIGN KEY (metric_id) REFERENCES metric (id),
    FOREIGN KEY (threshold_id) REFERENCES threshold (id),
    FOREIGN KEY (statistic_id) REFERENCES statistic (id)
);
INSERT INTO down_boundary(
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
ALTER TABLE down_boundary
    RENAME TO boundary;
-- alert
CREATE TABLE down_alert (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    boundary_id INTEGER NOT NULL,
    boundary_limit BOOLEAN NOT NULL,
    status INTEGER NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (boundary_id) REFERENCES boundary (id)
);
INSERT INTO down_alert(
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
ALTER TABLE down_alert
    RENAME TO alert;
PRAGMA foreign_keys = on;