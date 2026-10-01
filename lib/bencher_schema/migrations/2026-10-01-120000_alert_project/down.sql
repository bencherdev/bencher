PRAGMA foreign_keys = off;
DROP INDEX IF EXISTS index_alert_threshold;
DROP INDEX IF EXISTS index_alert_project_status_modified;
CREATE TABLE down_alert (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    boundary_id INTEGER NOT NULL,
    boundary_limit BOOLEAN NOT NULL,
    status INTEGER NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (boundary_id) REFERENCES boundary (id) ON DELETE CASCADE
);
INSERT INTO down_alert(
        id,
        uuid,
        boundary_id,
        boundary_limit,
        status,
        modified
    )
SELECT id,
    uuid,
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
PRAGMA foreign_keys = on;
