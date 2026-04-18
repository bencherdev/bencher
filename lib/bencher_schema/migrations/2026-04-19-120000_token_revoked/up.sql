ALTER TABLE token ADD COLUMN revoked BIGINT;

CREATE INDEX index_token_not_revoked ON token(id) WHERE revoked IS NULL;
