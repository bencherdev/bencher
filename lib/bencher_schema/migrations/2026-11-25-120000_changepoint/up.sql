PRAGMA foreign_keys = off;
-- changepoint
-- A version at which a line stepped, found by a threshold's changepoint model. The line is the
-- head, testbed, spec, benchmark, variant, measure, and metric name; the variant is what makes one
-- series of the many a threshold's parameters match, and the spec is the hardware the line was
-- read under, so a testbed moved to a new spec starts a new line.
--
-- It goes with its project, as an alert does, and with its version, benchmark, or variant, as an
-- alert goes with its report: each of those is deleted only once no report holds it. The threshold
-- and the model it was judged against stay while a changepoint a user sees stands, as a boundary
-- holds them; a candidate is deleted with its threshold. A deleted spec leaves it with no spec, as
-- it leaves a report.
CREATE TABLE changepoint (
    id INTEGER PRIMARY KEY NOT NULL,
    uuid TEXT NOT NULL UNIQUE,
    project_id INTEGER NOT NULL,
    head_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    spec_id INTEGER,
    benchmark_id INTEGER NOT NULL,
    variant_id INTEGER NOT NULL,
    measure_id INTEGER NOT NULL,
    metric TEXT NOT NULL,
    version_id INTEGER NOT NULL,
    threshold_id INTEGER NOT NULL,
    model_id INTEGER NOT NULL,
    mean_before DOUBLE NOT NULL,
    mean_after DOUBLE NOT NULL,
    stddev_before DOUBLE NOT NULL,
    stddev_after DOUBLE NOT NULL,
    pvalue DOUBLE NOT NULL,
    tstatistic DOUBLE,
    degrees_of_freedom INTEGER NOT NULL,
    status INTEGER NOT NULL,
    created BIGINT NOT NULL,
    modified BIGINT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (head_id) REFERENCES head (id),
    FOREIGN KEY (testbed_id) REFERENCES testbed (id),
    FOREIGN KEY (spec_id) REFERENCES spec (id) ON DELETE SET NULL,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id) ON DELETE CASCADE,
    FOREIGN KEY (variant_id) REFERENCES variant (id) ON DELETE CASCADE,
    FOREIGN KEY (measure_id) REFERENCES measure (id),
    FOREIGN KEY (version_id) REFERENCES version (id) ON DELETE CASCADE,
    FOREIGN KEY (threshold_id) REFERENCES threshold (id),
    FOREIGN KEY (model_id) REFERENCES model (id)
);
-- The list reads one status of a project, and `modified DESC` serves its default order.
CREATE INDEX index_changepoint_project_status_modified ON changepoint(project_id, status, modified DESC);
-- The changepoints of one line, which the detector reads at every scan. A head replacement
-- reads the changepoints of its old head by the first column.
CREATE INDEX index_changepoint_line ON changepoint(head_id, testbed_id, spec_id, benchmark_id, variant_id, measure_id, metric);
-- Deleting any other parent looks up its changepoints by these.
CREATE INDEX index_changepoint_testbed ON changepoint(testbed_id);
CREATE INDEX index_changepoint_spec ON changepoint(spec_id);
CREATE INDEX index_changepoint_benchmark ON changepoint(benchmark_id);
CREATE INDEX index_changepoint_variant ON changepoint(variant_id);
CREATE INDEX index_changepoint_measure ON changepoint(measure_id);
CREATE INDEX index_changepoint_version ON changepoint(version_id);
CREATE INDEX index_changepoint_threshold ON changepoint(threshold_id);
CREATE INDEX index_changepoint_model ON changepoint(model_id);
-- changepoint_scan
-- What one scan measured at a changepoint, for each scan that found it up to its confirmation.
-- The changepoint's measured fields are its latest scan's while it is a candidate, and its
-- confirming scan's once it is confirmed. Its scans go with it.
CREATE TABLE changepoint_scan (
    id INTEGER PRIMARY KEY NOT NULL,
    changepoint_id INTEGER NOT NULL,
    mean_before DOUBLE NOT NULL,
    mean_after DOUBLE NOT NULL,
    stddev_before DOUBLE NOT NULL,
    stddev_after DOUBLE NOT NULL,
    pvalue DOUBLE NOT NULL,
    tstatistic DOUBLE,
    degrees_of_freedom INTEGER NOT NULL,
    created BIGINT NOT NULL,
    FOREIGN KEY (changepoint_id) REFERENCES changepoint (id) ON DELETE CASCADE
);
-- The scans of one changepoint, oldest first, which the view reads and a delete looks up.
CREATE INDEX index_changepoint_scan_changepoint_created ON changepoint_scan(changepoint_id, created);
PRAGMA foreign_keys = on;
