PRAGMA foreign_keys = off;
CREATE TABLE up_organization (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    subscription TEXT UNIQUE,
    license TEXT UNIQUE,
    constraint zero_or_one_plan check (
        not (
            subscription is not null
            and license is not null
        )
    )
);
INSERT INTO up_organization(
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
ALTER TABLE up_organization
    RENAME TO organization;
PRAGMA foreign_keys = on;