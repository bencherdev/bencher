CREATE TABLE oauth_grant (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    user_key_id INTEGER,
    client_id TEXT NOT NULL,
    client_name TEXT NOT NULL,
    scope TEXT NOT NULL,
    resource TEXT NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    expiration BIGINT NOT NULL,
    revoked BIGINT,
    FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE CASCADE,
    FOREIGN KEY (user_key_id) REFERENCES user_key (id) ON DELETE CASCADE
);
CREATE INDEX index_oauth_grant_user_id ON oauth_grant(user_id);
CREATE INDEX index_oauth_grant_user_key_id ON oauth_grant(user_key_id);
CREATE TABLE oauth_token (
    id INTEGER PRIMARY KEY NOT NULL,
    grant_id INTEGER NOT NULL,
    kind INTEGER NOT NULL,
    token_hash TEXT NOT NULL,
    created BIGINT NOT NULL,
    expiration BIGINT NOT NULL,
    used BIGINT,
    FOREIGN KEY (grant_id) REFERENCES oauth_grant (id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX index_oauth_token_hash ON oauth_token(token_hash);
CREATE INDEX index_oauth_token_grant_id ON oauth_token(grant_id);
CREATE TABLE oauth_authorization (
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
    refresh BOOLEAN NOT NULL,
    user_id INTEGER,
    user_key_id INTEGER,
    code_hash TEXT,
    grant_id INTEGER,
    created BIGINT NOT NULL,
    expiration BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE CASCADE,
    FOREIGN KEY (user_key_id) REFERENCES user_key (id) ON DELETE CASCADE,
    FOREIGN KEY (grant_id) REFERENCES oauth_grant (id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX index_oauth_authorization_code_hash ON oauth_authorization(code_hash);
CREATE INDEX index_oauth_authorization_user_id ON oauth_authorization(user_id);
CREATE INDEX index_oauth_authorization_user_key_id ON oauth_authorization(user_key_id);
CREATE INDEX index_oauth_authorization_grant_id ON oauth_authorization(grant_id);
CREATE INDEX index_oauth_authorization_expiration ON oauth_authorization(expiration);
