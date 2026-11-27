PRAGMA foreign_keys = off;
-- model
-- Without the column there is no changepoint test, so each changepoint model goes, and
-- each changepoint slot that held one is emptied.
UPDATE threshold
SET changepoint_model_id = NULL
WHERE changepoint_model_id IN (
        SELECT id
        FROM model
        WHERE test = 50
    );
DELETE FROM model
WHERE test = 50;
CREATE TABLE down_model (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    threshold_id INTEGER NOT NULL,
    test INTEGER NOT NULL,
    min_sample_size BIGINT,
    max_sample_size BIGINT,
    window BIGINT,
    lower_boundary DOUBLE,
    upper_boundary DOUBLE,
    created BIGINT NOT NULL,
    replaced BIGINT,
    FOREIGN KEY (threshold_id) REFERENCES threshold (id) ON DELETE CASCADE
);
INSERT INTO down_model(
        id,
        uuid,
        threshold_id,
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
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
    created,
    replaced
FROM model;
DROP TABLE model;
ALTER TABLE down_model
    RENAME TO model;
CREATE INDEX index_model_threshold_created ON model(threshold_id, created);
-- report_warning
-- Older code reads no changepoint model (7), significance (8), or confirmations (9) warning.
DELETE FROM report_warning
WHERE resource IN (7, 8, 9);
PRAGMA foreign_keys = on;
