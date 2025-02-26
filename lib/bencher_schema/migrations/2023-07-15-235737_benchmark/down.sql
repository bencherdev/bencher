PRAGMA foreign_keys = off;
CREATE TABLE down_benchmark (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    created BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    UNIQUE(project_id, name)
);
INSERT INTO down_benchmark(
        id,
        uuid,
        project_id,
        name,
        created
    )
SELECT id,
    uuid,
    project_id,
    name,
    created
FROM benchmark;
DROP TABLE benchmark;
ALTER TABLE down_benchmark
    RENAME TO benchmark;
PRAGMA foreign_keys = on;