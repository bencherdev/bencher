-- Revert `runner.key_hash` → `runner.token_hash`.
DROP INDEX IF EXISTS index_runner_key_hash;
ALTER TABLE runner RENAME COLUMN key_hash TO token_hash;
CREATE UNIQUE INDEX index_runner_token_hash ON runner(token_hash);
