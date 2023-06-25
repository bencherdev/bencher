PRAGMA foreign_keys = off;
CREATE TABLE down_alert (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    perf_id INTEGER NOT NULL,
    threshold_id INTEGER NOT NULL,
    statistic_id INTEGER NOT NULL,
    side BOOLEAN NOT NULL,
    boundary REAL NOT NULL,
    outlier REAL NOT NULL,
    FOREIGN KEY (perf_id) REFERENCES perf (id),
    FOREIGN KEY (threshold_id) REFERENCES threshold (id),
    FOREIGN KEY (statistic_id) REFERENCES statistic (id)
);
INSERT INTO down_alert(
        id,
        uuid,
        perf_id,
        threshold_id,
        statistic_id,
        side,
        boundary,
        outlier
    )
SELECT id,
    uuid,
    (
        SELECT perf_id
        FROM boundary,
            metric
        WHERE boundary.metric_id = metric.id
        LIMIT 1
    ), (
        SELECT threshold_id
        FROM boundary
        WHERE boundary.uuid = alert.uuid
        LIMIT 1
    ), (
        SELECT statistic_id
        FROM boundary
        WHERE boundary.uuid = alert.uuid
        LIMIT 1
    ), side, 1.0, 0.0
FROM alert;
DROP TABLE alert;
ALTER TABLE down_alert
    RENAME TO alert;
PRAGMA foreign_keys = on;