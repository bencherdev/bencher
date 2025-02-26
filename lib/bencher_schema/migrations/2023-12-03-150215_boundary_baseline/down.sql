PRAGMA foreign_keys = off;
CREATE TABLE down_boundary (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    threshold_id INTEGER NOT NULL,
    statistic_id INTEGER NOT NULL,
    metric_id INTEGER NOT NULL UNIQUE,
    average DOUBLE NOT NULL,
    lower_limit DOUBLE,
    upper_limit DOUBLE,
    FOREIGN KEY (threshold_id) REFERENCES threshold (id),
    FOREIGN KEY (statistic_id) REFERENCES statistic (id),
    FOREIGN KEY (metric_id) REFERENCES metric (id) ON DELETE CASCADE
);
INSERT INTO down_boundary(
        id,
        uuid,
        threshold_id,
        statistic_id,
        metric_id,
        average,
        lower_limit,
        upper_limit
    )
SELECT id,
    uuid,
    threshold_id,
    statistic_id,
    metric_id,
    baseline,
    lower_limit,
    upper_limit
FROM boundary;
DROP TABLE boundary;
ALTER TABLE down_boundary
    RENAME TO boundary;
PRAGMA foreign_keys = on;