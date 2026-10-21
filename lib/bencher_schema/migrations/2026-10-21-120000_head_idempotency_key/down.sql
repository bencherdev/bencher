PRAGMA foreign_keys = off;
DROP INDEX IF EXISTS index_head_idempotency_key;
DROP INDEX IF EXISTS index_head_branch_created;
DROP INDEX IF EXISTS index_head_start_point;
CREATE TABLE down_head (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    branch_id INTEGER NOT NULL,
    start_point_id INTEGER,
    created BIGINT NOT NULL,
    replaced BIGINT,
    FOREIGN KEY (branch_id) REFERENCES branch (id) ON DELETE CASCADE,
    FOREIGN KEY (start_point_id) REFERENCES head_version (id) ON DELETE
    SET NULL
);
INSERT INTO down_head(
        id,
        uuid,
        branch_id,
        start_point_id,
        created,
        replaced
    )
SELECT id,
    uuid,
    branch_id,
    start_point_id,
    created,
    replaced
FROM head
ORDER BY id;
DROP TABLE head;
ALTER TABLE down_head
    RENAME TO head;
CREATE INDEX index_head_branch_created ON head(branch_id, created);
CREATE INDEX index_head_start_point ON head(start_point_id);
PRAGMA foreign_keys = on;
