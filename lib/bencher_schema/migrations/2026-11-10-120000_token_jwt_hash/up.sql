-- `jwt_hash` is a function the API registers on the connection that runs the migrations.
-- A value that is already a hash is one this migration's down left behind, and it is kept.
-- `secure_delete` zeroes the old table's pages as they are freed, so no token outlives it
-- in the file.
PRAGMA foreign_keys = off;

DROP INDEX IF EXISTS index_token_user_id;

CREATE TABLE up_token (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    jwt_hash TEXT NOT NULL,
    creation BIGINT NOT NULL,
    expiration BIGINT NOT NULL,
    last_used BIGINT,
    revoked BIGINT,
    FOREIGN KEY (user_id) REFERENCES user (id)
);

INSERT INTO up_token(
        id,
        uuid,
        user_id,
        name,
        jwt_hash,
        creation,
        expiration,
        last_used,
        revoked
    )
SELECT id,
    uuid,
    user_id,
    name,
    CASE
        WHEN length(jwt) = 64 AND jwt NOT GLOB '*[^0-9a-f]*' THEN jwt
        ELSE jwt_hash(jwt)
    END,
    creation,
    expiration,
    NULL,
    revoked
FROM token;

PRAGMA secure_delete = on;
DROP TABLE token;
PRAGMA secure_delete = off;

ALTER TABLE up_token
    RENAME TO token;

CREATE UNIQUE INDEX index_token_jwt_hash ON token(jwt_hash);
CREATE INDEX index_token_user_id ON token(user_id);

PRAGMA foreign_keys = on;
