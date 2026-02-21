PRAGMA foreign_keys = off;

CREATE TABLE up_testbed (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    spec_id INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    archived BIGINT,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (spec_id) REFERENCES spec (id) ON DELETE SET NULL,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);

INSERT INTO up_testbed(
        id,
        uuid,
        project_id,
        name,
        slug,
        spec_id,
        created,
        modified,
        archived
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug,
    null,
    created,
    modified,
    archived
FROM testbed;

DROP TABLE testbed;

ALTER TABLE up_testbed
    RENAME TO testbed;

CREATE INDEX index_testbed_project_created ON testbed(project_id, created);

PRAGMA foreign_keys = on;
