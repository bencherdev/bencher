PRAGMA foreign_keys = off;
CREATE TABLE boundary (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    perf_id INTEGER NOT NULL,
    threshold_id INTEGER NOT NULL,
    statistic_id INTEGER NOT NULL,
    left_side DOUBLE,
    right_side DOUBLE,
    FOREIGN KEY (perf_id) REFERENCES perf (id),
    FOREIGN KEY (threshold_id) REFERENCES threshold (id),
    FOREIGN KEY (statistic_id) REFERENCES statistic (id)
);
INSERT INTO boundary(
        id,
        uuid,
        perf_id,
        threshold_id,
        statistic_id,
        left_side,
        right_side
    )
SELECT id,
    uuid,
    perf_id,
    threshold_id,
    statistic_id,
    null,
    null
FROM alert;
PRAGMA foreign_keys = on;