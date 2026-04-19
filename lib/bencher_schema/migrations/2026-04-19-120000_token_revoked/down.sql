DROP INDEX IF EXISTS index_token_jwt;

PRAGMA foreign_keys = off;

-- token: remove revoked column
CREATE TABLE down_token (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    jwt TEXT NOT NULL,
    creation BIGINT NOT NULL,
    expiration BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id)
);

INSERT INTO down_token(
        id,
        uuid,
        user_id,
        name,
        jwt,
        creation,
        expiration
    )
SELECT id,
    uuid,
    user_id,
    name,
    jwt,
    creation,
    expiration
FROM token;

DROP TABLE token;

ALTER TABLE down_token
    RENAME TO token;

PRAGMA foreign_keys = on;
