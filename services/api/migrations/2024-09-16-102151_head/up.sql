PRAGMA foreign_keys = off;
-- head
CREATE TABLE head (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    branch_id INTEGER NOT NULL,
    start_point_id INTEGER,
    created BIGINT NOT NULL,
    replaced BIGINT,
    FOREIGN KEY (branch_id) REFERENCES branch (id) ON DELETE CASCADE,
    FOREIGN KEY (start_point_id) REFERENCES head_version (id) ON DELETE
    SET NULL
);
INSERT INTO head(
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
-- head_version
CREATE TABLE head_version (
    id INTEGER PRIMARY KEY NOT NULL,
    head_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    FOREIGN KEY (head_id) REFERENCES head (id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES version (id) ON DELETE CASCADE,
    UNIQUE(head_id, version_id)
);
INSERT INTO head_version(
        id,
        head_id,
        version_id
    )
SELECT id,
    branch_id,
    version_id
FROM branch_version;
DROP TABLE branch_version;
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
    FOREIGN KEY (head_id) REFERENCES head (id),
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
    -- Connect to the head and version individually and not to their head_version
    -- This is necessary in order for cloned heads to work
    -- Cloned heads will *not* have a report tied to their specific head_version
    -- So we don't want to have to query through the head_version table
    -- to filter on the branch and list all of the versions
    head_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    adapter INTEGER NOT NULL,
    start_time BIGINT NOT NULL,
    end_time BIGINT NOT NULL,
    created BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id),
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (head_id) REFERENCES head (id),
    FOREIGN KEY (version_id) REFERENCES version (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id)
);
INSERT INTO up_report(
        id,
        uuid,
        user_id,
        project_id,
        head_id,
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
DROP INDEX IF EXISTS index_report_testbed_end_time;
DROP INDEX IF EXISTS index_report_benchmark;
CREATE INDEX index_report_testbed_end_time ON report(testbed_id, end_time);
CREATE INDEX index_report_benchmark ON report_benchmark(report_id, benchmark_id);
-- new indexes
CREATE INDEX index_measure_project ON measure(uuid, project_id);
CREATE INDEX index_report_version ON report(version_id, end_time);
PRAGMA foreign_keys = on;