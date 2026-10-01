CREATE TABLE report_warning (
    id INTEGER PRIMARY KEY NOT NULL,
    report_id INTEGER NOT NULL,
    resource INTEGER NOT NULL,
    action INTEGER NOT NULL,
    count INTEGER NOT NULL,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE,
    UNIQUE(report_id, resource, action)
);
