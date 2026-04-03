PRAGMA foreign_keys = off;

DROP INDEX IF EXISTS index_report_idempotency_key;

CREATE TABLE down_report (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER,
    project_id INTEGER NOT NULL,
    head_id INTEGER NOT NULL,
    version_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    spec_id INTEGER,
    adapter INTEGER NOT NULL,
    start_time BIGINT NOT NULL,
    end_time BIGINT NOT NULL,
    created BIGINT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user (id),
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (head_id) REFERENCES head (id),
    FOREIGN KEY (version_id) REFERENCES version (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id),
    FOREIGN KEY (spec_id) REFERENCES spec (id) ON DELETE SET NULL
);

INSERT INTO down_report(
        id, uuid, user_id, project_id, head_id,
        version_id, testbed_id, spec_id, adapter, start_time, end_time, created
    )
SELECT id, uuid, user_id, project_id, head_id,
    version_id, testbed_id, spec_id, adapter, start_time, end_time, created
FROM report;

DROP TABLE report;
ALTER TABLE down_report RENAME TO report;

CREATE INDEX index_report_testbed_end_time ON report(testbed_id, end_time);
CREATE INDEX index_report_version ON report(version_id, end_time);
CREATE INDEX index_report_project_end_time ON report(project_id, end_time);
CREATE INDEX index_report_project_created ON report(project_id, created);
CREATE INDEX index_report_version_testbed ON report(version_id, testbed_id);
CREATE INDEX index_report_spec ON report(spec_id);
CREATE INDEX index_report_head ON report(head_id);

PRAGMA foreign_keys = on;
