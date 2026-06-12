CREATE TABLE user_key (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL,
    creation BIGINT NOT NULL,
    expiration BIGINT NOT NULL,
    revoked BIGINT,
    FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX index_user_key_hash ON user_key(key_hash);
CREATE INDEX index_user_key_user_id ON user_key(user_id);
