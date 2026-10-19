PRAGMA foreign_keys = off;
CREATE TABLE up_oauth_authorization (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    client_id TEXT NOT NULL,
    client_name TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    scope TEXT NOT NULL,
    resource TEXT NOT NULL,
    code_challenge TEXT NOT NULL,
    state TEXT,
    nonce TEXT,
    max_age BIGINT,
    refresh BOOLEAN NOT NULL,
    user_id INTEGER,
    user_key_id INTEGER,
    auth_time BIGINT,
    code_hash TEXT,
    grant_id INTEGER,
    created BIGINT NOT NULL,
    expiration BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE CASCADE,
    FOREIGN KEY (user_key_id) REFERENCES user_key (id) ON DELETE CASCADE,
    FOREIGN KEY (grant_id) REFERENCES oauth_grant (id) ON DELETE CASCADE
);
INSERT INTO up_oauth_authorization(
    id,
    uuid,
    client_id,
    client_name,
    redirect_uri,
    scope,
    resource,
    code_challenge,
    state,
    nonce,
    max_age,
    refresh,
    user_id,
    user_key_id,
    auth_time,
    code_hash,
    grant_id,
    created,
    expiration
)
SELECT id,
    uuid,
    client_id,
    client_name,
    redirect_uri,
    scope,
    resource,
    code_challenge,
    state,
    nonce,
    NULL,
    refresh,
    user_id,
    user_key_id,
    NULL,
    code_hash,
    grant_id,
    created,
    expiration
FROM oauth_authorization
ORDER BY id;
DROP TABLE oauth_authorization;
ALTER TABLE up_oauth_authorization
    RENAME TO oauth_authorization;
CREATE UNIQUE INDEX index_oauth_authorization_code_hash ON oauth_authorization(code_hash);
CREATE INDEX index_oauth_authorization_user_id ON oauth_authorization(user_id);
CREATE INDEX index_oauth_authorization_user_key_id ON oauth_authorization(user_key_id);
CREATE INDEX index_oauth_authorization_grant_id ON oauth_authorization(grant_id);
CREATE INDEX index_oauth_authorization_expiration ON oauth_authorization(expiration);
PRAGMA foreign_keys = on;
