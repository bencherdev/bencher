CREATE TABLE runner_status (
    runner_id INTEGER PRIMARY KEY NOT NULL,
    availability INTEGER NOT NULL,
    reasons TEXT NOT NULL,
    since BIGINT,
    health TEXT,
    changed BIGINT NOT NULL,
    FOREIGN KEY (runner_id) REFERENCES runner (id) ON DELETE CASCADE
);
