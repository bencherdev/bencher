PRAGMA foreign_keys = off;

CREATE TABLE down_testbed (
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

INSERT INTO down_testbed(
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
    archived
FROM testbed;

DROP TABLE testbed;

ALTER TABLE down_testbed
    RENAME TO testbed;

CREATE INDEX index_testbed_project_created ON testbed(project_id, created);

PRAGMA foreign_keys = on;
