-- project: add bmf_version column
--
-- The column lives beside visibility, so the table is rebuilt around it.
-- Every project starts at 0, so a payload that declares no version reads as it
-- always has.
PRAGMA foreign_keys = off;

DROP INDEX IF EXISTS index_project_organization_created;

DROP INDEX IF EXISTS index_project_not_deleted;

CREATE TABLE up_project (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    organization_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    url TEXT,
    visibility INTEGER NOT NULL,
    bmf_version INTEGER NOT NULL DEFAULT 0,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    deleted BIGINT,
    FOREIGN KEY (organization_id) REFERENCES organization (id) ON DELETE CASCADE,
    UNIQUE(organization_id, name)
);

INSERT INTO up_project(
        id,
        uuid,
        organization_id,
        name,
        slug,
        url,
        visibility,
        created,
        modified,
        deleted
    )
SELECT id,
    uuid,
    organization_id,
    name,
    slug,
    url,
    visibility,
    created,
    modified,
    deleted
FROM project;

DROP TABLE project;

ALTER TABLE up_project
    RENAME TO project;

CREATE INDEX index_project_organization_created ON project(organization_id, created);

CREATE INDEX index_project_not_deleted ON project(id) WHERE deleted IS NULL;

PRAGMA foreign_keys = on;
