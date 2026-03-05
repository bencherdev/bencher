CREATE TABLE metric_count_by_report (
    report_id INTEGER PRIMARY KEY NOT NULL,
    metric_count INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (report_id) REFERENCES report (id) ON DELETE CASCADE
);

INSERT INTO metric_count_by_report (report_id, metric_count)
SELECT rb.report_id, COUNT(m.id)
FROM report_benchmark rb
INNER JOIN metric m ON m.report_benchmark_id = rb.id
GROUP BY rb.report_id;
