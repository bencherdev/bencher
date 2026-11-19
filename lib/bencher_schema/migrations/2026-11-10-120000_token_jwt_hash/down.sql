-- The plaintext tokens are gone for good: this restores the table's shape, not its tokens.
-- Each row keeps its hash where its token was, so the up migration finds it again.
PRAGMA foreign_keys = off;

DROP INDEX IF EXISTS index_token_jwt_hash;
DROP INDEX IF EXISTS index_token_user_id;

CREATE TABLE down_token (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    jwt TEXT NOT NULL,
    creation BIGINT NOT NULL,
    expiration BIGINT NOT NULL,
    revoked BIGINT,
    FOREIGN KEY (user_id) REFERENCES user (id)
);

INSERT INTO down_token(
        id,
        uuid,
        user_id,
        name,
        jwt,
        creation,
        expiration,
        revoked
    )
SELECT id,
    uuid,
    user_id,
    name,
    jwt_hash,
    creation,
    expiration,
    revoked
FROM token;

DROP TABLE token;

ALTER TABLE down_token
    RENAME TO token;

CREATE INDEX index_token_user_id ON token(user_id);

PRAGMA foreign_keys = on;
