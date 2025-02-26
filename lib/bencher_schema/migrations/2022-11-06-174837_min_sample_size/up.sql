PRAGMA foreign_keys = off;
CREATE TABLE up_statistic (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    -- test kind: Z or T
    test INTEGER NOT NULL,
    -- min sample size
    min_sample_size BIGINT,
    -- max sample size
    max_sample_size BIGINT,
    -- time window
    window BIGINT,
    -- left side percentage
    left_side REAL,
    -- right side percentage
    right_side REAL
);
INSERT INTO up_statistic(
        id,
        uuid,
        test,
        max_sample_size,
        window,
        left_side,
        right_side
    )
SELECT id,
    uuid,
    test,
    max_sample_size,
    window,
    left_side,
    right_side
FROM statistic;
DROP TABLE statistic;
ALTER TABLE up_statistic
    RENAME TO statistic;
PRAGMA foreign_keys = on;