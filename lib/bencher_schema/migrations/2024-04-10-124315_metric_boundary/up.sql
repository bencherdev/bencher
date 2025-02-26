CREATE INDEX index_report_testbed_end_time ON report(testbed_id, end_time);
CREATE INDEX index_report_benchmark ON report_benchmark(report_id, benchmark_id);
CREATE INDEX index_alert_boundary ON alert(boundary_id);
-- metric_boundary
CREATE VIEW metric_boundary AS
SELECT metric.id AS metric_id,
    metric.uuid AS metric_uuid,
    metric.report_benchmark_id,
    metric.measure_id,
    metric.value,
    metric.lower_value,
    metric.upper_value,
    boundary.id AS boundary_id,
    boundary.uuid AS boundary_uuid,
    boundary.threshold_id,
    boundary.model_id,
    boundary.baseline,
    boundary.lower_limit,
    boundary.upper_limit
FROM metric
    LEFT OUTER JOIN boundary ON (boundary.metric_id = metric.id);