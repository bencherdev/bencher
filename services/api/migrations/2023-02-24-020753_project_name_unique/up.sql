PRAGMA foreign_keys = off;
-- Deduplicate projects
DELETE FROM project
WHERE rowid NOT IN (
        SELECT min(rowid)
        FROM project
        GROUP BY organization_id,
            name
    );
CREATE TABLE up_project (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    organization_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    url TEXT,
    public BOOLEAN NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization (id) ON DELETE CASCADE,
    UNIQUE(organization_id, name)
);
INSERT INTO up_project(
        id,
        uuid,
        organization_id,
        name,
        slug,
        url,
        public
    )
SELECT id,
    uuid,
    organization_id,
    name,
    slug,
    url,
    public
FROM project;
DROP TABLE project;
ALTER TABLE up_project
    RENAME TO project;
PRAGMA foreign_keys = on;