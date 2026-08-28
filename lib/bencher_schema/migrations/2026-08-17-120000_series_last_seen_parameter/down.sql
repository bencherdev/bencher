PRAGMA foreign_keys = off;
-- series_last_seen
CREATE TABLE down_series_last_seen (
    organization_id INTEGER NOT NULL,
    project_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    measure_id INTEGER NOT NULL,
    last_seen BIGINT NOT NULL,
    PRIMARY KEY (testbed_id, benchmark_id, measure_id),
    FOREIGN KEY (organization_id) REFERENCES organization (id) ON DELETE CASCADE,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (testbed_id) REFERENCES testbed (id) ON DELETE CASCADE,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id) ON DELETE CASCADE,
    FOREIGN KEY (measure_id) REFERENCES measure (id) ON DELETE CASCADE
);
-- Variants of one benchmark collapse back into one series, keeping the greatest
-- `last_seen` of the rows that merge, which is what the pre-parameter cache held.
INSERT INTO down_series_last_seen (
        organization_id,
        project_id,
        testbed_id,
        benchmark_id,
        measure_id,
        last_seen
    )
SELECT organization_id,
    project_id,
    testbed_id,
    benchmark_id,
    measure_id,
    MAX(last_seen)
FROM series_last_seen
GROUP BY testbed_id,
    benchmark_id,
    measure_id;
DROP TABLE series_last_seen;
ALTER TABLE down_series_last_seen
    RENAME TO series_last_seen;
CREATE INDEX index_series_last_seen_org_last_seen
    ON series_last_seen (organization_id, last_seen);
CREATE INDEX index_series_last_seen_project ON series_last_seen (project_id);
CREATE INDEX index_series_last_seen_benchmark ON series_last_seen (benchmark_id);
CREATE INDEX index_series_last_seen_measure ON series_last_seen (measure_id);
PRAGMA foreign_keys = on;
