-- `jwt_hash` and `jwt_refused` are functions the API registers on the connection that runs the
-- migrations, `jwt_refused` with the server's own token key.
-- A value that is already a hash is one this migration's down left behind, and it is kept.
-- `secure_delete` zeroes what the old table frees, so no token outlives it in the file.
PRAGMA foreign_keys = off;

PRAGMA secure_delete = on;

-- A token the server refused before it found tokens by their hash stays dead once found by it.
-- Its expiry is left to its row, and a hash down left behind was judged when it was hashed.
UPDATE token
SET revoked = CAST(strftime('%s', 'now') AS INTEGER)
WHERE revoked IS NULL
    AND NOT (length(jwt) = 64 AND jwt NOT GLOB '*[^0-9a-f]*')
    AND jwt_refused(
        jwt,
        (
            SELECT email
            FROM user
            WHERE user.id = token.user_id
        )
    );

-- Two requests in one second made the same token twice. Its copies merge into the lowest id,
-- revoked at the earliest time any copy was.
UPDATE token
SET revoked = copies.revoked
FROM (
        SELECT MIN(id) AS id,
            MIN(revoked) AS revoked
        FROM token
        GROUP BY jwt
        HAVING COUNT(*) > 1
    ) AS copies
WHERE token.id = copies.id;

DELETE FROM token
WHERE id NOT IN (
        SELECT MIN(id)
        FROM token
        GROUP BY jwt
    );

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

DROP TABLE token;
PRAGMA secure_delete = off;

ALTER TABLE up_token
    RENAME TO token;

CREATE UNIQUE INDEX index_token_jwt_hash ON token(jwt_hash);
CREATE INDEX index_token_user_id ON token(user_id);

PRAGMA foreign_keys = on;
