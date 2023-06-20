-- Since `2023-01-17-135550_branch_version` it has not been possible to know the exact branch of a report.
-- This is due to the M-M relationship between the `version` and `branch` tables created by the introduction of the `branch_version` table.
-- This migration attempts to fix this issue by adding a `branch_id` column to the `report` table.
-- This migration will simply use the first branch that the version was associated with, that is the base branch.
-- This should be correct, as the report should always be for with the base branch.
PRAGMA foreign_keys = off;
CREATE TABLE up_report (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    branch_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    adapter INTEGER NOT NULL,
    start_time BIGINT NOT NULL,
    end_time BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id),
    FOREIGN KEY (version_id) REFERENCES version (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id)
);
INSERT INTO up_report(
        id,
        uuid,
        user_id,
        branch_id,
        version_id,
        testbed_id,
        adapter,
        start_time,
        end_time
    )
SELECT id,
    uuid,
    user_id,
    (
        SELECT branch_id
        FROM branch_version
        WHERE branch_version.version_id = report.version_id
        ORDER BY branch_version.id
        LIMIT 1
    ), version_id, testbed_id, adapter, start_time, end_time
FROM report;
DROP TABLE report;
ALTER TABLE up_report
    RENAME TO report;
PRAGMA foreign_keys = on;