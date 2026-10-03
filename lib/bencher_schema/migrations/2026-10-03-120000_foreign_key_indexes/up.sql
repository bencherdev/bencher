-- Deleting a parent row looks up its children by these columns, which seeks an
-- index rather than walking the table.
CREATE INDEX index_branch_head_id ON branch(head_id);
CREATE INDEX index_job_organization ON job(organization_id);
DROP INDEX IF EXISTS index_job_runner_id;
CREATE INDEX index_job_runner_id ON job(runner_id);
CREATE INDEX index_metric_measure ON metric(measure_id);
CREATE INDEX index_organization_role_organization ON organization_role(organization_id);
CREATE INDEX index_plot_benchmark_benchmark ON plot_benchmark(benchmark_id);
CREATE INDEX index_plot_branch_branch ON plot_branch(branch_id);
CREATE INDEX index_plot_measure_measure ON plot_measure(measure_id);
CREATE INDEX index_plot_testbed_testbed ON plot_testbed(testbed_id);
CREATE INDEX index_project_key_creator ON project_key(creator_id);
CREATE INDEX index_project_role_project ON project_role(project_id);
CREATE INDEX index_report_user ON report(user_id);
CREATE INDEX index_sso_organization ON sso(organization_id);
CREATE INDEX index_threshold_measure ON threshold(measure_id);
CREATE INDEX index_threshold_testbed ON threshold(testbed_id);
CREATE INDEX index_version_project ON version(project_id);
