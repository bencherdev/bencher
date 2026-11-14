-- An invitation's link names its row by a secret, of which the row keeps only the hash.
-- A sign-in started with an invitation keeps the row's ID in place of the invitation's token,
-- and a sign-in under way when this runs keeps none, since no token names a row.
PRAGMA foreign_keys = off;
CREATE TABLE invite (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    organization_id INTEGER NOT NULL,
    email TEXT NOT NULL,
    role TEXT NOT NULL,
    inviter_id INTEGER NOT NULL,
    secret_hash TEXT NOT NULL,
    created BIGINT NOT NULL,
    expiration BIGINT NOT NULL,
    FOREIGN KEY (organization_id) REFERENCES organization (id) ON DELETE CASCADE,
    FOREIGN KEY (inviter_id) REFERENCES user (id) ON DELETE CASCADE,
    UNIQUE(organization_id, email)
);
CREATE UNIQUE INDEX index_invite_secret_hash ON invite(secret_hash);
CREATE INDEX index_invite_inviter_id ON invite(inviter_id);
CREATE TABLE up_pending_sign_in (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    session_id INTEGER,
    email TEXT NOT NULL,
    binding_hash TEXT NOT NULL,
    code_hash TEXT NOT NULL,
    link_hash TEXT NOT NULL,
    tries INTEGER NOT NULL,
    invite_id INTEGER,
    claim TEXT,
    plan TEXT,
    authorization TEXT,
    pin_provider INTEGER,
    pin_account TEXT,
    pin_handle TEXT,
    created BIGINT NOT NULL,
    expiration BIGINT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES session (id) ON DELETE CASCADE,
    FOREIGN KEY (invite_id) REFERENCES invite (id) ON DELETE SET NULL
);
INSERT INTO up_pending_sign_in(
        id,
        uuid,
        session_id,
        email,
        binding_hash,
        code_hash,
        link_hash,
        tries,
        invite_id,
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
    session_id,
    email,
    binding_hash,
    code_hash,
    link_hash,
    tries,
    NULL,
    claim,
    plan,
    authorization,
    pin_provider,
    pin_account,
    pin_handle,
    created,
    expiration
FROM pending_sign_in
ORDER BY id;
DROP INDEX IF EXISTS index_pending_sign_in_session_id;
DROP INDEX IF EXISTS index_pending_sign_in_link_hash;
DROP INDEX IF EXISTS index_pending_sign_in_binding_hash;
DROP TABLE pending_sign_in;
ALTER TABLE up_pending_sign_in
    RENAME TO pending_sign_in;
CREATE UNIQUE INDEX index_pending_sign_in_binding_hash ON pending_sign_in(binding_hash);
CREATE UNIQUE INDEX index_pending_sign_in_link_hash ON pending_sign_in(link_hash);
CREATE INDEX index_pending_sign_in_session_id ON pending_sign_in(session_id);
CREATE INDEX index_pending_sign_in_invite_id ON pending_sign_in(invite_id);
CREATE TABLE up_provider_state (
    id INTEGER PRIMARY KEY NOT NULL,
    session_id INTEGER,
    provider INTEGER NOT NULL,
    state_hash TEXT NOT NULL,
    binding_hash TEXT NOT NULL,
    verifier TEXT NOT NULL,
    nonce TEXT,
    invite_id INTEGER,
    claim TEXT,
    plan TEXT,
    authorization TEXT,
    created BIGINT NOT NULL,
    expiration BIGINT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES session (id) ON DELETE CASCADE,
    FOREIGN KEY (invite_id) REFERENCES invite (id) ON DELETE SET NULL
);
INSERT INTO up_provider_state(
        id,
        session_id,
        provider,
        state_hash,
        binding_hash,
        verifier,
        nonce,
        invite_id,
        claim,
        plan,
        authorization,
        created,
        expiration
    )
SELECT id,
    session_id,
    provider,
    state_hash,
    binding_hash,
    verifier,
    nonce,
    NULL,
    claim,
    plan,
    authorization,
    created,
    expiration
FROM provider_state
ORDER BY id;
DROP INDEX IF EXISTS index_provider_state_session_id;
DROP INDEX IF EXISTS index_provider_state_expiration;
DROP INDEX IF EXISTS index_provider_state_state_hash;
DROP TABLE provider_state;
ALTER TABLE up_provider_state
    RENAME TO provider_state;
CREATE UNIQUE INDEX index_provider_state_state_hash ON provider_state(state_hash);
CREATE INDEX index_provider_state_expiration ON provider_state(expiration);
CREATE INDEX index_provider_state_session_id ON provider_state(session_id);
CREATE INDEX index_provider_state_invite_id ON provider_state(invite_id);
PRAGMA foreign_keys = on;
