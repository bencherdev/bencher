ALTER TABLE token ADD COLUMN revoked BIGINT;

CREATE INDEX index_token_user_id ON token(user_id);
