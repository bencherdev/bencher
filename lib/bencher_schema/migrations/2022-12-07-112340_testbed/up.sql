PRAGMA foreign_keys = off;
CREATE TABLE up_testbed (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id),
    UNIQUE(project_id, slug)
);
INSERT INTO up_testbed(
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
ALTER TABLE up_testbed
    RENAME TO testbed;
PRAGMA foreign_keys = on;