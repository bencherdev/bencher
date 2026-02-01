-- Composite index for metrics_data query (version_id + testbed_id filtering)
-- Improves the 7-way JOIN in detector/data.rs when querying historical metrics
CREATE INDEX index_report_version_testbed ON report(version_id, testbed_id);
