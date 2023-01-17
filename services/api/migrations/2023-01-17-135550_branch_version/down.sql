PRAGMA foreign_keys = off;
DROP TABLE branch_version;
DROP TABLE version;
CREATE TABLE version (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    branch_id INTEGER NOT NULL,
    number INTEGER NOT NULL,
    hash TEXT,
    FOREIGN KEY (branch_id) REFERENCES branch (id),
    UNIQUE(branch_id, number)
);
PRAGMA foreign_keys = on;