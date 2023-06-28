PRAGMA foreign_keys = off;
CREATE TABLE boundary (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    threshold_id INTEGER NOT NULL,
    statistic_id INTEGER NOT NULL,
    metric_id INTEGER NOT NULL UNIQUE,
    lower_limit DOUBLE,
    upper_limit DOUBLE,
    FOREIGN KEY (metric_id) REFERENCES metric (id),
    FOREIGN KEY (threshold_id) REFERENCES threshold (id),
    FOREIGN KEY (statistic_id) REFERENCES statistic (id)
);
INSERT INTO boundary(
        id,
        uuid,
        threshold_id,
        statistic_id,
        metric_id,
        lower_limit,
        upper_limit
    )
SELECT id,
    uuid,
    threshold_id,
    statistic_id,
    (
        SELECT id
        FROM metric
        WHERE metric.perf_id = alert.perf_id
        LIMIT 1
    ), null,
    null
FROM alert;
PRAGMA foreign_keys = on;