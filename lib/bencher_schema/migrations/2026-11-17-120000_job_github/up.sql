CREATE TABLE job_github (
    job_id INTEGER PRIMARY KEY NOT NULL,
    repository_id BIGINT NOT NULL,
    head_sha TEXT NOT NULL,
    number BIGINT,
    ci_id TEXT,
    ci_only_thresholds BOOLEAN NOT NULL,
    ci_only_on_alert BOOLEAN NOT NULL,
    check_run_id BIGINT,
    state INTEGER NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    status INTEGER,
    failure INTEGER,
    approval_url TEXT,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (job_id) REFERENCES job (id) ON DELETE CASCADE
);
-- Startup recovery reads only unfinished posts, which are Pending (0).
CREATE INDEX index_job_github_pending ON job_github(state)
WHERE state = 0;
-- A post reads every job of its pull request on its head SHA.
CREATE INDEX index_job_github_pull_request ON job_github(repository_id, number, head_sha);
