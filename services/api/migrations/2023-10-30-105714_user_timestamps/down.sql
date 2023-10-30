PRAGMA foreign_keys = off;
CREATE TABLE down_user (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE,
    admin BOOLEAN NOT NULL,
    locked BOOLEAN NOT NULL
);
INSERT INTO down_user(
        id,
        uuid,
        name,
        slug,
        email,
        admin,
        locked
    )
SELECT id,
    uuid,
    name,
    slug,
    email,
    admin,
    locked
FROM user;
DROP TABLE user;
ALTER TABLE down_user
    RENAME TO user;
PRAGMA foreign_keys = on;