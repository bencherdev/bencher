PRAGMA foreign_keys = off;
-- model
-- A model gains `significance`, the chance of a false alarm its test may take, beside
-- its boundaries, and `confirmations`, the count of consecutive scans that confirm a
-- changepoint. Only the changepoint test takes them, so every existing row is NULL.
--
-- The table is recreated so that the columns sit after `upper_boundary`. Every key and
-- index it had is kept as it stood.
CREATE TABLE up_model (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    threshold_id INTEGER NOT NULL,
    test INTEGER NOT NULL,
    min_sample_size BIGINT,
    max_sample_size BIGINT,
    window BIGINT,
    lower_boundary DOUBLE,
    upper_boundary DOUBLE,
    significance DOUBLE,
    confirmations BIGINT,
    created BIGINT NOT NULL,
    replaced BIGINT,
    FOREIGN KEY (threshold_id) REFERENCES threshold (id) ON DELETE CASCADE
);
INSERT INTO up_model(
        id,
        uuid,
        threshold_id,
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
        significance,
        confirmations,
        created,
        replaced
    )
SELECT id,
    uuid,
    threshold_id,
    test,
    min_sample_size,
    max_sample_size,
    window,
    lower_boundary,
    upper_boundary,
    NULL,
    NULL,
    created,
    replaced
FROM model;
DROP TABLE model;
ALTER TABLE up_model
    RENAME TO model;
CREATE INDEX index_model_threshold_created ON model(threshold_id, created);
PRAGMA foreign_keys = on;
