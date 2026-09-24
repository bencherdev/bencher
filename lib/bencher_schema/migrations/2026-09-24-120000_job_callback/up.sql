CREATE TABLE job_callback (
    job_id INTEGER PRIMARY KEY NOT NULL,
    request BLOB,
    state INTEGER NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    status INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (job_id) REFERENCES job (id) ON DELETE CASCADE
);
-- Startup recovery reads only unfinished callbacks: Pending (0) and Delivering (1).
CREATE INDEX index_job_callback_unfinished ON job_callback(state)
WHERE state = 0 OR state = 1;
