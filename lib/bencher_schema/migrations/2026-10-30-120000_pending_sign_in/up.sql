CREATE TABLE pending_sign_in (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL,
    binding_hash TEXT NOT NULL,
    code_hash TEXT NOT NULL,
    link_hash TEXT NOT NULL,
    tries INTEGER NOT NULL,
    invite TEXT,
    claim TEXT,
    plan TEXT,
    authorization TEXT,
    created BIGINT NOT NULL,
    expiration BIGINT NOT NULL
);
CREATE UNIQUE INDEX index_pending_sign_in_binding_hash ON pending_sign_in(binding_hash);
CREATE UNIQUE INDEX index_pending_sign_in_link_hash ON pending_sign_in(link_hash);
