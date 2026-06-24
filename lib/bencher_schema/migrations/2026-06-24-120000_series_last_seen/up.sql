-- Cache of each monitored series' most recent activity, for value-based billing on
-- monthly-active series (a series is a distinct testbed x benchmark x measure) and
-- for telemetry. `last_seen` is the greatest `report.created` (the server-side
-- ingestion time, not the user-supplied `end_time`) ever recorded for the series, kept
-- monotonic on ingest so reprocessing an older report cannot lower it.
--
-- `organization_id` and `project_id` are denormalized so a billing read is a single
-- index range scan over (organization_id, last_seen), without joining through the
-- entity tables. The series key (testbed_id, benchmark_id, measure_id) is globally
-- unique on its own because each entity is per-project and id-unique, so it is the
-- primary key.
--
-- Hard-deleting a testbed, benchmark, or measure cascades its series rows away and can
-- lower the current period's active-series count; an accepted exception to monotonic
-- billing, since entity deletion requires first deleting all of its reports.
CREATE TABLE series_last_seen (
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

CREATE INDEX index_series_last_seen_org_last_seen
    ON series_last_seen (organization_id, last_seen);

-- Index the remaining cascaded foreign keys so parent deletes do not full-scan this
-- table. `testbed_id` is already the primary key's prefix and `organization_id` is
-- covered by the billing index above.
CREATE INDEX index_series_last_seen_project ON series_last_seen (project_id);
CREATE INDEX index_series_last_seen_benchmark ON series_last_seen (benchmark_id);
CREATE INDEX index_series_last_seen_measure ON series_last_seen (measure_id);

-- Backfill from existing metrics: one row per distinct series with the latest
-- `report.created` it has produced. Runs once, against the table just created empty
-- above and before the server serves any request, so it needs no conflict handling
-- (like `metric_count_by_report`). Correct on day one, so the cache equals a fresh
-- COUNT(DISTINCT testbed, benchmark, measure) over the same metrics.
INSERT INTO series_last_seen (organization_id, project_id, testbed_id, benchmark_id, measure_id, last_seen)
SELECT p.organization_id, p.id, r.testbed_id, rb.benchmark_id, m.measure_id, MAX(r.created)
FROM metric m
INNER JOIN report_benchmark rb ON m.report_benchmark_id = rb.id
INNER JOIN report r ON rb.report_id = r.id
INNER JOIN benchmark b ON rb.benchmark_id = b.id
INNER JOIN project p ON b.project_id = p.id
GROUP BY r.testbed_id, rb.benchmark_id, m.measure_id;
