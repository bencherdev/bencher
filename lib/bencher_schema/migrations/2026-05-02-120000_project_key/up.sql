CREATE TABLE project_key (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    creator_id INTEGER,
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL,
    creation BIGINT NOT NULL,
    expiration BIGINT NOT NULL,
    revoked BIGINT,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (creator_id) REFERENCES user (id) ON DELETE SET NULL
);
CREATE UNIQUE INDEX index_project_key_hash ON project_key(key_hash);
CREATE INDEX index_project_key_project_id ON project_key(project_id);
