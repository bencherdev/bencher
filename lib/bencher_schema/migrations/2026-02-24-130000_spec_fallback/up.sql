PRAGMA foreign_keys = off;

CREATE TABLE up_spec (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    architecture TEXT NOT NULL,
    cpu INTEGER NOT NULL CHECK (cpu > 0),
    memory BIGINT NOT NULL CHECK (memory > 0),
    disk BIGINT NOT NULL CHECK (disk > 0),
    network BOOLEAN NOT NULL DEFAULT 0,
    fallback BIGINT,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    archived BIGINT
);

INSERT INTO up_spec(id, uuid, name, slug, architecture, cpu, memory, disk, network, created, modified, archived)
SELECT id, uuid, name, slug, architecture, cpu, memory, disk, network, created, modified, archived
FROM spec;

DROP TABLE spec;
ALTER TABLE up_spec RENAME TO spec;

PRAGMA foreign_keys = on;

CREATE TRIGGER enforce_single_fallback_insert
BEFORE INSERT ON spec
WHEN NEW.fallback IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'only one spec can be the fallback')
    WHERE EXISTS (SELECT 1 FROM spec WHERE fallback IS NOT NULL);
END;

CREATE TRIGGER enforce_single_fallback_update
BEFORE UPDATE OF fallback ON spec
WHEN NEW.fallback IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'only one spec can be the fallback')
    WHERE EXISTS (SELECT 1 FROM spec WHERE fallback IS NOT NULL AND id != NEW.id);
END;
