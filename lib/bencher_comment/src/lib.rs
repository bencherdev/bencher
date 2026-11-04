#![expect(clippy::format_push_string, reason = "todo")]

use std::{collections::HashSet, fmt, ops::BitOr, time::Duration};

use bencher_json::{
    AlertUuid, BenchmarkSlug, BranchSlug, JsonAlert, JsonBenchmark, JsonMeasure, JsonPerfQuery,
    JsonReport, MeasureSlug, MetricName, ModelTest, ParameterSet, ProjectSlug, ReportUuid,
    ResourceName, TestbedSlug, ThresholdUuid, Units,
    project::{
        alert::AlertStatus,
        boundary::BoundaryLimit,
        report::{
            JsonReportBoundary, JsonReportIteration, JsonReportMeasure, JsonReportMetric,
            JsonReportResult, JsonReportWarning, ReportWarningAction, ReportWarningResource,
        },
    },
};
use ordered_float::OrderedFloat;
use url::Url;

mod check;
mod column;
mod group;
mod tag;

use column::{ColumnGroup, DefaultMetrics};
use group::escape;

pub use check::SectionCheck;
pub use group::{GroupComment, Section};
pub use tag::{CommentTag, group_tag};

// 4 weeks
const DEFAULT_REPORT_HISTORY: Duration = Duration::from_hours(672);

const EMPTY_CELL: &str = "<td></td>";
const TRUNCATED_NOTICE: &str = "<blockquote><h3>⚠️ WARNING: Truncated view!</h3><p>The full continuous benchmarking report exceeds the maximum length allowed on this platform.</p></blockquote>";

/// The branded report title markup.
/// Shared with the `bencher` CLI GitHub Check summaries,
/// so the in-progress and completed states cannot drift apart.
pub const BENCHER_REPORT_TITLE: &str = r#"<img src="https://bencher.dev/favicon.svg" width="24" height="24" alt="🐰" /> Bencher Report"#;

pub struct ReportComment {
    console_url: Url,
    project_slug: ProjectSlug,
    public_links: bool,
    multiple_iterations: bool,
    benchmark_count: usize,
    missing_threshold: HashSet<Measure>,
    default_metrics: DefaultMetrics,
    json_report: JsonReport,
    sub_adapter: SubAdapter,
    source: String,
    pending: bool,
}

pub struct SubAdapter {
    pub build_time: bool,
    pub file_size: bool,
}

impl ReportComment {
    pub fn new(
        console_url: Url,
        json_report: JsonReport,
        sub_adapter: SubAdapter,
        source: String,
    ) -> Self {
        let results = json_report.results.as_deref().unwrap_or_default();
        Self {
            console_url,
            project_slug: json_report.project.slug.clone(),
            public_links: json_report.project.visibility.is_public(),
            multiple_iterations: results.len() > 1,
            benchmark_count: results.iter().map(Vec::len).sum(),
            missing_threshold: Measure::missing_threshold(&json_report),
            default_metrics: DefaultMetrics::new(results),
            json_report,
            sub_adapter,
            source,
            pending: false,
        }
    }

    /// Pending is the caller's to say, since a job that finished without results leaves the same report.
    #[must_use]
    pub fn with_pending(mut self, pending: bool) -> Self {
        self.pending = pending;
        self
    }

    fn results(&self) -> &[JsonReportIteration] {
        self.json_report.results.as_deref().unwrap_or_default()
    }

    fn alerts(&self) -> &[JsonAlert] {
        self.json_report.alerts.as_deref().unwrap_or_default()
    }

    fn warnings(&self) -> &[JsonReportWarning] {
        self.json_report.warnings.as_deref().unwrap_or_default()
    }

    pub fn human(&self) -> String {
        let mut text = String::new();
        self.human_report_link(&mut text);
        self.human_no_benchmarks(&mut text);
        self.human_results_list(&mut text);
        self.human_warnings(&mut text);
        self.human_alerts_list(&mut text);
        self.human_unclaimed(&mut text);
        text
    }

    fn human_report_link(&self, text: &mut String) {
        let url = self.resource_url_human(Resource::Report(self.json_report.uuid));
        text.push_str(&format!("View report: {url}"));
    }

    fn human_no_benchmarks(&self, text: &mut String) {
        if self.benchmark_count == 0 {
            if self.pending {
                text.push_str("\n\nResults pending until the remote job finishes.");
            } else {
                text.push_str("\n\nWARNING: No benchmarks found!");
            }
        }
    }

    fn human_results_list(&self, text: &mut String) {
        if self.benchmark_count == 0 {
            return;
        }
        text.push_str("\n\nView results:");
        for (i, iteration) in self.results().iter().enumerate() {
            if self.multiple_iterations {
                if i != 0 {
                    text.push('\n');
                }
                text.push_str(&format!("\nIteration {i}:"));
            }

            for result in iteration {
                for report_measure in &result.measures {
                    let measure = Measure::from(report_measure.measure.clone());
                    text.push_str(&format!(
                        "\n- {benchmark}{parameters} ({measure}): {console_url}",
                        benchmark = result.benchmark.name,
                        parameters = human_parameters(&result.variant.parameters),
                        measure = metrics_label(&measure.name, self.default_metrics.get(&measure)),
                        console_url = self.perf_url(&result.benchmark, &report_measure.measure)
                    ));
                }
            }
        }
    }

    fn human_warnings(&self, text: &mut String) {
        if self.warnings().is_empty() {
            return;
        }

        text.push_str("\n\nWarnings:");
        for warning in self.warnings() {
            text.push_str(&format!(
                "\n- {resources} {action}",
                resources = counted(warning.count, resource_noun(warning.resource)),
                action = action_verb(warning.action),
            ));
        }
    }

    fn human_alerts_list(&self, text: &mut String) {
        if self.alerts().is_empty() {
            return;
        }

        text.push_str("\n\nView alerts:");
        for alert in self.alerts() {
            text.push_str(&format!(
                "\n- {benchmark_name}{parameters} ({measure_name}){iter}: {console_url}",
                benchmark_name = alert.benchmark.name,
                parameters = human_parameters(&alert.variant.parameters),
                measure_name = metrics_label(&alert.threshold.measure.name, &[alert_metric(alert)]),
                iter = if self.multiple_iterations {
                    format!(" (Iteration {iteration})", iteration = alert.iteration)
                } else {
                    String::new()
                },
                console_url = self.alert_perf_url(alert)
            ));
        }
    }

    fn human_unclaimed(&self, text: &mut String) {
        if self.json_report.project.claimed.is_some() {
            return;
        }

        let mut url = self.console_url.clone();
        url.set_path("/auth/signup");
        url.query_pairs_mut()
            .append_pair("claim", &self.json_report.project.organization.to_string());

        text.push_str(&format!("\n\nClaim this project: {url}"));
    }

