PRAGMA foreign_keys = off;
-- threshold
-- A threshold gains a second model slot, `changepoint_model_id`, beside `model_id`.
-- A model checks a metric and a changepoint model checks a line. Both lineages of
-- model rows hang off the one threshold through `model.threshold_id`, and the test of
-- a model tells them apart. NULL is an empty slot, which is what every existing row has.
--
-- The table is recreated so that the slot sits beside `model_id`. Every key and index
-- it had is kept as it stood.
CREATE TABLE up_threshold (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL,
    project_id INTEGER NOT NULL,
    branch_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    parameters BLOB,
    measure_id INTEGER NOT NULL,
    metric TEXT,
    model_id INTEGER,
    changepoint_model_id INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (branch_id) REFERENCES branch (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id),
    FOREIGN KEY (measure_id) REFERENCES measure (id),
    FOREIGN KEY (model_id) REFERENCES model (id),
    FOREIGN KEY (changepoint_model_id) REFERENCES model (id)
);
INSERT INTO up_threshold(
        id,
        uuid,
        project_id,
        branch_id,
        testbed_id,
        parameters,
        measure_id,
        metric,
        model_id,
        changepoint_model_id,
        created,
        modified
    )
SELECT id,
    uuid,
    project_id,
    branch_id,
    testbed_id,
    parameters,
    measure_id,
    metric,
    model_id,
    NULL,
    created,
    modified
FROM threshold;
DROP TABLE threshold;
ALTER TABLE up_threshold
    RENAME TO threshold;
CREATE UNIQUE INDEX index_threshold_uuid ON threshold(uuid);
CREATE UNIQUE INDEX index_threshold_dimensions ON threshold(
    branch_id,
    testbed_id,
    COALESCE(parameters, x''),
    measure_id,
    COALESCE(metric, 'value')
);
CREATE INDEX index_threshold_project_created ON threshold(project_id, created);
CREATE INDEX index_threshold_branch ON threshold(branch_id);
CREATE INDEX index_threshold_model ON threshold(model_id);
-- Deleting a model checks for a threshold that still holds it in either slot, which
-- seeks here for the changepoint slot.
CREATE INDEX index_threshold_changepoint_model ON threshold(changepoint_model_id);
CREATE INDEX index_threshold_measure ON threshold(measure_id);
CREATE INDEX index_threshold_testbed ON threshold(testbed_id);
PRAGMA foreign_keys = on;
