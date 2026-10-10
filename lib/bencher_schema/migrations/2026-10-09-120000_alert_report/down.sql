PRAGMA foreign_keys = off;
DROP INDEX IF EXISTS index_alert_report;
DROP INDEX IF EXISTS index_alert_project_created;
CREATE TABLE down_alert (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    threshold_id INTEGER NOT NULL,
    boundary_id INTEGER NOT NULL,
    boundary_limit BOOLEAN NOT NULL,
    status INTEGER NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (threshold_id) REFERENCES threshold (id),
    FOREIGN KEY (boundary_id) REFERENCES boundary (id) ON DELETE CASCADE
);
INSERT INTO down_alert(
        id,
        uuid,
        project_id,
        threshold_id,
        boundary_id,
        boundary_limit,
        status,
        modified
    )
SELECT id,
    uuid,
    project_id,
    threshold_id,
    boundary_id,
    boundary_limit,
    status,
    modified
FROM alert
ORDER BY id;
DROP TABLE alert;
ALTER TABLE down_alert
    RENAME TO alert;
CREATE INDEX index_alert_boundary ON alert(boundary_id);
CREATE INDEX index_alert_threshold ON alert(threshold_id);
CREATE INDEX index_alert_project_status_modified ON alert(project_id, status, modified DESC, threshold_id);
PRAGMA foreign_keys = on;
