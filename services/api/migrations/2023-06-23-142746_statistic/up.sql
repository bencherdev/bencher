PRAGMA foreign_keys = off;
CREATE TABLE up_statistic (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    test INTEGER NOT NULL,
    min_sample_size BIGINT,
    max_sample_size BIGINT,
    window BIGINT,
    lower_boundary DOUBLE,
    upper_boundary DOUBLE
);
INSERT INTO up_statistic(
        id,
        uuid,
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary
    )
SELECT id,
    uuid,
    test,
    min_sample_size,
    max_sample_size,
    window,
    left_side,
    right_side
FROM statistic;
DROP TABLE statistic;
ALTER TABLE up_statistic
    RENAME TO statistic;
PRAGMA foreign_keys = on;