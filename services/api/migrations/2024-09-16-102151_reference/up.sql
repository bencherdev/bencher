PRAGMA foreign_keys = off;
-- reference
CREATE TABLE reference (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    branch_id INTEGER NOT NULL,
    start_point_id INTEGER,
    created BIGINT NOT NULL,
    replaced BIGINT,
    FOREIGN KEY (branch_id) REFERENCES branch (id) ON DELETE CASCADE,
    FOREIGN KEY (start_point_id) REFERENCES branch_version (id) ON DELETE
    SET NULL
);
INSERT INTO reference(
        id,
        uuid,
        branch_id,
        start_point_id,
        created,
        replaced
    )
SELECT id,
    uuid,
    id,
    start_point_id,
    created,
    null
FROM branch;
-- reference_version
CREATE TABLE reference_version (
    id INTEGER PRIMARY KEY NOT NULL,
    reference_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    FOREIGN KEY (reference_id) REFERENCES reference (id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES version (id) ON DELETE CASCADE,
    UNIQUE(reference_id, version_id)
);
INSERT INTO reference_version(
        id,
        reference_id,
        version_id
    )
SELECT id,
    branch_id,
    version_id
FROM branch_version;
DROP TABLE branch_version;
-- reference
CREATE TABLE up_reference (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    branch_id INTEGER NOT NULL,
    start_point_id INTEGER,
    created BIGINT NOT NULL,
    replaced BIGINT,
    FOREIGN KEY (branch_id) REFERENCES branch (id) ON DELETE CASCADE,
    FOREIGN KEY (start_point_id) REFERENCES reference_version (id) ON DELETE
    SET NULL
);
INSERT INTO up_reference(
        id,
        uuid,
        branch_id,
        start_point_id,
        created,
        replaced
    )
SELECT id,
    uuid,
    branch_id,
    start_point_id,
    created,
    replaced
FROM reference;
DROP TABLE reference;
ALTER TABLE up_reference
    RENAME TO reference;
-- branch
CREATE TABLE up_branch (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    head_id INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    archived BIGINT,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (head_id) REFERENCES reference (id),
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO up_branch(
        id,
        uuid,
        project_id,
        name,
        slug,
        head_id,
        created,
        modified,
        archived
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug,
    id,
    created,
    modified,
    archived
FROM branch;
DROP TABLE branch;
ALTER TABLE up_branch
    RENAME TO branch;
-- report
CREATE TABLE up_report (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    project_id INTEGER NOT NULL,
    -- Connect to the reference and version individually and not to their reference_version
    -- This is necessary in order for cloned references to work
    -- Cloned references will *not* have a report tied to their specific reference_version
    -- So we don't want to have to query through the reference_version table
    -- to filter on the branch and list all of the versions
    reference_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    adapter INTEGER NOT NULL,
    start_time BIGINT NOT NULL,
    end_time BIGINT NOT NULL,
    created BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id),
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (reference_id) REFERENCES reference (id),
    FOREIGN KEY (version_id) REFERENCES version (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id)
);
INSERT INTO up_report(
        id,
        uuid,
        user_id,
        project_id,
        reference_id,
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
    project_id,
    branch_id,
    version_id,
    testbed_id,
    adapter,
    start_time,
    end_time,
    created
FROM report;
DROP TABLE report;
ALTER TABLE up_report
    RENAME TO report;
-- index
DROP INDEX index_report_benchmark;
CREATE INDEX index_report_testbed_end_time ON report(testbed_id, end_time);
CREATE INDEX index_report_benchmark ON report_benchmark(report_id, benchmark_id);
PRAGMA foreign_keys = on;