PRAGMA foreign_keys = off;
-- alert
-- A missing boundary reads NULL, which fails the copy rather than dropping the alert.
CREATE TABLE up_alert (
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
INSERT INTO up_alert(
        id,
        uuid,
        project_id,
        threshold_id,
        boundary_id,
        boundary_limit,
        status,
        modified
    )
SELECT alert.id,
    alert.uuid,
    (
        SELECT threshold.project_id
        FROM boundary
            INNER JOIN threshold ON threshold.id = boundary.threshold_id
        WHERE boundary.id = alert.boundary_id
    ),
    (
        SELECT boundary.threshold_id
        FROM boundary
        WHERE boundary.id = alert.boundary_id
    ),
    alert.boundary_id,
    alert.boundary_limit,
    alert.status,
    alert.modified
FROM alert
ORDER BY alert.id;
DROP TABLE alert;
ALTER TABLE up_alert
    RENAME TO alert;
CREATE INDEX index_alert_boundary ON alert(boundary_id);
-- Deleting a threshold looks up its alerts, which seeks here rather than walking the table.
CREATE INDEX index_alert_threshold ON alert(threshold_id);
-- `modified DESC` serves the list's default order, and `threshold_id` spares the count the alert rows.
CREATE INDEX index_alert_project_status_modified ON alert(project_id, status, modified DESC, threshold_id);
PRAGMA foreign_keys = on;
