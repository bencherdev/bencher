PRAGMA foreign_keys = off;
CREATE TABLE up_benchmark (
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
INSERT INTO up_benchmark(
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
    uuid,
    created,
    created
FROM benchmark;
DROP TABLE benchmark;
ALTER TABLE up_benchmark
    RENAME TO benchmark;
PRAGMA foreign_keys = on;