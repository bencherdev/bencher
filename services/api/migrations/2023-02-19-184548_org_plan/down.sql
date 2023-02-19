PRAGMA foreign_keys = off;
CREATE TABLE down_organization (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE
);
INSERT INTO down_organization(
        id,
        uuid,
        name,
        slug
    )
SELECT id,
    uuid,
    name,
    slug
FROM organization;
DROP TABLE organization;
ALTER TABLE down_organization
    RENAME TO organization;
PRAGMA foreign_keys = on;