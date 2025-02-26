PRAGMA foreign_keys = off;
CREATE TABLE down_branch (
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
INSERT INTO down_branch(
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
FROM branch;
DROP TABLE branch;
ALTER TABLE down_branch
    RENAME TO branch;
PRAGMA foreign_keys = on;