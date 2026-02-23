DROP INDEX IF EXISTS index_project_not_deleted;
DROP INDEX IF EXISTS index_organization_not_deleted;

PRAGMA foreign_keys = off;

-- project: remove deleted column
CREATE TABLE down_project (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    organization_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    url TEXT,
    visibility INTEGER NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization (id) ON DELETE CASCADE,
    UNIQUE(organization_id, name)
);

INSERT INTO down_project(
        id,
        uuid,
        organization_id,
        name,
        slug,
        url,
        visibility,
        created,
        modified
    )
SELECT id,
    uuid,
    organization_id,
    name,
    slug,
    url,
    visibility,
    created,
    modified
FROM project;

DROP TABLE project;

ALTER TABLE down_project
    RENAME TO project;

CREATE INDEX index_project_organization_created ON project(organization_id, created);

-- organization: remove deleted column
CREATE TABLE down_organization (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    license TEXT UNIQUE,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL
);

INSERT INTO down_organization(
        id,
        uuid,
        name,
        slug,
        license,
        created,
        modified
    )
SELECT id,
    uuid,
    name,
    slug,
    license,
    created,
    modified
FROM organization;

DROP TABLE organization;

ALTER TABLE down_organization
    RENAME TO organization;

PRAGMA foreign_keys = on;
