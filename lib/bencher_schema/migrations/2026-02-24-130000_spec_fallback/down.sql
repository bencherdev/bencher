DROP TRIGGER IF EXISTS enforce_single_fallback_insert;
DROP TRIGGER IF EXISTS enforce_single_fallback_update;

PRAGMA foreign_keys = off;

CREATE TABLE down_spec (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    architecture TEXT NOT NULL,
    cpu INTEGER NOT NULL CHECK (cpu > 0),
    memory BIGINT NOT NULL CHECK (memory > 0),
    disk BIGINT NOT NULL CHECK (disk > 0),
    network BOOLEAN NOT NULL DEFAULT 0,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    archived BIGINT
);

INSERT INTO down_spec(id, uuid, name, slug, architecture, cpu, memory, disk, network, created, modified, archived)
SELECT id, uuid, name, slug, architecture, cpu, memory, disk, network, created, modified, archived
FROM spec;

DROP TABLE spec;
ALTER TABLE down_spec RENAME TO spec;

PRAGMA foreign_keys = on;
