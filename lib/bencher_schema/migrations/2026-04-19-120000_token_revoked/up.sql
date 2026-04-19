ALTER TABLE token ADD COLUMN revoked BIGINT;

CREATE INDEX index_token_jwt ON token(jwt);
