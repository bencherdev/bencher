PRAGMA foreign_keys = off;
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
        modified
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug,
    start_point_id,
    created,
    modified
FROM branch;
DROP TABLE branch;
ALTER TABLE down_branch
    RENAME TO branch;
-- testbed
CREATE TABLE down_testbed (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO down_testbed(
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
    created,
    modified
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
    slug TEXT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO down_benchmark(
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
    created,
    modified
FROM benchmark;
DROP TABLE benchmark;
ALTER TABLE down_benchmark
    RENAME TO benchmark;
-- measure
CREATE TABLE down_measure (
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
INSERT INTO down_measure(
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
ALTER TABLE down_measure
    RENAME TO measure;
PRAGMA foreign_keys = on;