    pub fn json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.json_report)
    }

    pub fn html(&self, require_threshold: bool, id: Option<&str>) -> String {
        self.html_inner(require_threshold, &self.bencher_tag(id), false)
    }

    pub fn html_with_max_length(
        &self,
        require_threshold: bool,
        id: Option<&str>,
        max_length: usize,
    ) -> String {
        self.html_with_tag(require_threshold, &self.bencher_tag(id), max_length)
    }

    fn html_with_tag(&self, require_threshold: bool, tag: &str, max_length: usize) -> String {
        let html = self.html_inner(require_threshold, tag, false);
        if html.len() > max_length {
            self.html_inner(require_threshold, tag, true)
        } else {
            html
        }
    }

    fn html_inner(&self, require_threshold: bool, tag: &str, truncated: bool) -> String {
        let mut html = String::new();
        let html_mut = &mut html;
        self.html_header(html_mut);
        self.html_report_table(html_mut);
        self.html_benchmarks(html_mut, require_threshold, truncated);
        self.html_footer(html_mut);
        // DO NOT MOVE: The Bencher tag must be the last thing in the HTML for updates to work
        html_mut.push_str(tag);
        html
    }

    fn html_header(&self, html: &mut String) {
        html.push_str(&format!(
            r#"<h2><a href="{url}">{BENCHER_REPORT_TITLE}</a></h2>"#,
            url = self.report_url(),
        ));
    }

    fn html_report_table(&self, html: &mut String) {
        html.push_str("<table>");
        for (row, name, url) in [
            (
                "Project",
                self.json_report.project.name.to_string(),
                self.resource_url(Resource::Project),
            ),
            (
                "Branch",
                self.json_report.branch.name.to_string(),
                self.resource_url(Resource::Branch(self.json_report.branch.slug.clone())),
            ),
            (
                "Testbed",
                self.json_report.testbed.name.to_string(),
                self.resource_url(Resource::Testbed(self.json_report.testbed.slug.clone())),
            ),
        ] {
            html_link_row(html, row, &name, &url);
        }
        html.push_str("</table>");
    }

    fn report_url(&self) -> Url {
        self.resource_url(Resource::Report(self.json_report.uuid))
    }

    fn html_benchmarks(&self, html: &mut String, require_threshold: bool, truncated: bool) {
        self.html_no_benchmarks(html, truncated);
        self.html_no_threshold(html, require_threshold, truncated);
        self.html_alerts(html, truncated);
        self.html_benchmark_details(html, require_threshold, truncated);
        if truncated {
            html_warnings_total(html, self.warnings_total());
        } else {
            self.html_warnings(html);
        }
    }

    fn html_no_benchmarks(&self, html: &mut String, truncated: bool) {
        if self.benchmark_count == 0 {
            if self.pending {
                html.push_str(
                    "<blockquote><h3>Results pending until the remote job finishes.</h3></blockquote>",
                );
            } else {
                html.push_str("<blockquote><h3>⚠️ WARNING: No benchmarks found!</h3></blockquote>");
            }
        } else if truncated {
            html.push_str(TRUNCATED_NOTICE);
        }
    }

    fn html_no_threshold(&self, html: &mut String, require_threshold: bool, truncated: bool) {
        if self.benchmark_count == 0 || self.missing_threshold.is_empty() || require_threshold {
            return;
        }

        html.push_str("<blockquote>");
        html.push_str("<h3>⚠️ WARNING: No Threshold found!</h3>");
        html.push_str("<p>Without a Threshold, no Alerts will ever be generated.</p>");

        if !truncated {
            html.push_str("<ul>");
            for Measure { name, slug, units } in &self.missing_threshold {
                let url = self.resource_url(Resource::Measure(slug.clone()));
                html.push_str(&format!("<li><a href=\"{url}\">{name} ({units})</a></li>"));
            }
            html.push_str("</ul>");

            html.push_str(&format!(
                "<p><a href=\"{url}\">Click here to create a new Threshold</a><br />",
                url = self.resource_url(Resource::Report(self.json_report.uuid)),
            ));
            html.push_str(&format!("For more information, see <a href=\"https://bencher.dev/docs/explanation/thresholds/{utm}\">the Threshold documentation</a>.<br />", utm = self.utm_query()));
            html.push_str(&format!("To only post results if a Threshold exists, set <a href=\"https://bencher.dev/docs/explanation/bencher-run/{utm}#--ci-only-thresholds\">the <code lang=\"rust\">--ci-only-thresholds</code> flag</a>.</p>", utm = self.utm_query()));
        }

        html.push_str("</blockquote>");
    }

    fn html_alerts(&self, html: &mut String, truncated: bool) {
        if self.alerts().is_empty() {
            return;
        }
        html_alerts_heading(html, self.alerts().len());

        if !truncated {
            self.html_alerts_table(html);
        }
    }

    fn html_alerts_table(&self, html: &mut String) {
        let columns = self.alert_columns();
        html.push_str("<table>");
        html.push_str("<thead>");
        html.push_str("<tr>");
        columns.html_header_cells(html);
        html.push_str("</tr>");
        html.push_str("</thead>");
        html.push_str("<tbody>");
        for alert in self.alerts() {
            html.push_str("<tr>");
            self.html_alert_cells(html, alert, columns);
            html.push_str("</tr>");
        }
        html.push_str("</tbody>");
        html.push_str("</table>");
    }

    fn alert_columns(&self) -> AlertColumns {
        AlertColumns {
            iteration: self.multiple_iterations,
            lower: self.has_boundary_alert(BoundaryLimit::Lower),
            upper: self.has_boundary_alert(BoundaryLimit::Upper),
        }
    }

    fn html_alert_cells(&self, html: &mut String, alert: &JsonAlert, columns: AlertColumns) {
        let (factor, units, units_symbol) = {
            let mut min = alert.value;
            if let Some(lower_limit) = alert.boundary.lower_limit {
                min = min.min(lower_limit);
            }
            if let Some(upper_limit) = alert.boundary.upper_limit {
                min = min.min(upper_limit);
            }
            let units = Units::new(min.into(), alert.threshold.measure.units.clone());
            (
                units.scale_factor(),
                units.scale_units(),
                units.scale_units_symbol(),
            )
        };

        if columns.iteration {
            html.push_str(&format!("<td>{}</td>", alert.iteration));
        }
        html.push_str(&format!(
            "<td><a href=\"{url}\">{benchmark}</a>{parameters}</td>",
            url = self.resource_url(Resource::Benchmark(alert.benchmark.slug.clone())),
            benchmark = alert.benchmark.name,
            parameters = html_parameters(&alert.variant.parameters),
        ));
        html.push_str(&format!(
            "<td><a href=\"{url}\">{measure}<br />{units}</a></td>",
            url = self.resource_url(Resource::Measure(alert.threshold.measure.slug.clone())),
            measure = metrics_label(&alert.threshold.measure.name, &[alert_metric(alert)]),
        ));
        self.html_alerts_table_view_cell(html, alert);
        value_cell(
            html,
            alert.value,
            alert.boundary.baseline,
            factor,
            &units_symbol,
            true,
        );
        if columns.lower {
            lower_limit_cell(
                html,
                alert.value,
                alert.boundary.lower_limit,
                factor,
                &units_symbol,
                alert.limit == BoundaryLimit::Lower,
            );
        }
        if columns.upper {
            upper_limit_cell(
                html,
                alert.value,
                alert.boundary.upper_limit,
                factor,
                &units_symbol,
                alert.limit == BoundaryLimit::Upper,
            );
        }
    }

    fn html_alerts_table_view_cell(&self, html: &mut String, alert: &JsonAlert) {
        html.push_str("<td>");
        html.push_str(&format!(
            "📈 <a href=\"{url}\">plot</a>",
            url = self.alert_perf_url(alert)
        ));
        html.push_str("<br />");
        html.push_str(&format!(
            "🚷 <a href=\"{url}\">threshold</a>",
            url = self.resource_url(Resource::Threshold(alert.threshold.uuid)),
        ));
        html.push_str("<br />");
        html.push_str(&format!(
            "🚨 <a href=\"{url}\">alert ({status})</a>",
            url = self.resource_url(Resource::Alert(alert.uuid)),
            status = alert_status(alert),
        ));
        html.push_str("</td>");
    }

    fn html_benchmark_details(&self, html: &mut String, require_threshold: bool, truncated: bool) {
        if self.benchmark_count == 0 || truncated {
            return;
        }

        html.push_str("<details><summary>Click to view all benchmark results</summary>");
        html.push_str("<br />");
        self.html_iteration_tables(html, require_threshold);
        html.push_str("</details>");
    }

    fn html_iteration_tables(&self, html: &mut String, require_threshold: bool) {
        for iteration in self.results() {
            self.html_iteration_table(html, iteration, require_threshold);
        }
    }

    fn has_boundary_alert(&self, boundary_limit: BoundaryLimit) -> bool {
        self.alerts()
            .iter()
            .any(|alert| alert.limit == boundary_limit)
    }

    fn html_iteration_table(
        &self,
        html: &mut String,
        iteration: &JsonReportIteration,
        require_threshold: bool,
    ) {
        let groups = self
            .default_metrics
            .column_groups(iteration, require_threshold);

        html.push_str("<table>");
        self.html_iteration_table_header(html, &groups);
        self.html_iteration_table_body(html, iteration, &groups);
        html.push_str("</table>");
    }

    fn html_iteration_table_header(&self, html: &mut String, groups: &[ColumnGroup<'_>]) {
        html.push_str("<thead>");
        html.push_str("<tr>");
        html.push_str("<th>Benchmark</th>");
        for ColumnGroup {
            measure,
            metric,
            limits,
        } in groups
        {
            let units = Units::new(limits.min.into(), measure.units.clone()).scale_units();

            html.push_str(&format!(
                "<th><a href=\"{url}\">{label}</a></th>",
                url = self.resource_url(Resource::Measure(measure.slug.clone())),
                label = metrics_label(&measure.name, std::slice::from_ref(*metric)),
            ));

            html.push_str("<th>");
            if limits.has_limit() {
                html.push_str("Benchmark Result<br />");
            }
            html.push_str(units.as_ref());
            if limits.has_limit() {
                html.push_str("<br />(Result Δ%)");
            }
            html.push_str("</th>");

            if limits.lower {
                html.push_str(&format!(
                    "<th>Lower Boundary<br />{units}<br />(Limit %)</th>"
                ));
            }

            if limits.upper {
                html.push_str(&format!(
                    "<th>Upper Boundary<br />{units}<br />(Limit %)</th>"
                ));
            }
        }
        html.push_str("</tr>");
        html.push_str("</thead>");
    }

    /// A row per variant, and a sub-row per threshold when two or more checked one metric.
    fn html_iteration_table_body(
        &self,
        html: &mut String,
        iteration: &JsonReportIteration,
        groups: &[ColumnGroup<'_>],
    ) {
        html.push_str("<tbody>");
        for result in iteration {
            let cells = groups
                .iter()
                .map(|group| column::report_metric(result, group.measure, group.metric))
                .collect::<Vec<_>>();
            let sub_rows = cells
                .iter()
                .flatten()
                .map(|(_, report_metric)| report_metric.boundaries.len())
                .max()
                .unwrap_or_default()
                .max(1);
            for sub_row in 0..sub_rows {
                html.push_str("<tr>");
                if sub_row == 0 {
                    self.html_iteration_table_benchmark_cell(html, result, sub_rows);
                }
                for (group, cell) in groups.iter().zip(cells.iter().copied()) {
                    if let Some(cell) = cell.filter(|(_, report_metric)| {
                        sub_row < report_metric.boundaries.len().max(1)
                    }) {
                        self.html_iteration_table_cells(
                            html,
                            result,
                            group,
                            cell,
                            sub_row,
                            sub_rows > 1,
                        );
                    } else {
                        group.limits.html_empty_cells(html);
                    }
                }
                html.push_str("</tr>");
            }
        }
        html.push_str("</tbody>");
    }

    fn html_iteration_table_benchmark_cell(
        &self,
        html: &mut String,
        result: &JsonReportResult,
        sub_rows: usize,
    ) {
        if sub_rows > 1 {
            html.push_str(&format!("<td rowspan=\"{sub_rows}\">"));
        } else {
            html.push_str("<td>");
        }
        html.push_str(&format!(
            "<a href=\"{url}\">{name}</a>{parameters}</td>",
            url = self.resource_url(Resource::Benchmark(result.benchmark.slug.clone())),
            name = result.benchmark.name,
            parameters = html_parameters(&result.variant.parameters),
        ));
    }

    /// One column group's cells for the threshold of a sub-row, or for none.
    fn html_iteration_table_cells(
        &self,
        html: &mut String,
        result: &JsonReportResult,
        group: &ColumnGroup<'_>,
        (report_measure, report_metric): (&JsonReportMeasure, &JsonReportMetric),
        sub_row: usize,
        named: bool,
    ) {
        let check = report_metric.boundaries.get(sub_row);
        let (factor, units_symbol) = {
            let units = Units::new(group.limits.min.into(), group.measure.units.clone());
            (units.scale_factor(), units.scale_units_symbol())
        };
        let alert = check.and_then(|check| self.find_alert(result, check.threshold.uuid));
        let boundary = check.map(|check| check.boundary);

        self.html_iteration_table_view_cell(html, result, report_measure, check, alert, named);
        value_cell(
            html,
            report_metric.value,
            boundary.and_then(|b| b.baseline),
            factor,
            &units_symbol,
            alert.is_some(),
        );
        if group.limits.lower {
            lower_limit_cell(
                html,
                report_metric.value,
                boundary.and_then(|b| b.lower_limit),
                factor,
                &units_symbol,
                alert.is_some_and(|a| a.limit == BoundaryLimit::Lower),
            );
        }
        if group.limits.upper {
            upper_limit_cell(
                html,
                report_metric.value,
                boundary.and_then(|b| b.upper_limit),
                factor,
                &units_symbol,
                alert.is_some_and(|a| a.limit == BoundaryLimit::Upper),
            );
        }
    }

    fn html_iteration_table_view_cell(
        &self,
        html: &mut String,
        result: &JsonReportResult,
        report_measure: &JsonReportMeasure,
        check: Option<&JsonReportBoundary>,
        alert: Option<&JsonAlert>,
        named: bool,
    ) {
        html.push_str("<td>");
        html.push_str(&format!(
            "📈 <a href=\"{url}\">view plot</a>",
            url = self.perf_url(&result.benchmark, &report_measure.measure)
        ));
        if let Some(check) = check {
            html.push_str("<br />");
            html.push_str(&format!(
                "🚷 <a href=\"{url}\">view threshold{test}</a>",
                url = self.resource_url(Resource::Threshold(check.threshold.uuid)),
                test = if named {
                    format!(" ({})", model_test_name(check.threshold.model.test))
                } else {
                    String::new()
                },
            ));
        } else {
            html.push_str("<br />");
            html.push_str("⚠️ NO THRESHOLD");
        }
        if let Some(alert) = alert {
            html.push_str("<br />");
            html.push_str(&format!(
                "🚨 <a href=\"{url}\">view alert ({status})</a>",
                url = self.resource_url(Resource::Alert(alert.uuid)),
                status = alert_status(alert),
            ));
        }
        html.push_str("</td>");
    }

    fn html_warnings(&self, html: &mut String) {
        if self.warnings().is_empty() {
            return;
        }

        html.push_str(&format!(
            "<details><summary>⚠️ Warnings ({total})</summary><br />",
            total = self.warnings_total(),
        ));
        html.push_str(
            "<table><thead><tr><th>Resource</th><th>Action</th><th>Count</th></tr></thead><tbody>",
        );
        for warning in self.warnings() {
            html.push_str(&format!(
                "<tr><td>{resource}</td><td>{action}</td><td>{count}</td></tr>",
                resource = capitalized(resource_noun(warning.resource)),
                action = capitalized(action_verb(warning.action)),
                count = warning.count,
            ));
        }
        html.push_str("</tbody></table></details>");
    }

    /// The sum of the warnings' counts.
    fn warnings_total(&self) -> u64 {
        self.warnings()
            .iter()
            .map(|warning| u64::from(warning.count))
            .sum()
    }

    fn html_footer(&self, html: &mut String) {
        html.push_str(&format!(
            r#"<a href="{url}">🐰 View full continuous benchmarking report in Bencher</a>"#,
            url = self.report_url(),
        ));
    }

    // The Bencher tag allows us to easily check whether a comment is a Bencher report when updating
    pub fn bencher_tag(&self, id: Option<&str>) -> String {
        tag::job_tag(&self.json_report.project.slug, &self.tag_id(id))
    }

    /// The ID in the Bencher tag: `id`, else the Branch, Testbed, and Adapter of the Report.
    pub fn tag_id(&self, id: Option<&str>) -> String {
        id.map_or_else(
            || {
                format!(
                    "{branch}/{testbed}/{adapter}{build_time}{file_size}",
                    branch = self.json_report.branch.slug,
                    testbed = self.json_report.testbed.slug,
                    adapter = self.json_report.adapter,
                    build_time = if self.sub_adapter.build_time {
                        "-build_time"
                    } else {
                        ""
                    },
                    file_size = if self.sub_adapter.file_size {
                        "-file_size"
                    } else {
                        ""
                    },
                )
            },
            str::to_owned,
        )
    }

    /// The name of the Project that the Report belongs to.
    /// Used by the `bencher` CLI to name the GitHub Check,
    /// so Reports for different Projects on the same commit do not collide.
    pub fn project_name(&self) -> &ResourceName {
        &self.json_report.project.name
    }

    pub fn project_slug(&self) -> &ProjectSlug {
        &self.json_report.project.slug
    }

    pub fn has_threshold(&self) -> bool {
        for iteration in self.results() {
            for result in iteration {
                for report_measure in &result.measures {
                    if is_checked(report_measure) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn has_alert(&self) -> bool {
        !self.alerts().is_empty()
    }

    /// The threshold's alert on this iteration and variant. A threshold checks one metric of
    /// one measure, so its UUID names the metric.
    fn find_alert(
        &self,
        result: &JsonReportResult,
        threshold: ThresholdUuid,
    ) -> Option<&JsonAlert> {
        self.alerts().iter().find(|alert| {
            alert.iteration == result.iteration
                && alert.variant.uuid == result.variant.uuid
                && alert.threshold.uuid == threshold
        })
    }

    #[cfg_attr(
        not(feature = "plus"),
        expect(clippy::unused_self, reason = "self used only with plus feature")
    )]
    fn is_bencher_cloud(&self) -> bool {
        #[cfg(feature = "plus")]
        {
            bencher_json::is_bencher_cloud(&self.console_url)
        }
        #[cfg(not(feature = "plus"))]
        false
    }

    fn resource_url_human(&self, resource: Resource) -> Url {
        self.resource_url_inner(resource, false)
    }

    fn resource_url(&self, resource: Resource) -> Url {
        self.resource_url_inner(resource, true)
    }

    fn resource_url_inner(&self, resource: Resource, utm: bool) -> Url {
        let url = self.console_url.clone();
        let mut path = format!("/console/projects/{project}", project = self.project_slug);
        if let Some((resource_name, id)) = resource.into_suffix() {
            path.push_str(&format!("/{resource_name}/{id}"));
        }
        let mut url = url.join(&path).unwrap_or(url);

        if utm && self.is_bencher_cloud() {
            url.query_pairs_mut()
                .append_pair("utm_medium", "referral")
                .append_pair("utm_source", &self.source)
                .append_pair("utm_content", "comment")
                .append_pair("utm_campaign", "pr+comments")
                .append_pair("utm_term", self.project_slug.as_ref());
        }

        url
    }

    fn utm_query(&self) -> String {
        if self.is_bencher_cloud() {
            format!(
                "?utm_medium=referral&utm_source={source}&utm_content=comment&utm_campaign=pr+comments&utm_term={project}",
                source = self.source,
                project = self.project_slug,
            )
        } else {
            String::new()
        }
    }

    /// The plot of the line that alerted.
    fn alert_perf_url(&self, alert: &JsonAlert) -> Url {
        let metric = alert_metric(alert);
        let parameters = &alert.variant.parameters;
        let parameters = (!parameters.is_empty()).then(|| vec![parameters.clone()]);
        self.plot_url(
            &alert.benchmark,
            &alert.threshold.measure,
            parameters,
            &[metric],
        )
    }

    /// The plot of every variant of the benchmark.
    fn perf_url(&self, benchmark: &JsonBenchmark, measure: &JsonMeasure) -> Url {
        self.plot_url(benchmark, measure, None, &[])
    }

    /// The public plot on a public project, or else Explore.
    fn plot_url(
        &self,
        benchmark: &JsonBenchmark,
        measure: &JsonMeasure,
        parameters: Option<Vec<ParameterSet>>,
        metrics: &[MetricName],
    ) -> Url {
        let json_perf_query = JsonPerfQuery {
            branches: vec![self.json_report.branch.uuid],
            heads: vec![Some(self.json_report.branch.head.uuid)],
            testbeds: vec![self.json_report.testbed.uuid],
            #[cfg(feature = "plus")]
            specs: self
                .json_report
                .testbed
                .spec
                .as_ref()
                .map(|spec| vec![Some(spec.uuid)])
                .unwrap_or_default(),
            benchmarks: vec![benchmark.uuid],
            parameters,
            measures: vec![measure.uuid],
            start_time: Some(
                (self.json_report.start_time.into_inner() - DEFAULT_REPORT_HISTORY).into(),
            ),
            end_time: Some(self.json_report.end_time),
        };
        json_perf_query.to_console_url(
            &self.console_url,
            &self.project_slug,
            self.public_links,
            metrics,
            Some(self.json_report.uuid),
        )
    }
}

enum Resource {
    Project,
    Report(ReportUuid),
    Branch(BranchSlug),
    Testbed(TestbedSlug),
    Benchmark(BenchmarkSlug),
    Measure(MeasureSlug),
    Threshold(ThresholdUuid),
    Alert(AlertUuid),
}

impl Resource {
    fn into_suffix(self) -> Option<(&'static str, String)> {
        match self {
            Self::Project => None,
            Self::Report(uuid) => Some(("reports", uuid.to_string())),
            Self::Branch(slug) => Some(("branches", slug.to_string())),
            Self::Testbed(slug) => Some(("testbeds", slug.to_string())),
            Self::Benchmark(slug) => Some(("benchmarks", slug.to_string())),
            Self::Measure(slug) => Some(("measures", slug.to_string())),
            Self::Threshold(uuid) => Some(("thresholds", uuid.to_string())),
            Self::Alert(uuid) => Some(("alerts", uuid.to_string())),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Measure {
    name: ResourceName,
    slug: MeasureSlug,
    units: ResourceName,
}

impl From<JsonMeasure> for Measure {
    fn from(json_measure: JsonMeasure) -> Self {
        let JsonMeasure {
            name, slug, units, ..
        } = json_measure;
        Self { name, slug, units }
    }
}

impl Measure {
    fn missing_threshold(json_report: &JsonReport) -> HashSet<Self> {
        json_report
            .results
            .as_deref()
            .unwrap_or_default()
            .iter()
            .flat_map(|iteration| {
                iteration.iter().flat_map(|result| {
                    result
                        .measures
                        .iter()
                        .filter(|&report_measure| !is_checked(report_measure))
                        .map(|report_measure| Self::from(report_measure.measure.clone()))
                })
            })
            .collect()
    }
}

/// Checked when a threshold wrote a boundary for one of its metrics, with or without a limit.
fn is_checked(report_measure: &JsonReportMeasure) -> bool {
    report_measure
        .metrics
        .iter()
        .any(|metric| !metric.boundaries.is_empty())
}

/// The measure's name, then its metrics when they are not just `value`.
fn metrics_label(measure: &ResourceName, metrics: &[MetricName]) -> String {
    if metrics.iter().all(|metric| *metric == MetricName::value()) {
        return measure.to_string();
    }
    let metrics = metrics
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<&str>>()
        .join(", ");
    format!("{measure}: {metrics}")
}

/// A threshold that names no metric checks `value`.
fn alert_metric(alert: &JsonAlert) -> MetricName {
    alert
        .threshold
        .metric
        .clone()
        .unwrap_or_else(MetricName::value)
}

/// The variant's `key=value` pairs, as the console's parameter chips print them.
fn human_parameters(parameters: &ParameterSet) -> String {
    parameters.tags().fold(String::new(), |mut text, tag| {
        text.push(' ');
        text.push_str(&tag);
        text
    })
}

fn html_parameters(parameters: &ParameterSet) -> String {
    if parameters.is_empty() {
        return String::new();
    }
    let tags = parameters
        .tags()
        .map(|tag| format!("<code>{}</code>", escape(&tag)))
        .collect::<Vec<_>>()
        .join(" ");
    format!("<br />{tags}")
}

/// A truncated comment's one line in place of its warnings section.
fn html_warnings_total(html: &mut String, total: u64) {
    if total > 0 {
        html.push_str(&format!("<p>⚠️ Warnings ({total})</p>"));
    }
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The test as `--threshold-test` spells it.
fn model_test_name(test: ModelTest) -> &'static str {
    match test {
        ModelTest::Static => "static",
        ModelTest::Percentage => "percentage",
        ModelTest::ZScore => "z_score",
        ModelTest::TTest => "t_test",
        ModelTest::LogNormal => "log_normal",
        ModelTest::Iqr => "iqr",
        ModelTest::DeltaIqr => "delta_iqr",
    }
}

fn resource_noun(resource: ReportWarningResource) -> &'static str {
    match resource {
        ReportWarningResource::Benchmark => "benchmark",
        ReportWarningResource::Variant => "variant",
        ReportWarningResource::Measure => "measure",
        ReportWarningResource::Metric => "metric",
        ReportWarningResource::Threshold => "threshold",
        ReportWarningResource::Adapter => "adapter",
        ReportWarningResource::Average => "average",
    }
}

fn action_verb(action: ReportWarningAction) -> &'static str {
    match action {
        ReportWarningAction::Skip => "skipped",
        ReportWarningAction::Ignore => "ignored",
    }
}

fn alert_status(alert: &JsonAlert) -> &str {
    match alert.status {
        AlertStatus::Active => "🔔",
        AlertStatus::Dismissed | AlertStatus::Silenced => "🔕",
    }
}

fn html_link_row(html: &mut String, row: &str, name: &str, url: &Url) {
    html.push_str(&format!(
        r#"<tr><td>{row}</td><td><a href="{url}">{name}</a></td></tr>"#,
    ));
}

fn html_alerts_heading(html: &mut String, alerts_len: usize) {
    html.push_str(&format!("<h3>🚨 {}</h3>", counted(alerts_len, "Alert")));
}

fn counted<N: Copy + fmt::Display + PartialEq + From<u8>>(count: N, noun: &str) -> String {
    if count == N::from(1) {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// The alerts table columns that only some tables have.
#[derive(Clone, Copy)]
struct AlertColumns {
    iteration: bool,
    lower: bool,
    upper: bool,
}

impl AlertColumns {
    fn html_header_cells(self, html: &mut String) {
        if self.iteration {
            html.push_str("<th>Iteration</th>");
        }
        html.push_str("<th>Benchmark</th>");
        html.push_str("<th>Measure<br />Units</th>");
        html.push_str("<th>View</th>");
        html.push_str("<th>Benchmark Result<br />(Result Δ%)</th>");
        if self.lower {
            html.push_str("<th>Lower Boundary<br />(Limit %)</th>");
        }
        if self.upper {
            html.push_str("<th>Upper Boundary<br />(Limit %)</th>");
        }
    }
}

impl BitOr for AlertColumns {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self {
            iteration: self.iteration || rhs.iteration,
            lower: self.lower || rhs.lower,
            upper: self.upper || rhs.upper,
        }
    }
}

fn value_cell(
    html: &mut String,
    value: OrderedFloat<f64>,
    baseline: Option<OrderedFloat<f64>>,
    factor: OrderedFloat<f64>,
    units_symbol: &str,
    bold: bool,
) {
    fn value_cell_inner(
        value: OrderedFloat<f64>,
        baseline: Option<OrderedFloat<f64>>,
        factor: OrderedFloat<f64>,
        units_symbol: &str,
    ) -> String {
        let mut cell = Units::format_float((value / factor).into());
        if !units_symbol.is_empty() {
            cell.push_str(&format!(" {units_symbol}"));
        }

        if let Some(baseline) = baseline {
            let percent = percent_difference(value, baseline);
            let plus = if percent > 0.0.into() { "+" } else { "" };
            let percent = Units::format_float(percent.into());
            let baseline = Units::format_float((baseline / factor).into());
            cell.push_str("<br />");
            cell.push_str("<details>");
            cell.push_str("<summary>");
            cell.push_str(&format!("({plus}{percent}%)"));
            cell.push_str("</summary>");
            cell.push_str(&format!("Baseline: {baseline}"));
            if !units_symbol.is_empty() {
                cell.push_str(&format!(" {units_symbol}"));
            }
            cell.push_str("</details>");
        }

        cell
    }

    html.push_str("<td>");
    if bold {
        html.push_str(&format!(
            "<b>{}</b>",
            value_cell_inner(value, baseline, factor, units_symbol)
        ));
    } else {
        html.push_str(&value_cell_inner(value, baseline, factor, units_symbol));
    }
    html.push_str("</td>");
}

/// The percent difference between a value and its baseline.
///
/// Only the baseline has to be non-zero.
/// A value of zero is a `-100.00%` difference from a positive baseline,
/// not a `0.00%` difference.
/// There is no percent difference from a zero baseline (`±∞`)
/// nor from a non-finite value, so fall back to zero.
fn percent_difference(value: OrderedFloat<f64>, baseline: OrderedFloat<f64>) -> OrderedFloat<f64> {
    if baseline.is_normal() && value.is_finite() {
        ((value - baseline) / baseline) * 100.0
    } else {
        0.0.into()
    }
}

fn lower_limit_cell(
    html: &mut String,
    value: OrderedFloat<f64>,
    lower_limit: Option<OrderedFloat<f64>>,
    factor: OrderedFloat<f64>,
    units_symbol: &str,
    bold: bool,
) {
    let Some(limit) = lower_limit else {
        html.push_str(EMPTY_CELL);
        return;
    };

    let percent = percent_of(limit, value);

    limit_cell(html, limit, percent, factor, units_symbol, bold);
}

fn upper_limit_cell(
    html: &mut String,
    value: OrderedFloat<f64>,
    upper_limit: Option<OrderedFloat<f64>>,
    factor: OrderedFloat<f64>,
    units_symbol: &str,
    bold: bool,
) {
    let Some(limit) = upper_limit else {
        html.push_str(EMPTY_CELL);
        return;
    };

    let percent = percent_of(value, limit);

    limit_cell(html, limit, percent, factor, units_symbol, bold);
}

/// The numerator as a percent of the denominator.
///
/// Only the denominator has to be non-zero.
/// A numerator of zero is `0.00%` of a positive denominator.
/// There is no percent of a zero denominator (`±∞`)
/// nor of a non-finite numerator, so fall back to zero.
fn percent_of(numerator: OrderedFloat<f64>, denominator: OrderedFloat<f64>) -> OrderedFloat<f64> {
    if denominator.is_normal() && numerator.is_finite() {
        (numerator / denominator) * 100.0
    } else {
        0.0.into()
    }
}

fn limit_cell(
    html: &mut String,
    limit: OrderedFloat<f64>,
    percent: OrderedFloat<f64>,
    factor: OrderedFloat<f64>,
    units_symbol: &str,
    bold: bool,
) {
    fn limit_cell_inner(
        limit: OrderedFloat<f64>,
        percent: OrderedFloat<f64>,
        factor: OrderedFloat<f64>,
        units_symbol: &str,
    ) -> String {
        let mut cell = Units::format_float((limit / factor).into());
        if !units_symbol.is_empty() {
            cell.push_str(&format!(" {units_symbol}"));
        }
        let percent = Units::format_float(percent.into());
        cell.push_str(&format!("<br />({percent}%)"));
        cell
    }

    html.push_str("<td>");
    if bold {
        // The two extra line breaks are here to make the text line up
        // with the value cell on GitHub,
        // where the row cells are vertically aligned to each other.
        html.push_str(&format!(
            "<b>{}<br /><br /></b>",
            limit_cell_inner(limit, percent, factor, units_symbol)
        ));
    } else {
        html.push_str(&limit_cell_inner(limit, percent, factor, units_symbol));
    }
    html.push_str("</td>");
}

#[derive(Clone, Copy)]
pub struct BoundaryLimits {
    min: OrderedFloat<f64>,
    lower: bool,
    upper: bool,
}

impl BitOr for BoundaryLimits {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self {
            min: self.min.min(rhs.min),
            lower: self.lower || rhs.lower,
            upper: self.upper || rhs.upper,
        }
    }
}

impl BoundaryLimits {
    /// The smallest of the metric's value and every limit its thresholds set,
    /// and whether any of them set a lower or an upper limit.
    fn of(report_metric: &JsonReportMetric) -> Self {
        let limits = report_metric
            .boundaries
            .iter()
            .flat_map(|check| [check.boundary.lower_limit, check.boundary.upper_limit]);
        Self {
            min: limits.flatten().fold(report_metric.value, Ord::min),
            lower: report_metric
                .boundaries
                .iter()
                .any(|check| check.boundary.lower_limit.is_some()),
            upper: report_metric
                .boundaries
                .iter()
                .any(|check| check.boundary.upper_limit.is_some()),
        }
    }

    fn has_limit(self) -> bool {
        self.lower || self.upper
    }

    /// The group's cells for a result or a sub-row with nothing to draw in it.
    fn html_empty_cells(self, html: &mut String) {
        html.push_str(EMPTY_CELL);
        html.push_str(EMPTY_CELL);
        if self.lower {
            html.push_str(EMPTY_CELL);
        }
        if self.upper {
            html.push_str(EMPTY_CELL);
        }
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{
        DateTime, JsonBranch, JsonHead, JsonProject, JsonReport, JsonReportCounts, JsonTestbed,
        project::{
            Visibility,
            boundary::BoundaryLimit,
            report::{
                Adapter, JsonReportResults, JsonReportWarning, ReportWarningAction,
                ReportWarningResource,
            },
        },
    };
    use ordered_float::OrderedFloat;
    use url::Url;

    use crate::{
        ReportComment, SubAdapter, TRUNCATED_NOTICE, percent_difference, percent_of, value_cell,
    };

    const CONSOLE_URL: &str = "https://bencher.example.com";

    pub(crate) fn json_report(visibility: Visibility) -> JsonReport {
        let project_uuid = "11111111-1111-1111-1111-111111111111".parse().unwrap();
        JsonReport {
            uuid: "00000000-0000-0000-0000-000000000000".parse().unwrap(),
            user: None,
            project_key: None,
            project: JsonProject {
                uuid: project_uuid,
                organization: "22222222-2222-2222-2222-222222222222".parse().unwrap(),
                name: "My Project".parse().unwrap(),
                slug: "my-project".parse().unwrap(),
                url: None,
                visibility,
                bmf_version: bencher_json::BmfVersion::default(),
                created: DateTime::TEST,
                modified: DateTime::TEST,
                claimed: Some(DateTime::TEST),
            },
            branch: JsonBranch {
                uuid: "33333333-3333-3333-3333-333333333333".parse().unwrap(),
                project: project_uuid,
                name: "main".parse().unwrap(),
                slug: "main".parse().unwrap(),
                head: JsonHead {
                    uuid: "44444444-4444-4444-4444-444444444444".parse().unwrap(),
                    start_point: None,
                    version: None,
                    created: DateTime::TEST,
                    replaced: None,
                },
                created: DateTime::TEST,
                modified: DateTime::TEST,
                archived: None,
            },
            testbed: JsonTestbed {
                uuid: "55555555-5555-5555-5555-555555555555".parse().unwrap(),
                project: project_uuid,
                name: "localhost".parse().unwrap(),
                slug: "localhost".parse().unwrap(),
                #[cfg(feature = "plus")]
                spec: None,
                created: DateTime::TEST,
                modified: DateTime::TEST,
                archived: None,
            },
            start_time: DateTime::TEST,
            end_time: DateTime::TEST,
            bmf_version: None,
            adapter: Adapter::Magic,
            parse_outcome: None,
            results: Some(Vec::new()),
            alerts: Some(Vec::new()),
            counts: JsonReportCounts::default(),
            warnings: None,
            #[cfg(feature = "plus")]
            job: None,
            created: DateTime::TEST,
        }
    }

    pub(crate) fn report_comment(visibility: Visibility) -> ReportComment {
        report_comment_for(json_report(visibility))
    }

    pub(crate) fn report_comment_for(report: JsonReport) -> ReportComment {
        ReportComment::new(
            CONSOLE_URL.parse().unwrap(),
            report,
            SubAdapter {
                build_time: false,
                file_size: false,
            },
            "cli".to_owned(),
        )
    }

    pub(crate) fn job_report() -> JsonReport {
        JsonReport {
            #[cfg(feature = "plus")]
            job: Some("12121212-1212-1212-1212-121212121212".parse().unwrap()),
            ..json_report(Visibility::Public)
        }
    }

    /// One iteration with two measures of one benchmark: one that named a point
    /// estimate and one that named only a percentile.
    ///
    /// Built from JSON rather than constructors because the absent deprecated
    /// `metric` is the wire shape under test.
    pub(crate) fn value_less_results() -> JsonReportResults {
        let date = serde_json::to_value(DateTime::TEST).unwrap();
        let measure = |uuid: &str, name: &str, slug: &str| {
            serde_json::json!({
                "uuid": uuid,
                "project": "11111111-1111-1111-1111-111111111111",
                "name": name,
                "slug": slug,
                "units": "nanoseconds (ns)",
                "created": date,
                "modified": date,
                "archived": null,
            })
        };
        serde_json::from_value(serde_json::json!([[{
            "iteration": 0,
            "benchmark": {
                "uuid": "66666666-6666-6666-6666-666666666666",
                "project": "11111111-1111-1111-1111-111111111111",
                "name": "bench",
                "slug": "bench",
                "created": date,
                "modified": date,
                "archived": null,
            },
            "variant": {
                "uuid": "77777777-7777-7777-7777-777777777777",
                "parameters": {},
            },
            "measures": [
                {
                    "measure": measure("88888888-8888-8888-8888-888888888888", "Latency", "latency"),
                    "metrics": [{
                        "uuid": "99999999-9999-9999-9999-999999999999",
                        "name": "value",
                        "value": 1.0,
                        "boundaries": [],
                    }],
                    "metric": {
                        "uuid": "99999999-9999-9999-9999-999999999999",
                        "value": 1.0,
                        "lower_value": null,
                        "upper_value": null,
                    },
                    "threshold": null,
                    "boundary": null,
                },
                {
                    "measure": measure("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "Throughput", "throughput"),
                    "metrics": [{
                        "uuid": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
                        "name": "p99",
                        "value": 2.0,
                        "boundaries": [],
                    }],
                    "threshold": null,
                    "boundary": null,
                },
            ],
        }]]))
        .unwrap()
    }

    const NAMED_ALERT_PROJECT: &str = "11111111-1111-1111-1111-111111111111";
    const NAMED_ALERT_BENCHMARK: &str = "66666666-6666-6666-6666-666666666666";
    const NAMED_ALERT_VARIANT: &str = "77777777-7777-7777-7777-777777777777";
    const NAMED_ALERT_BARE: &str = "cccccccc-cccc-cccc-cccc-cccccccccccc";
    const NAMED_ALERT_UUID: &str = "ffffffff-ffff-ffff-ffff-ffffffffffff";

    /// A report whose `latency` measure is checked by a bare threshold that did not
    /// breach, beside a `p99` threshold that did.
    ///
    /// Built from JSON because the wire shape is what is under test: the singular
    /// fields carry the bare threshold, and the alert carries the `p99` one.
    pub(crate) fn named_alert_report() -> JsonReport {
        let mut report = json_report(Visibility::Public);
        report.results = Some(serde_json::from_value(named_alert_results()).unwrap());
        // The only alert is the `p99` threshold's, on a row this table never draws.
        report.alerts = Some(
            serde_json::from_value(named_alert_alerts(&report.branch, &report.testbed)).unwrap(),
        );
        report
    }

    fn named_alert_date() -> serde_json::Value {
        serde_json::to_value(DateTime::TEST).unwrap()
    }

    fn named_alert_benchmark() -> serde_json::Value {
        serde_json::json!({
            "uuid": NAMED_ALERT_BENCHMARK,
            "project": NAMED_ALERT_PROJECT,
            "name": "bench",
            "slug": "bench",
            "created": named_alert_date(),
            "modified": named_alert_date(),
            "archived": null,
        })
    }

    fn named_alert_measure() -> serde_json::Value {
        serde_json::json!({
            "uuid": "88888888-8888-8888-8888-888888888888",
            "project": NAMED_ALERT_PROJECT,
            "name": "Latency",
            "slug": "latency",
            "units": "nanoseconds (ns)",
            "created": named_alert_date(),
            "modified": named_alert_date(),
            "archived": null,
        })
    }

    /// The bare threshold, as the deprecated singular `threshold` field carries it.
    fn named_alert_bare_threshold() -> serde_json::Value {
        serde_json::json!({
            "uuid": NAMED_ALERT_BARE,
            "project": NAMED_ALERT_PROJECT,
            "model": {
                "uuid": "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee",
                "test": "t_test",
                "min_sample_size": null,
                "max_sample_size": null,
                "window": null,
                "lower_boundary": null,
                "upper_boundary": 0.98,
                "created": named_alert_date(),
                "replaced": null,
            },
            "created": named_alert_date(),
        })
    }

    /// One benchmark whose `latency` `value` row the bare threshold checked and
    /// accepted: 1.0 sits well inside its upper limit of 100.0.
    pub(crate) fn named_alert_results() -> serde_json::Value {
        let boundary = serde_json::json!({
            "baseline": 1.0,
            "lower_limit": null,
            "upper_limit": 100.0,
        });
        serde_json::json!([[{
            "iteration": 0,
            "benchmark": named_alert_benchmark(),
            "variant": {
                "uuid": NAMED_ALERT_VARIANT,
                "parameters": {},
            },
            "measures": [{
                "measure": named_alert_measure(),
                "metrics": [{
                    "uuid": "99999999-9999-9999-9999-999999999999",
                    "name": "value",
                    "value": 1.0,
                    "boundaries": [{
                        "threshold": named_alert_bare_threshold(),
                        "boundary": boundary,
                    }],
                }],
                "metric": {
                    "uuid": "99999999-9999-9999-9999-999999999999",
                    "value": 1.0,
                    "lower_value": null,
                    "upper_value": null,
                },
                "threshold": named_alert_bare_threshold(),
                "boundary": boundary,
            }],
        }]])
    }

    /// One alert, from a `p99` threshold that is not the one the singular fields
    /// describe, on the same benchmark and measure.
    fn named_alert_alerts(branch: &JsonBranch, testbed: &JsonTestbed) -> serde_json::Value {
        serde_json::json!([{
            "uuid": NAMED_ALERT_UUID,
            "report": "00000000-0000-0000-0000-000000000000",
            "iteration": 0,
            "benchmark": named_alert_benchmark(),
            "variant": {
                "uuid": NAMED_ALERT_VARIANT,
                "benchmark": NAMED_ALERT_BENCHMARK,
                "parameters": {},
                "created": named_alert_date(),
                "modified": named_alert_date(),
                "archived": null,
            },
            "value": 500.0,
            "threshold": {
                "uuid": "dddddddd-dddd-dddd-dddd-dddddddddddd",
                "project": NAMED_ALERT_PROJECT,
                "branch": serde_json::to_value(branch).unwrap(),
                "testbed": serde_json::to_value(testbed).unwrap(),
                "measure": named_alert_measure(),
                "metric": "p99",
                "model": null,
                "created": named_alert_date(),
                "modified": named_alert_date(),
            },
            "boundary": {
                "baseline": 10.0,
                "lower_limit": null,
                "upper_limit": 20.0,
            },
            "limit": "upper",
            "status": "active",
            "created": named_alert_date(),
            "modified": named_alert_date(),
        }])
    }

    /// A report on its own Branch and Testbed, so links from two of them differ.
    pub(crate) fn report_on(uuid: &str, branch: (&str, &str), testbed: (&str, &str)) -> JsonReport {
        let mut report = json_report(Visibility::Public);
        report.uuid = uuid.parse().unwrap();
        let (branch_uuid, branch_slug) = branch;
        report.branch.uuid = branch_uuid.parse().unwrap();
        report.branch.name = branch_slug.parse().unwrap();
        report.branch.slug = branch_slug.parse().unwrap();
        let (testbed_uuid, testbed_slug) = testbed;
        report.testbed.uuid = testbed_uuid.parse().unwrap();
        report.testbed.name = testbed_slug.parse().unwrap();
        report.testbed.slug = testbed_slug.parse().unwrap();
        report
    }

    /// The report with one alert past `limit` in each of its `iterations` iterations.
    pub(crate) fn with_alerts(
        mut report: JsonReport,
        limit: BoundaryLimit,
        iterations: u32,
    ) -> JsonReport {
        let result = named_alert_results()[0][0].clone();
        let mut alert = named_alert_alerts(&report.branch, &report.testbed)[0].clone();
        alert["limit"] = serde_json::to_value(limit).unwrap();
        if limit == BoundaryLimit::Lower {
            alert["boundary"] = serde_json::json!({
                "baseline": 1000.0,
                "lower_limit": 600.0,
                "upper_limit": null,
            });
        }
        let (results, alerts): (Vec<_>, Vec<_>) = (0..iterations)
            .map(|iteration| {
                let mut result = result.clone();
                result["iteration"] = iteration.into();
                let mut alert = alert.clone();
                alert["iteration"] = iteration.into();
                (serde_json::json!([result]), alert)
            })
            .unzip();
        report.results = Some(serde_json::from_value(results.into()).unwrap());
        report.alerts = Some(serde_json::from_value(alerts.into()).unwrap());
        report
    }

    // A measure counts as checked when a threshold wrote a boundary for one of its metrics,
    // whether or not the deprecated singular threshold describes it, so a project whose
    // thresholds are all narrow is not told it has none.
    #[test]
    fn is_checked_reads_the_metrics_boundaries() {
        // The singular threshold is present.
        let report = named_alert_report();
        let comment = report_comment_for(report);
        assert!(comment.has_threshold());
        assert!(comment.missing_threshold.is_empty());

        // No singular threshold, but the metric carries a boundary.
        let mut report = json_report(Visibility::Public);
        let mut results = named_alert_results();
        results[0][0]["measures"][0]["threshold"] = serde_json::Value::Null;
        results[0][0]["measures"][0]["boundary"] = serde_json::Value::Null;
        report.results = Some(serde_json::from_value(results).unwrap());
        let comment = report_comment_for(report);
        assert!(
            comment.has_threshold(),
            "a narrow threshold's boundary is still a check"
        );
        assert!(comment.missing_threshold.is_empty());

        // Neither: no singular threshold and no boundary anywhere.
        let mut report = json_report(Visibility::Public);
        let mut results = named_alert_results();
        results[0][0]["measures"][0]["threshold"] = serde_json::Value::Null;
        results[0][0]["measures"][0]["boundary"] = serde_json::Value::Null;
        results[0][0]["measures"][0]["metrics"][0]["boundaries"] = serde_json::json!([]);
        report.results = Some(serde_json::from_value(results).unwrap());
        let comment = report_comment_for(report);
        assert!(!comment.has_threshold());
        assert_eq!(
            comment.missing_threshold.len(),
            1,
            "the unchecked measure is named"
        );
    }

    // An alert from a threshold the singular fields do not describe never lands on
    // the `value` cell. The bare threshold did not breach, so the row draws no alert
    // although the report carries one for the same benchmark and measure.
    #[test]
    fn find_alert_ignores_another_threshold_s_alert() {
        let html = report_comment_for(named_alert_report()).html(false, None);
        // The alerts table above carries the `p99` alert; only the benchmark table
        // below is under test.
        let (_, benchmark_table) = html
            .split_once("Click to view all benchmark results")
            .expect("the comment carries the benchmark table");

        assert!(
            benchmark_table.contains("<td>1.00 ns<br />"),
            "the value cell is drawn plain: {benchmark_table}"
        );
        assert!(
            !benchmark_table.contains("<b>1.00 ns"),
            "the value the bare threshold accepted is not styled as alerting: {benchmark_table}"
        );
        assert!(
            !benchmark_table.contains("<b>100.00 ns"),
            "and its upper limit is not flagged as crossed: {benchmark_table}"
        );
        assert!(
            !benchmark_table.contains("/alerts/"),
            "the other threshold's alert does not badge the row: {benchmark_table}"
        );
    }

    // A measure that named no point estimate reaches the comment with the deprecated
    // `metric` absent. Its column group is its default metric, so its value is drawn,
    // and rendering neither panics nor invents a zero.
    #[test]
    fn report_table_value_less_measure() {
        let mut report = json_report(Visibility::Public);
        report.results = Some(value_less_results());

        let html = report_comment_for(report).html(false, None);
        assert!(
            html.contains(">Latency</a></th>"),
            "unexpected table: {html}"
        );
        assert!(
            html.contains("<td>1.00 ns</td>"),
            "unexpected table: {html}"
        );
        assert!(
            html.contains(">Throughput: p99</a></th>"),
            "the measure that named only `p99` shows it: {html}"
        );
        assert!(
            html.contains("<td>2.00 ns</td>"),
            "unexpected table: {html}"
        );
        // It is still a measure of this report, so the threshold warning names it.
        assert!(
            html.contains("/measures/throughput\">Throughput"),
            "unexpected report: {html}"
        );
    }

    pub(crate) fn warning(
        resource: ReportWarningResource,
        action: ReportWarningAction,
        count: u32,
    ) -> JsonReportWarning {
        JsonReportWarning {
            resource,
            action,
            count,
        }
    }

    fn with_warnings(mut report: JsonReport) -> JsonReport {
        report.warnings = Some(vec![
            warning(
                ReportWarningResource::Benchmark,
                ReportWarningAction::Skip,
                3,
            ),
            warning(
                ReportWarningResource::Measure,
                ReportWarningAction::Ignore,
                1,
            ),
            warning(
                ReportWarningResource::Threshold,
                ReportWarningAction::Skip,
                2,
            ),
        ]);
        report
    }

    const WARNINGS_SECTION: &str = "<details><summary>⚠️ Warnings (6)</summary><br /><table><thead><tr><th>Resource</th><th>Action</th><th>Count</th></tr></thead><tbody><tr><td>Benchmark</td><td>Skipped</td><td>3</td></tr><tr><td>Measure</td><td>Ignored</td><td>1</td></tr><tr><td>Threshold</td><td>Skipped</td><td>2</td></tr></tbody></table></details>";
    const WARNINGS_LINES: &str =
        "\n\nWarnings:\n- 3 benchmarks skipped\n- 1 measure ignored\n- 2 thresholds skipped";

    // Kills warnings missing from the run output or the comment, out of their place after the
    // results and the tables, a summary that counts the rows rather than summing their counts,
    // a plural on a count of 1 or none on others, swapped actions, and table cells without the
    // console's capitalized labels.
    #[test]
    fn warnings_follow_the_results_and_the_tables() {
        let comment = report_comment_for(with_warnings(named_alert_report()));

        let human = comment.human();
        let (results, warnings) = human.split_once(WARNINGS_LINES).expect(&human);
        assert!(
            results.contains("View results:\n- bench (Latency): "),
            "{human}"
        );
        assert!(warnings.starts_with("\n\nView alerts:\n"), "{human}");

        let html = comment.html(false, None);
        assert!(
            html.contains(&format!(
                "</table></details>{WARNINGS_SECTION}<a href=\"https://bencher.example.com/console/projects/my-project/reports/"
            )),
            "{html}"
        );
    }

    // Kills warnings dropped from a report with no benchmarks, where they say why, a truncated
    // comment that keeps the collapsible section or drops the total, and a section or a total
    // for a report whose warnings are empty.
    #[test]
    fn warnings_show_without_benchmarks_and_only_when_present() {
        let comment = report_comment_for(with_warnings(json_report(Visibility::Public)));
        let human = comment.human();
        assert!(
            human.contains(&format!("No benchmarks found!{WARNINGS_LINES}")),
            "{human}"
        );
        let html = comment.html(false, None);
        assert!(
            html.contains(&format!(
                "No benchmarks found!</h3></blockquote>{WARNINGS_SECTION}"
            )),
            "{html}"
        );

        let truncated = report_comment_for(with_warnings(named_alert_report()))
            .html_with_max_length(false, None, 0);
        assert!(truncated.contains(TRUNCATED_NOTICE), "{truncated}");
        assert!(
            truncated.contains("1 Alert</h3><p>⚠️ Warnings (6)</p><a href="),
            "{truncated}"
        );
        assert_eq!(truncated.matches("Warnings").count(), 1, "{truncated}");

        let mut report = named_alert_report();
        report.warnings = Some(Vec::new());
        let comment = report_comment_for(report);
        assert!(!comment.human().contains("Warnings"));
        assert!(!comment.html(false, None).contains("Warnings"));
        assert!(
            !comment
                .html_with_max_length(false, None, 0)
                .contains("Warnings")
        );
    }

    #[test]
    fn pending_report_replaces_no_benchmarks_warning() {
        let comment = report_comment_for(job_report()).with_pending(true);

        let human = comment.human();
        assert!(
            human.contains("\n\nResults pending until the remote job finishes."),
            "{human}"
        );
        assert!(!human.contains("No benchmarks found"), "{human}");

        let html = comment.html(false, None);
        assert!(
            html.contains(
                "<blockquote><h3>Results pending until the remote job finishes.</h3></blockquote>"
            ),
            "{html}"
        );
        assert!(!html.contains("No benchmarks found"), "{html}");
    }

    #[test]
    fn finished_job_without_results_keeps_no_benchmarks_warning() {
        let comment = report_comment_for(job_report()).with_pending(false);

        let human = comment.human();
        assert!(
            human.contains("\n\nWARNING: No benchmarks found!"),
            "{human}"
        );
        assert!(!human.contains("Results pending"), "{human}");

        let html = comment.html(false, None);
        assert!(
            html.contains("<blockquote><h3>⚠️ WARNING: No benchmarks found!</h3></blockquote>"),
            "{html}"
        );
        assert!(!html.contains("Results pending"), "{html}");
    }

    // A public project's report table opens the console, and the branch row drops its head.
    #[test]
    fn report_table_public_project() {
        let html = report_comment(Visibility::Public).html(false, None);
        let project = r#"<tr><td>Project</td><td><a href="https://bencher.example.com/console/projects/my-project">My Project</a></td></tr>"#;
        let branch = r#"<tr><td>Branch</td><td><a href="https://bencher.example.com/console/projects/my-project/branches/main">main</a></td></tr>"#;
        let testbed = r#"<tr><td>Testbed</td><td><a href="https://bencher.example.com/console/projects/my-project/testbeds/localhost">localhost</a></td></tr>"#;
        let project_pos = html.find(project).expect("missing project row");
        let branch_pos = html.find(branch).expect("missing branch row");
        let testbed_pos = html.find(testbed).expect("missing testbed row");
        assert!(project_pos < branch_pos);
        assert!(branch_pos < testbed_pos);
    }

    // A private project's report table opens the console, and the testbed row drops its spec.
    #[cfg(feature = "plus")]
    #[test]
    fn report_table_private_project() {
        let mut report = json_report(Visibility::Private);
        report.testbed.spec = Some(json_spec());
        let html = report_comment_for(report).html(false, None);
        let project = r#"<tr><td>Project</td><td><a href="https://bencher.example.com/console/projects/my-project">My Project</a></td></tr>"#;
        let branch = r#"<tr><td>Branch</td><td><a href="https://bencher.example.com/console/projects/my-project/branches/main">main</a></td></tr>"#;
        let testbed = r#"<tr><td>Testbed</td><td><a href="https://bencher.example.com/console/projects/my-project/testbeds/localhost">localhost</a></td></tr>"#;
        let project_pos = html.find(project).expect("missing project row");
        let branch_pos = html.find(branch).expect("missing branch row");
        let testbed_pos = html.find(testbed).expect("missing testbed row");
        assert!(project_pos < branch_pos);
        assert!(branch_pos < testbed_pos);
    }

    #[cfg(feature = "plus")]
    fn json_spec() -> bencher_json::JsonSpec {
        let date = serde_json::to_value(DateTime::TEST).unwrap();
        serde_json::from_value(serde_json::json!({
            "uuid": "abababab-abab-abab-abab-abababababab",
            "name": "Spec",
            "slug": "spec",
            "os": "linux",
            "architecture": "x86_64",
            "sandbox": null,
            "cpu": 2,
            "memory": 0x0001_0000_0000i64,
            "disk": 0x0002_8000_0000i64,
            "network": false,
            "fallback": null,
            "created": date,
            "modified": date,
            "archived": null,
        }))
        .unwrap()
    }

    /// Every link in the HTML, in order.
    fn hrefs(html: &str) -> Vec<Url> {
        html.split("href=\"")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .map(|href| href.parse().unwrap())
            .collect()
    }

    fn query_keys(url: &Url) -> Vec<String> {
        url.query_pairs().map(|(key, _)| key.into_owned()).collect()
    }

    /// The keys every plot link carries, without the line an alert's link adds.
    fn plot_query_keys(url: &Url) -> Vec<String> {
        query_keys(url)
            .into_iter()
            .filter(|key| key != "metrics" && key != "parameters")
            .collect()
    }

    fn query_values(url: &Url, key: &str) -> Vec<String> {
        url.query_pairs()
            .filter(|(name, _)| name == key)
            .map(|(_, value)| value.into_owned())
            .collect()
    }

    /// The alert's plot links: the alerts table's, then the run output's.
    fn alert_plot_links(report: JsonReport) -> Vec<Url> {
        let comment = report_comment_for(report);
        let html = comment.html(false, None);
        let (alerts_table, _) = html
            .split_once("Click to view all benchmark results")
            .expect("the comment carries the benchmark table");
        let mut links = hrefs(alerts_table)
            .into_iter()
            .filter(|url| url.path() == "/perf/my-project")
            .collect::<Vec<_>>();
        let human = comment.human();
        let (_, alerts) = human.split_once("View alerts:").expect(&human);
        links.extend(
            alerts
                .lines()
                .filter_map(|line| line.split_once("): "))
                .map(|(_, link)| link.parse::<Url>().unwrap()),
        );
        assert_eq!(links.len(), 2, "{links:#?}");
        links
    }

    fn report_with_alert(edit: impl FnOnce(&mut serde_json::Value)) -> JsonReport {
        let mut report = named_alert_report();
        let mut alerts = named_alert_alerts(&report.branch, &report.testbed);
        edit(&mut alerts[0]);
        report.alerts = Some(serde_json::from_value(alerts).unwrap());
        report
    }

    // An alert's plot links open on the line that alerted: its threshold's metric and
    // its variant's parameter set, as Explore reads them. The benchmark table's link
    // covers every line of its row, so it carries neither, even on the row that alerted.
    #[test]
    fn alert_plot_links_open_the_alerted_line() {
        let report = report_with_alert(|alert| {
            alert["variant"]["parameters"] = serde_json::json!({"size_mb": 16, "op": "read"});
            alert["threshold"]["uuid"] = serde_json::json!(NAMED_ALERT_BARE);
        });
        let html = report_comment_for(report.clone()).html(false, None);
        for link in alert_plot_links(report) {
            assert_eq!(query_values(&link, "metrics"), ["p99"], "{link}");
            assert_eq!(
                query_values(&link, "parameters"),
                [r#"{"op":"read","size_mb":16}"#],
                "{link}"
            );
        }

        let (_, benchmark_table) = html
            .split_once("Click to view all benchmark results")
            .expect("the comment carries the benchmark table");
        let plots = hrefs(benchmark_table)
            .into_iter()
            .filter(|url| url.path() == "/perf/my-project")
            .collect::<Vec<_>>();
        assert_eq!(plots.len(), 1, "{plots:#?}");
        for plot in plots {
            assert_eq!(query_keys(&plot), PLOT_QUERY_KEYS, "{plot}");
        }
        assert!(
            benchmark_table.contains("/alerts/"),
            "the row draws its alert: {benchmark_table}"
        );
    }

    // A variant with no parameters adds no set, and a threshold that names no metric
    // checks `value`.
    #[test]
    fn alert_plot_links_leave_out_what_the_line_lacks() {
        for link in alert_plot_links(named_alert_report()) {
            assert_eq!(query_values(&link, "metrics"), ["p99"], "{link}");
            assert!(query_values(&link, "parameters").is_empty(), "{link}");
        }

        let report = report_with_alert(|alert| {
            alert["threshold"]["metric"] = serde_json::Value::Null;
        });
        for link in alert_plot_links(report) {
            assert_eq!(query_values(&link, "metrics"), ["value"], "{link}");
            assert!(query_values(&link, "parameters").is_empty(), "{link}");
        }
    }

    fn visibilities() -> Vec<Visibility> {
        vec![
            Visibility::Public,
            #[cfg(feature = "plus")]
            Visibility::Private,
        ]
    }

    const PLOT_QUERY_KEYS: [&str; 8] = [
        "branches",
        "heads",
        "testbeds",
        "benchmarks",
        "measures",
        "start_time",
        "end_time",
        "report",
    ];

    // Report, benchmark, measure, threshold, and alert links open their console pages
    // at either visibility, and a threshold link drops its model.
    #[test]
    fn resource_links_open_the_console() {
        for visibility in visibilities() {
            let mut report = named_alert_report();
            report.project.visibility = visibility;
            let links = hrefs(&report_comment_for(report).html(false, None))
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>();
            for expected in [
                "https://bencher.example.com/console/projects/my-project/reports/00000000-0000-0000-0000-000000000000",
                "https://bencher.example.com/console/projects/my-project/benchmarks/bench",
                "https://bencher.example.com/console/projects/my-project/measures/latency",
                // The alert's threshold, from the alerts table.
                "https://bencher.example.com/console/projects/my-project/thresholds/dddddddd-dddd-dddd-dddd-dddddddddddd",
                // The bare threshold, which has a model, from the benchmark table.
                "https://bencher.example.com/console/projects/my-project/thresholds/cccccccc-cccc-cccc-cccc-cccccccccccc",
                "https://bencher.example.com/console/projects/my-project/alerts/ffffffff-ffff-ffff-ffff-ffffffffffff",
            ] {
                assert!(
                    links.iter().any(|link| link == expected),
                    "{visibility:?} missing {expected}: {links:#?}"
                );
            }
        }
    }

    // A public project's plot links, from the benchmark table and the alerts table,
    // open the public plot with the query and no boundary flags.
    #[test]
    fn plot_links_public_project() {
        let html = report_comment_for(named_alert_report()).html(false, None);
        let plots = hrefs(&html)
            .into_iter()
            .filter(|url| !url.path().starts_with("/console/"))
            .collect::<Vec<_>>();
        assert_eq!(plots.len(), 2, "{plots:#?}");
        for plot in plots {
            assert_eq!(plot.path(), "/perf/my-project", "{plot}");
            assert_eq!(plot_query_keys(&plot), PLOT_QUERY_KEYS, "{plot}");
        }
    }

    // A private project's plot links open Explore with the query and no boundary flags.
    #[cfg(feature = "plus")]
    #[test]
    fn plot_links_private_project() {
        let mut report = named_alert_report();
        report.project.visibility = Visibility::Private;
        let html = report_comment_for(report).html(false, None);
        let plots = hrefs(&html)
            .into_iter()
            .filter(|url| url.path().ends_with("/explore"))
            .collect::<Vec<_>>();
        assert_eq!(plots.len(), 2, "{plots:#?}");
        for plot in plots {
            assert_eq!(
                plot.path(),
                "/console/projects/my-project/explore",
                "{plot}"
            );
            assert_eq!(plot_query_keys(&plot), PLOT_QUERY_KEYS, "{plot}");
        }
    }

    // A testbed with a spec puts it in the plot's query, where Explore reads it.
    #[cfg(feature = "plus")]
    #[test]
    fn plot_link_carries_the_testbed_spec() {
        let mut report = named_alert_report();
        report.testbed.spec = Some(json_spec());
        let human = report_comment_for(report).human();
        assert!(
            human.contains("&specs=abababab-abab-abab-abab-abababababab&"),
            "{human}"
        );
    }

    // Create a new Threshold opens the report page, where each line no threshold checks offers its flags.
    #[test]
    fn create_threshold_opens_the_report() {
        for visibility in visibilities() {
            let mut report = json_report(visibility);
            report.results = Some(value_less_results());
            let html = report_comment_for(report).html(false, None);
            assert!(
                html.contains(r#"<a href="https://bencher.example.com/console/projects/my-project/reports/00000000-0000-0000-0000-000000000000">Click here to create a new Threshold</a>"#),
                "{visibility:?}: {html}"
            );
        }
    }

    // `bencher run`'s output: the report opens in the console, and the results and
    // alerts open the public plot.
    #[test]
    fn run_output_public_project() {
        let human = report_comment_for(named_alert_report()).human();
        let lines = human.lines().collect::<Vec<_>>();
        assert_eq!(
            lines.first().copied(),
            Some(
                "View report: https://bencher.example.com/console/projects/my-project/reports/00000000-0000-0000-0000-000000000000"
            ),
            "{human}"
        );
        assert_run_output_plots(&human, "/perf/my-project");
    }

    // `bencher run`'s output for a private project: the results and alerts open Explore.
    #[cfg(feature = "plus")]
    #[test]
    fn run_output_private_project() {
        let mut report = named_alert_report();
        report.project.visibility = Visibility::Private;
        let human = report_comment_for(report).human();
        assert!(
            human.starts_with("View report: https://bencher.example.com/console/projects/my-project/reports/00000000-0000-0000-0000-000000000000\n"),
            "{human}"
        );
        assert_run_output_plots(&human, "/console/projects/my-project/explore");
    }

    fn assert_run_output_plots(human: &str, path: &str) {
        let (_, results) = human.split_once("View results:").expect(human);
        let (results, alerts) = results.split_once("View alerts:").expect(human);
        for section in [results, alerts] {
            let link = section
                .lines()
                .filter(|line| line.starts_with("- bench (Latency"))
                .find_map(|line| line.split_once("): ").map(|(_, link)| link))
                .unwrap_or_else(|| panic!("no plot link: {human}"));
            let url: Url = link.parse().unwrap();
            assert_eq!(url.path(), path, "{human}");
            assert_eq!(plot_query_keys(&url), PLOT_QUERY_KEYS, "{human}");
        }
    }

    // Bencher Cloud links keep their `utm_*` parameters.
    #[cfg(feature = "plus")]
    #[test]
    fn cloud_links_keep_utm() {
        let mut report = json_report(Visibility::Public);
        report.results = Some(value_less_results());
        let html = ReportComment::new(
            "https://bencher.dev".parse().unwrap(),
            report,
            SubAdapter {
                build_time: false,
                file_size: false,
            },
            "cli".to_owned(),
        )
        .html(false, None);
        let utm = "?utm_medium=referral&utm_source=cli&utm_content=comment&utm_campaign=pr%2Bcomments&utm_term=my-project";
        assert!(
            html.contains(&format!(
                r#"<a href="https://bencher.dev/console/projects/my-project/reports/00000000-0000-0000-0000-000000000000{utm}">Click here to create a new Threshold</a>"#
            )),
            "{html}"
        );
        assert!(
            html.contains(&format!(
                r#"<a href="https://bencher.dev/console/projects/my-project/branches/main{utm}">main</a>"#
            )),
            "{html}"
        );
    }

    #[test]
    fn percent_difference_value() {
        // A value that drops all the way to zero is a 100% improvement,
        // not a 0% no-op.
        assert_eq!(
            percent_difference(OrderedFloat(0.0), OrderedFloat(3_069_448.0)),
            OrderedFloat(-100.0)
        );
        assert_eq!(
            percent_difference(OrderedFloat(50.0), OrderedFloat(100.0)),
            OrderedFloat(-50.0)
        );
        assert_eq!(
            percent_difference(OrderedFloat(150.0), OrderedFloat(100.0)),
            OrderedFloat(50.0)
        );
        assert_eq!(
            percent_difference(OrderedFloat(100.0), OrderedFloat(100.0)),
            OrderedFloat(0.0)
        );
    }

    #[test]
    fn percent_difference_zero_baseline() {
        // There is no percent difference from a zero baseline
        // nor from a non-finite value,
        // so fall back to zero instead of infinity or `NaN`.
        assert_eq!(
            percent_difference(OrderedFloat(0.0), OrderedFloat(0.0)),
            OrderedFloat(0.0)
        );
        assert_eq!(
            percent_difference(OrderedFloat(100.0), OrderedFloat(0.0)),
            OrderedFloat(0.0)
        );
        assert_eq!(
            percent_difference(OrderedFloat(100.0), OrderedFloat(f64::INFINITY)),
            OrderedFloat(0.0)
        );
        assert_eq!(
            percent_difference(OrderedFloat(f64::INFINITY), OrderedFloat(100.0)),
            OrderedFloat(0.0)
        );
    }

    #[test]
    fn percent_of_value() {
        // A zero numerator is zero percent of the denominator.
        assert_eq!(
            percent_of(OrderedFloat(0.0), OrderedFloat(100.0)),
            OrderedFloat(0.0)
        );
        assert_eq!(
            percent_of(OrderedFloat(50.0), OrderedFloat(100.0)),
            OrderedFloat(50.0)
        );
        assert_eq!(
            percent_of(OrderedFloat(100.0), OrderedFloat(100.0)),
            OrderedFloat(100.0)
        );
        // A zero denominator has no percent, so fall back to zero.
        assert_eq!(
            percent_of(OrderedFloat(100.0), OrderedFloat(0.0)),
            OrderedFloat(0.0)
        );
        assert_eq!(
            percent_of(OrderedFloat(0.0), OrderedFloat(0.0)),
            OrderedFloat(0.0)
        );
    }

    #[test]
    fn value_cell_zero_value() {
        let mut html = String::new();
        value_cell(
            &mut html,
            OrderedFloat(0.0),
            Some(OrderedFloat(3_069_448.0)),
            OrderedFloat(1.0),
            "B",
            false,
        );
        assert!(
            html.contains("<summary>(-100.00%)</summary>"),
            "unexpected value cell: {html}"
        );
        assert!(
            html.contains("Baseline: 3,069,448.00 B"),
            "unexpected value cell: {html}"
        );
    }
}
