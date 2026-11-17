-- `commit` is an SQLite keyword, so the name is always quoted.
CREATE TABLE "commit" (
    id INTEGER PRIMARY KEY NOT NULL,
    project_id INTEGER NOT NULL,
    hash TEXT NOT NULL,
    title TEXT,
    created BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    UNIQUE(project_id, hash)
);
