PRAGMA foreign_keys = off;
CREATE TABLE down_pending_sign_in (
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
INSERT INTO down_pending_sign_in(
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
    created,
    expiration
FROM pending_sign_in
ORDER BY id;
DROP INDEX IF EXISTS index_pending_sign_in_link_hash;
DROP INDEX IF EXISTS index_pending_sign_in_binding_hash;
DROP TABLE pending_sign_in;
ALTER TABLE down_pending_sign_in
    RENAME TO pending_sign_in;
CREATE UNIQUE INDEX index_pending_sign_in_binding_hash ON pending_sign_in(binding_hash);
CREATE UNIQUE INDEX index_pending_sign_in_link_hash ON pending_sign_in(link_hash);
PRAGMA foreign_keys = on;
DROP INDEX IF EXISTS index_provider_pin_account;
DROP INDEX IF EXISTS index_provider_pin_user;
DROP TABLE IF EXISTS provider_pin;
DROP INDEX IF EXISTS index_provider_state_expiration;
DROP INDEX IF EXISTS index_provider_state_state_hash;
DROP TABLE IF EXISTS provider_state;
