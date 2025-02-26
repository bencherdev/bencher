PRAGMA foreign_keys = off;
CREATE TABLE up_project (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    organization_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    url TEXT,
    public BOOLEAN NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization (id)
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