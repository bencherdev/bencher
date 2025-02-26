PRAGMA foreign_keys = off;
CREATE TABLE up_metric (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    perf_id INTEGER NOT NULL,
    metric_kind_id INTEGER NOT NULL,
    value DOUBLE NOT NULL,
    lower_value DOUBLE,
    upper_value DOUBLE,
    FOREIGN KEY (perf_id) REFERENCES perf (id) ON DELETE CASCADE,
    FOREIGN KEY (metric_kind_id) REFERENCES metric_kind (id),
    UNIQUE(perf_id, metric_kind_id)
);
INSERT INTO up_metric(
        id,
        uuid,
        perf_id,
        metric_kind_id,
        value,
        lower_value,
        upper_value
    )
SELECT id,
    uuid,
    perf_id,
    metric_kind_id,
    value,
    lower_bound,
    upper_bound
FROM metric;
DROP TABLE metric;
ALTER TABLE up_metric
    RENAME TO metric;
PRAGMA foreign_keys = on;