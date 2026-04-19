-- Rename `runner.token_hash` → `runner.key_hash` (runner token → runner key).
DROP INDEX IF EXISTS index_runner_token_hash;
ALTER TABLE runner RENAME COLUMN token_hash TO key_hash;
CREATE UNIQUE INDEX index_runner_key_hash ON runner(key_hash);
