PRAGMA foreign_keys = off;
CREATE TABLE down_report (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    adapter INTEGER NOT NULL,
    start_time BIGINT NOT NULL,
    end_time BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id),
    FOREIGN KEY (version_id) REFERENCES version (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id)
);
INSERT INTO down_report(
        id,
        uuid,
        user_id,
        version_id,
        testbed_id,
        adapter,
        start_time,
        end_time
    )
SELECT id,
    uuid,
    user_id,
    version_id,
    testbed_id,
    adapter,
    start_time,
    end_time
FROM report;
DROP TABLE report;
ALTER TABLE down_report
    RENAME TO report;
PRAGMA foreign_keys = on;