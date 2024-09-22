PRAGMA foreign_keys = off;
-- branch_version
CREATE TABLE branch_version (
    id INTEGER PRIMARY KEY NOT NULL,
    branch_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    FOREIGN KEY (branch_id) REFERENCES branch (id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES version (id) ON DELETE CASCADE,
    UNIQUE(branch_id, version_id)
);
INSERT INTO branch_version(
        id,
        branch_id,
        version_id
    )
SELECT id,
    reference_id,
    version_id
FROM reference_version;
DROP TABLE reference_version;
-- branch
CREATE TABLE down_branch (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    start_point_id INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    archived BIGINT,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (start_point_id) REFERENCES branch_version (id) ON DELETE
    SET NULL,
        UNIQUE(project_id, name),
        UNIQUE(project_id, slug)
);
INSERT INTO down_branch(
        id,
        uuid,
        project_id,
        name,
        slug,
        start_point_id,
        created,
        modified,
        archived
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug,
    (
        SELECT start_point_id
        FROM reference
        WHERE reference.id = branch.head_id
    ),
    created,
    modified,
    archived
FROM branch;
DROP TABLE branch;
ALTER TABLE down_branch
    RENAME TO branch;
-- report
CREATE TABLE down_report (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    project_id INTEGER NOT NULL,
    branch_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    adapter INTEGER NOT NULL,
    start_time BIGINT NOT NULL,
    end_time BIGINT NOT NULL,
    created BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id),
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (branch_id) REFERENCES branch (id),
    FOREIGN KEY (version_id) REFERENCES version (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id)
);
INSERT INTO down_report(
        id,
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
    )
SELECT id,
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
FROM report;
DROP TABLE report;
ALTER TABLE down_report
    RENAME TO report;
--  reference
DROP TABLE reference;
-- index
DROP INDEX IF EXISTS index_report_testbed_end_time;
DROP INDEX IF EXISTS index_report_benchmark;
CREATE INDEX index_report_testbed_end_time ON report(testbed_id, end_time);
CREATE INDEX index_report_benchmark ON report_benchmark(report_id, benchmark_id);
PRAGMA foreign_keys = on;