PRAGMA foreign_keys = off;
CREATE TABLE down_statistic (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    -- test kind: Z or T
    test INTEGER NOT NULL,
    -- max sample size
    max_sample_size BIGINT,
    -- time window
    window BIGINT,
    -- left side percentage
    left_side REAL,
    -- right side percentage
    right_side REAL
);
INSERT INTO down_statistic(
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
ALTER TABLE down_statistic
    RENAME TO statistic;
PRAGMA foreign_keys = on;