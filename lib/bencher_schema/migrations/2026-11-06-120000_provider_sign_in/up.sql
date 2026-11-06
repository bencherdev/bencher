CREATE TABLE provider_state (
    id INTEGER PRIMARY KEY NOT NULL,
    provider INTEGER NOT NULL,
    state_hash TEXT NOT NULL,
    binding_hash TEXT NOT NULL,
    verifier TEXT NOT NULL,
    nonce TEXT,
    invite TEXT,
    claim TEXT,
    plan TEXT,
    authorization TEXT,
    created BIGINT NOT NULL,
    expiration BIGINT NOT NULL
);
CREATE UNIQUE INDEX index_provider_state_state_hash ON provider_state(state_hash);
CREATE INDEX index_provider_state_expiration ON provider_state(expiration);
CREATE TABLE provider_pin (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL,
    provider INTEGER NOT NULL,
    account_id TEXT NOT NULL,
    handle TEXT NOT NULL,
    created BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX index_provider_pin_user ON provider_pin(user_id, provider);
CREATE UNIQUE INDEX index_provider_pin_account ON provider_pin(provider, account_id);
PRAGMA foreign_keys = off;
CREATE TABLE up_pending_sign_in (
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
    pin_provider INTEGER,
    pin_account TEXT,
    pin_handle TEXT,
    created BIGINT NOT NULL,
    expiration BIGINT NOT NULL
);
INSERT INTO up_pending_sign_in(
        id,
        uuid,
        email,
        binding_hash,
        code_hash,
        link_hash,
        tries,
        invite,
        claim,
        plan,
        authorization,
        pin_provider,
        pin_account,
        pin_handle,
        created,
        expiration
    )
SELECT id,
    uuid,
    email,
    binding_hash,
    code_hash,
    link_hash,
    tries,
    invite,
    claim,
    plan,
    authorization,
    NULL,
    NULL,
    NULL,
    created,
    expiration
FROM pending_sign_in
ORDER BY id;
DROP INDEX IF EXISTS index_pending_sign_in_link_hash;
DROP INDEX IF EXISTS index_pending_sign_in_binding_hash;
DROP TABLE pending_sign_in;
ALTER TABLE up_pending_sign_in
    RENAME TO pending_sign_in;
CREATE UNIQUE INDEX index_pending_sign_in_binding_hash ON pending_sign_in(binding_hash);
CREATE UNIQUE INDEX index_pending_sign_in_link_hash ON pending_sign_in(link_hash);
PRAGMA foreign_keys = on;
