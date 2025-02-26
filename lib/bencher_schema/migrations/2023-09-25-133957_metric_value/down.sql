PRAGMA foreign_keys = off;
CREATE TABLE down_metric (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    perf_id INTEGER NOT NULL,
    metric_kind_id INTEGER NOT NULL,
    value DOUBLE NOT NULL,
    lower_bound DOUBLE,
    upper_bound DOUBLE,
    FOREIGN KEY (perf_id) REFERENCES perf (id) ON DELETE CASCADE,
    FOREIGN KEY (metric_kind_id) REFERENCES metric_kind (id),
    UNIQUE(perf_id, metric_kind_id)
);
INSERT INTO down_metric(
        id,
        uuid,
        perf_id,
        metric_kind_id,
        value,
        lower_bound,
        upper_bound
    )
SELECT id,
    uuid,
    perf_id,
    metric_kind_id,
    value,
    lower_value,
    upper_value
FROM metric;
DROP TABLE metric;
ALTER TABLE down_metric
    RENAME TO metric;
PRAGMA foreign_keys = on;