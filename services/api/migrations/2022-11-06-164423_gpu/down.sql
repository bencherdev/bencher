PRAGMA foreign_keys = off;
CREATE TABLE IF NOT EXISTS down_testbed (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    os_name TEXT,
    os_version TEXT,
    runtime_name TEXT,
    runtime_version TEXT,
    cpu TEXT,
    ram TEXT,
    disk TEXT,
    FOREIGN KEY (project_id) REFERENCES project (id),
    UNIQUE(project_id, slug)
);
INSERT INTO down_testbed(
        id,
        uuid,
        project_id,
        name,
        slug,
        os_name,
        os_version,
        runtime_name,
        runtime_version,
        cpu,
        ram,
        disk
    )
SELECT id,
    uuid,
    project_id,
    name,
    slug,
    os_name,
    os_version,
    runtime_name,
    runtime_version,
    cpu,
    ram,
    disk
FROM testbed;
DROP TABLE testbed;
ALTER TABLE down_testbed
    RENAME TO testbed;
PRAGMA foreign_keys = on;