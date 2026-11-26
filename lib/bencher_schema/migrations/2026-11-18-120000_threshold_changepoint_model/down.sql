PRAGMA foreign_keys = off;
-- threshold
-- Back to one model slot. The models a changepoint slot held stay as rows of their
-- threshold, and the slot that pointed at the current one goes.
CREATE TABLE down_threshold (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL,
    project_id INTEGER NOT NULL,
    branch_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    parameters BLOB,
    measure_id INTEGER NOT NULL,
    metric TEXT,
    model_id INTEGER,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (branch_id) REFERENCES branch (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id),
    FOREIGN KEY (measure_id) REFERENCES measure (id),
    FOREIGN KEY (model_id) REFERENCES model (id)
);
INSERT INTO down_threshold(
        id,
        uuid,
        project_id,
        branch_id,
        testbed_id,
        parameters,
        measure_id,
        metric,
        model_id,
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
    created,
    modified
FROM threshold;
DROP TABLE threshold;
ALTER TABLE down_threshold
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
CREATE INDEX index_threshold_measure ON threshold(measure_id);
CREATE INDEX index_threshold_testbed ON threshold(testbed_id);
PRAGMA foreign_keys = on;
