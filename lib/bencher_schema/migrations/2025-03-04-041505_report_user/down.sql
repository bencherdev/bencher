PRAGMA foreign_keys = off;
-- report
CREATE TABLE down_report (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    project_id INTEGER NOT NULL,
    -- Connect to the head and version individually and not to their head_version
    -- This is necessary in order for cloned heads to work
    -- Cloned heads will *not* have a report tied to their specific head_version
    -- So we don't want to have to query through the head_version table
    -- to filter on the branch and list all of the versions
    head_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    adapter INTEGER NOT NULL,
    start_time BIGINT NOT NULL,
    end_time BIGINT NOT NULL,
    created BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id),
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (head_id) REFERENCES head (id),
    FOREIGN KEY (version_id) REFERENCES version (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id)
);
INSERT INTO down_report(
        id,
        uuid,
        user_id,
        project_id,
        head_id,
        version_id,
        testbed_id,
        adapter,
        start_time,
        end_time,
        created
    )
SELECT id,
    uuid,
    user_id,
    project_id,
    head_id,
    version_id,
    testbed_id,
    adapter,
    start_time,
    end_time,
    created
FROM report;
DROP TABLE report;
ALTER TABLE down_report
    RENAME TO report;
-- index
DROP INDEX IF EXISTS index_report_testbed_end_time;
DROP INDEX IF EXISTS index_report_benchmark;
DROP INDEX IF EXISTS index_report_version;
CREATE INDEX index_report_testbed_end_time ON report(testbed_id, end_time);
CREATE INDEX index_report_benchmark ON report_benchmark(report_id, benchmark_id);
CREATE INDEX index_report_version ON report(version_id, end_time);
PRAGMA foreign_keys = on;