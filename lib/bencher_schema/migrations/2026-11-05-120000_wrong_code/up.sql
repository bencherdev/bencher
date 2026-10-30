CREATE TABLE wrong_code (
    id INTEGER PRIMARY KEY NOT NULL,
    email_hash TEXT NOT NULL,
    created BIGINT NOT NULL
);
CREATE INDEX index_wrong_code_email_hash ON wrong_code(email_hash, created);
CREATE INDEX index_wrong_code_created ON wrong_code(created);
