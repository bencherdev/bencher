PRAGMA foreign_keys = off;
-- series_last_seen
-- A series is a distinct `(testbed, benchmark, parameter, measure)` now: each grid
-- point of a benchmark has its own history and bills as its own series. A project
-- whose grid points are currently flat benchmarks bills exactly what it billed
-- before, since every one of its `report_benchmark` rows rides its benchmark's
-- empty parameter set.
--
-- Named metric values are deliberately absent from the key: they collapse into
-- their measure's series and are not billed.
CREATE TABLE up_series_last_seen (
    organization_id INTEGER NOT NULL,
    project_id INTEGER NOT NULL,
    testbed_id INTEGER NOT NULL,
    benchmark_id INTEGER NOT NULL,
    parameter_id INTEGER NOT NULL,
    measure_id INTEGER NOT NULL,
    last_seen BIGINT NOT NULL,
    PRIMARY KEY (testbed_id, benchmark_id, parameter_id, measure_id),
    FOREIGN KEY (organization_id) REFERENCES organization (id) ON DELETE CASCADE,
    FOREIGN KEY (project_id) REFERENCES project (id) ON DELETE CASCADE,
    FOREIGN KEY (testbed_id) REFERENCES testbed (id) ON DELETE CASCADE,
    FOREIGN KEY (benchmark_id) REFERENCES benchmark (id) ON DELETE CASCADE,
    FOREIGN KEY (parameter_id) REFERENCES parameter (id) ON DELETE CASCADE,
    FOREIGN KEY (measure_id) REFERENCES measure (id) ON DELETE CASCADE
);
-- Every existing row is a series over flat benchmarks, so it maps to its
-- benchmark's empty parameter set. `last_seen` carries over untouched, so no
-- series is resurrected and none is lost.
--
-- The join is a `LEFT JOIN` so that a `series_last_seen` row whose benchmark has
-- no empty parameter set trips the `NOT NULL` on `parameter_id` and fails the
-- migration. Every benchmark is born with one, and the parameter migration
-- backfilled every benchmark that predates that invariant, so this cannot fire on
-- valid data; an `INNER JOIN` would drop such a row silently instead.
INSERT INTO up_series_last_seen (
        organization_id,
        project_id,
        testbed_id,
        benchmark_id,
        parameter_id,
        measure_id,
        last_seen
    )
SELECT s.organization_id,
    s.project_id,
    s.testbed_id,
    s.benchmark_id,
    p.id,
    s.measure_id,
    s.last_seen
FROM series_last_seen s
    LEFT JOIN parameter p ON (
        p.benchmark_id = s.benchmark_id
        AND p.parameters = jsonb('{}')
    );
DROP TABLE series_last_seen;
ALTER TABLE up_series_last_seen
    RENAME TO series_last_seen;
CREATE INDEX index_series_last_seen_org_last_seen
    ON series_last_seen (organization_id, last_seen);
-- Index the remaining cascaded foreign keys so parent deletes do not full-scan this
-- table. `testbed_id` is already the primary key's prefix and `organization_id` is
-- covered by the billing index above.
CREATE INDEX index_series_last_seen_project ON series_last_seen (project_id);
CREATE INDEX index_series_last_seen_benchmark ON series_last_seen (benchmark_id);
CREATE INDEX index_series_last_seen_parameter ON series_last_seen (parameter_id);
CREATE INDEX index_series_last_seen_measure ON series_last_seen (measure_id);
PRAGMA foreign_keys = on;
-- Statistics
-- This stack rebuilds the largest tables in the database, and a rebuilt table has no
-- statistics. Without them the query planner falls back on its join order heuristics,
-- and on a branch filtered perf query those heuristics run the metric pivot before the
-- selective filters, so the query does far more work than the rows it returns call for.
-- `ANALYZE` hands the planner real row counts instead of guesses.
--
-- The analysis limit caps how many rows of each index are sampled, so the cost of this
-- statement is bounded rather than proportional to the size of the tables, while the
-- samples are still ample to order the joins correctly.
PRAGMA analysis_limit = 1000;
ANALYZE;
