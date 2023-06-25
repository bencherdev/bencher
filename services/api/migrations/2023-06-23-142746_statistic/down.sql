PRAGMA foreign_keys = off;
CREATE TABLE down_statistic (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    test INTEGER NOT NULL,
    min_sample_size BIGINT,
    max_sample_size BIGINT,
    window BIGINT,
    left_side REAL,
    right_side REAL
);
INSERT INTO down_statistic(
        id,
        uuid,
        test,
        min_sample_size,
        max_sample_size,
        window,
        left_side,
        right_side
    )
SELECT id,
    uuid,
    test,
    min_sample_size,
    max_sample_size,
    window,
    lower_limit,
    upper_limit
FROM statistic;
DROP TABLE statistic;
ALTER TABLE down_statistic
    RENAME TO statistic;
PRAGMA foreign_keys = on;