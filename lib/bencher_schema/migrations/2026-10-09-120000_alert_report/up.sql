PRAGMA foreign_keys = off;
-- A missing report reads NULL, which fails the copy rather than dropping the alert.
CREATE TABLE up_alert (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    report_id INTEGER NOT NULL,
    threshold_id INTEGER NOT NULL,
    boundary_id INTEGER NOT NULL,
    boundary_limit BOOLEAN NOT NULL,
    status INTEGER NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE,
    FOREIGN KEY (threshold_id) REFERENCES threshold (id),
    FOREIGN KEY (boundary_id) REFERENCES boundary (id) ON DELETE CASCADE
);
INSERT INTO up_alert(
        id,
        uuid,
        project_id,
        report_id,
        threshold_id,
        boundary_id,
        boundary_limit,
        status,
        created,
        modified
    )
SELECT alert.id,
    alert.uuid,
    alert.project_id,
    report.id,
    alert.threshold_id,
    alert.boundary_id,
    alert.boundary_limit,
    alert.status,
    report.created,
    alert.modified
FROM alert
    LEFT JOIN boundary ON boundary.id = alert.boundary_id
    LEFT JOIN metric ON metric.id = boundary.metric_id
    LEFT JOIN report_benchmark ON report_benchmark.id = metric.report_benchmark_id
    LEFT JOIN report ON report.id = report_benchmark.report_id
ORDER BY alert.id;
DROP TABLE alert;
ALTER TABLE up_alert
    RENAME TO alert;
CREATE INDEX index_alert_boundary ON alert(boundary_id);
-- Deleting a threshold looks up its alerts, which seeks here rather than walking the table.
CREATE INDEX index_alert_threshold ON alert(threshold_id);
-- `modified DESC` serves the list's default order, and `threshold_id` spares the count the alert rows.
CREATE INDEX index_alert_project_status_modified ON alert(project_id, status, modified DESC, threshold_id);
-- Deleting a report looks up its alerts, and a page of reports reads its alerts, by report.
CREATE INDEX index_alert_report ON alert(report_id);
-- A project's alerts in report order, so a created window is a range, and `report_id`, `status`, and `threshold_id` spare the counts and pages the alert rows.
CREATE INDEX index_alert_project_created ON alert(project_id, created, report_id, status, threshold_id);
PRAGMA foreign_keys = on;
