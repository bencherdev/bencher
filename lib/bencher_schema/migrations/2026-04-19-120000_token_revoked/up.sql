ALTER TABLE token ADD COLUMN revoked BIGINT;

CREATE UNIQUE INDEX index_token_active_jwt ON token(jwt) WHERE revoked IS NULL;
