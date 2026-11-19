-- A user's asked-for change of email, one at a time, whose link names it by a secret the row keeps
-- only the hash of.
CREATE TABLE email_change (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL,
    session_id INTEGER,
    email TEXT NOT NULL,
    new_email TEXT NOT NULL,
    secret_hash TEXT NOT NULL,
    created BIGINT NOT NULL,
    expiration BIGINT NOT NULL,
    confirmed BIGINT,
    FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE CASCADE,
    FOREIGN KEY (session_id) REFERENCES session (id) ON DELETE SET NULL
);
CREATE UNIQUE INDEX index_email_change_user_id ON email_change(user_id);
CREATE UNIQUE INDEX index_email_change_secret_hash ON email_change(secret_hash);
CREATE INDEX index_email_change_session_id ON email_change(session_id);
