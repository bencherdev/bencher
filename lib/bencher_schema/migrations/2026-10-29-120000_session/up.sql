CREATE TABLE session (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    token_hash TEXT NOT NULL,
    method INTEGER NOT NULL,
    device TEXT,
    created BIGINT NOT NULL,
    proved BIGINT NOT NULL,
    last_used BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX index_session_token_hash ON session(token_hash);
CREATE INDEX index_session_user_id ON session(user_id);
