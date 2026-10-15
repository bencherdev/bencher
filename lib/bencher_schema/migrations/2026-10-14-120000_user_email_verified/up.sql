PRAGMA foreign_keys = off;
CREATE TABLE up_user (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE,
    email_verified BIGINT,
    admin BOOLEAN NOT NULL,
    locked BOOLEAN NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL
);
INSERT INTO up_user(
        id,
        uuid,
        name,
        slug,
        email,
        email_verified,
        admin,
        locked,
        created,
        modified
    )
SELECT id,
    uuid,
    name,
    slug,
    email,
    NULL,
    admin,
    locked,
    created,
    modified
FROM user
ORDER BY id;
DROP TABLE user;
ALTER TABLE up_user
    RENAME TO user;
PRAGMA foreign_keys = on;
