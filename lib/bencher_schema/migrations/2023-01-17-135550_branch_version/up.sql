PRAGMA foreign_keys = off;
CREATE TABLE branch_version (
    id INTEGER PRIMARY KEY NOT NULL,
    branch_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    FOREIGN KEY (branch_id) REFERENCES branch (id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES version (id) ON DELETE CASCADE,
    UNIQUE(branch_id, version_id)
);
INSERT INTO branch_version(branch_id, version_id)
SELECT branch_id,
    id as version_id
FROM version;
CREATE TABLE up_version (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    number INTEGER NOT NULL,
    hash TEXT
);
INSERT INTO up_version(
        id,
        uuid,
        number,
        hash
    )
SELECT id,
    uuid,
    number,
    hash
FROM version;
DROP TABLE version;
ALTER TABLE up_version
    RENAME TO version;
PRAGMA foreign_keys = on;