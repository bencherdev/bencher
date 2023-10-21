PRAGMA foreign_keys = off;
CREATE TABLE down_organization (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    subscription TEXT UNIQUE,
    license TEXT UNIQUE,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    constraint zero_or_one_plan check (
        not (
            subscription is not null
            and license is not null
        )
    )
);
INSERT INTO down_organization(
        id,
        uuid,
        name,
        slug,
        subscription,
        license,
        created,
        modified
    )
SELECT organization.id,
    organization.uuid,
    organization.name,
    organization.slug,
    plan.metered_plan,
    organization.license,
    organization.created,
    organization.modified
FROM organization
    LEFT JOIN plan on plan.organization_id = organization.id;
DROP TABLE plan;
DROP TABLE organization;
ALTER TABLE down_organization
    RENAME TO organization;
PRAGMA foreign_keys = on;