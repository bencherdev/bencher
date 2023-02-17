PRAGMA foreign_keys = off;
-- Deduplicate branches
DELETE FROM branch
WHERE rowid NOT IN (
        SELECT min(rowid)
        FROM branch
        GROUP BY project_id,
            name
    );
CREATE TABLE up_branch (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    UNIQUE(project_id, name),
    UNIQUE(project_id, slug)
);
INSERT INTO up_branch(
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
FROM branch;
DROP TABLE branch;
ALTER TABLE up_branch
    RENAME TO branch;
PRAGMA foreign_keys = on;