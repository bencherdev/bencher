PRAGMA foreign_keys = off;
-- branch
CREATE TABLE up_branch (
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
INSERT INTO up_branch(
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
    start_point_id,
    created,
    modified,
    (
        SELECT strftime('%s', datetime('now', 'utc'))
        WHERE name LIKE '%@detached%'
            OR name LIKE '%@%/hash/%'
            OR name LIKE '%@%/version/%'
    )
FROM branch;
DROP TABLE branch;
ALTER TABLE up_branch
    RENAME TO branch;
-- testbed
CREATE TABLE up_testbed (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    archived BIGINT,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
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
        modified,
        archived
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug,
    created,
    modified,
    null
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
    slug TEXT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    archived BIGINT,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO up_benchmark(
        id,
        uuid,
        project_id,
        name,
        slug,
        created,
        modified,
        archived
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug,
    created,
    modified,
    null
FROM benchmark;
DROP TABLE benchmark;
ALTER TABLE up_benchmark
    RENAME TO benchmark;
-- measure
CREATE TABLE up_measure (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    units TEXT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    archived BIGINT,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO up_measure(
        id,
        uuid,
        project_id,
        name,
        slug,
        units,
        created,
        modified,
        archived
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug,
    units,
    created,
    modified,
    null
FROM measure;
DROP TABLE measure;
ALTER TABLE up_measure
    RENAME TO measure;
PRAGMA foreign_keys = on;