PRAGMA foreign_keys = off;
CREATE TABLE up_branch (
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
INSERT INTO up_branch(
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
    null,
    created,
    modified
FROM branch;
DROP TABLE branch;
ALTER TABLE up_branch
    RENAME TO branch;
PRAGMA foreign_keys = on;