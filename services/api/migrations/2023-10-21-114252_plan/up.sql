PRAGMA foreign_keys = off;
CREATE TABLE up_organization (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    license TEXT UNIQUE,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL
);
CREATE TABLE plan (
    id INTEGER PRIMARY KEY NOT NULL,
    organization_id INTEGER NOT NULL UNIQUE,
    metered_plan TEXT UNIQUE,
    license_plan TEXT UNIQUE,
    license TEXT UNIQUE,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    constraint only_one_plan check (
        metered_plan is not null
        or license_plan is not null
        and not (
            metered_plan is not null
            and license_plan is not null
        )
    ),
    constraint metered_plan_no_license check (
        not (
            metered_plan is not null
            and license is not null
        )
    ),
    constraint license_plan_has_license check (
        not (
            license_plan is not null
            and license is null
        )
    )
);
INSERT INTO plan(
        organization_id,
        metered_plan,
        license_plan,
        license,
        created,
        modified
    )
SELECT id,
    subscription,
    null,
    license,
    created,
    modified
FROM organization
WHERE subscription IS NOT NULL;
INSERT INTO up_organization(
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
ALTER TABLE up_organization
    RENAME TO organization;
PRAGMA foreign_keys = on;