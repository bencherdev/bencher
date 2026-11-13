-- The highest count of active Benchmark Series a Self-Hosted server has seen in each paid term of
-- an organization's license. Every term keeps its row, so a renewal keeps the ended term's peak.
CREATE TABLE license_peak (
    organization_id INTEGER NOT NULL,
    term_end BIGINT NOT NULL,
    active_series INTEGER NOT NULL,
    seen BIGINT NOT NULL,
    PRIMARY KEY (organization_id, term_end),
    FOREIGN KEY (organization_id) REFERENCES organization (id) ON DELETE CASCADE
);
