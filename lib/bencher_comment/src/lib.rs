#![expect(clippy::format_push_string, reason = "todo")]

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    ops::{BitOr, BitOrAssign},
    time::Duration,
};

#[cfg(feature = "plus")]
use bencher_json::SpecUuid;
use bencher_json::{
    AlertUuid, BenchmarkSlug, BenchmarkUuid, BranchSlug, HeadUuid, JsonAlert, JsonBenchmark,
    JsonBoundary, JsonMeasure, JsonPerfQuery, JsonReport, MeasureSlug, MetricName, ModelUuid,
    ParameterSet, ProjectSlug, ReportUuid, ResourceName, TestbedSlug, ThresholdUuid, Units,
    project::{
        alert::AlertStatus,
        boundary::BoundaryLimit,
        plot::{LOWER_BOUNDARY, UPPER_BOUNDARY},
        report::{JsonReportIteration, JsonReportMeasure, JsonReportResult},
        threshold::JsonThresholdModel,
    },
};
use ordered_float::OrderedFloat;
use url::Url;

// 30 days
const DEFAULT_REPORT_HISTORY: Duration = Duration::from_hours(720);

const EMPTY_CELL: &str = "<td></td>";

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
    grid_benchmarks: HashSet<BenchmarkUuid>,
    json_report: JsonReport,
    sub_adapter: SubAdapter,
    source: String,
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
            grid_benchmarks: grid_benchmarks(&json_report),
            json_report,
            sub_adapter,
            source,
        }
    }

    /// The name a row gives its benchmark, and the grid point that row carries.
    ///
    /// A benchmark whose every row in this comment carries the empty parameter set
    /// keeps the bare benchmark name it has always had. One non-empty set among its
    /// rows names them all, the empty set among them included, which reads `{}`.
    /// The set is spelled in its canonical form and follows the benchmark name, the
    /// way the perf image spells it.
    fn benchmark_label(&self, benchmark: &JsonBenchmark, set: &ParameterSet) -> String {
        if self.grid_benchmarks.contains(&benchmark.uuid) {
            format!("{name} {set}", name = benchmark.name, set = set.canonical())
        } else {
            benchmark.name.to_string()
        }
    }

    /// The name a row gives a gated measure: the measure, and the metric the
    /// threshold gates when that is not the conventional `value`.
    ///
    /// A threshold that names no metric gates `value`, so it reads as the measure
    /// alone, which is every threshold an older client could create.
    fn measure_label(measure: &JsonMeasure, metric: Option<&MetricName>) -> String {
        match metric {
            Some(metric) if *metric != MetricName::value() => {
                format!("{name} ({metric})", name = measure.name)
            },
            _ => measure.name.to_string(),
        }
    }

    fn results(&self) -> &[JsonReportIteration] {
        self.json_report.results.as_deref().unwrap_or_default()
    }

    fn alerts(&self) -> &[JsonAlert] {
        self.json_report.alerts.as_deref().unwrap_or_default()
    }

    pub fn human(&self) -> String {
        let mut text = String::new();
        self.human_report_link(&mut text);
        self.human_no_benchmarks(&mut text);
        self.human_results_list(&mut text);
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
            text.push_str("\n\nWARNING: No benchmarks found!");
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
                    text.push_str(&format!(
                        "\n- {benchmark} ({measure}): {console_url}",
                        benchmark = self.benchmark_label(&result.benchmark, &result.parameter.set),
                        measure = report_measure.measure.name,
                        console_url = self.perf_url(
                            &result.benchmark,
                            &report_measure.measure,
                            report_measure.boundary.map(Into::into)
                        )
                    ));
                }
            }
        }
    }

    fn human_alerts_list(&self, text: &mut String) {
        if self.alerts().is_empty() {
            return;
        }

        text.push_str("\n\nView alerts:");
        for alert in self.alerts() {
            text.push_str(&format!(
                "\n- {benchmark_name} ({measure_name}){iter}: {console_url}",
                benchmark_name = self.benchmark_label(&alert.benchmark, &alert.parameter.set),
                measure_name =
                    Self::measure_label(&alert.threshold.measure, alert.threshold.metric.as_ref()),
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
        self.html_inner(require_threshold, id, false)
    }

    pub fn html_with_max_length(
        &self,
        require_threshold: bool,
        id: Option<&str>,
        max_length: usize,
    ) -> String {
        let html = self.html(require_threshold, id);
        if html.len() > max_length {
            self.html_inner(require_threshold, id, true)
        } else {
            html
        }
    }

    fn html_inner(&self, require_threshold: bool, id: Option<&str>, truncated: bool) -> String {
        let mut html = String::new();
        let html_mut = &mut html;
        self.html_header(html_mut);
        self.html_report_table(html_mut);
        self.html_benchmarks(html_mut, require_threshold, truncated);
        self.html_footer(html_mut);
        // DO NOT MOVE: The Bencher tag must be the last thing in the HTML for updates to work
        self.html_bencher_tag(html_mut, id);
        html
    }

    fn html_header(&self, html: &mut String) {
        html.push_str(&format!(
            r#"<h2><a href="{url}">{BENCHER_REPORT_TITLE}</a></h2>"#,
            url = self.resource_url(Resource::Report(self.json_report.uuid)),
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
                self.resource_url(Resource::Branch {
                    slug: self.json_report.branch.slug.clone(),
                    head: self.json_report.branch.head.uuid,
                }),
            ),
            (
                "Testbed",
                self.json_report.testbed.name.to_string(),
                self.resource_url(Resource::Testbed {
                    slug: self.json_report.testbed.slug.clone(),
                    #[cfg(feature = "plus")]
                    spec: self.json_report.testbed.spec.as_ref().map(|s| s.uuid),
                }),
            ),
        ] {
            html.push_str(&format!(
                r#"<tr><td>{row}</td><td><a href="{url}">{name}</a></td></tr>"#,
            ));
        }
        html.push_str("</table>");
    }

    fn html_benchmarks(&self, html: &mut String, require_threshold: bool, truncated: bool) {
        self.html_no_benchmarks(html, truncated);
        self.html_no_threshold(html, require_threshold, truncated);
        self.html_alerts(html, truncated);
        self.html_benchmark_details(html, require_threshold, truncated);
    }

    fn html_no_benchmarks(&self, html: &mut String, truncated: bool) {
        if self.benchmark_count == 0 {
            html.push_str("<blockquote><h3>⚠️ WARNING: No benchmarks found!</h3></blockquote>");
        } else if truncated {
            html.push_str("<blockquote><h3>⚠️ WARNING: Truncated view!</h3><p>The full continuous benchmarking report exceeds the maximum length allowed on this platform.</p></blockquote>");
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

            html.push_str(&format!("<p><a href=\"{console_url}console/projects/{project}/thresholds/add{utm}\">Click here to create a new Threshold</a><br />", console_url = self.console_url, project = self.project_slug, utm = self.utm_query()));
            html.push_str(&format!("For more information, see <a href=\"https://bencher.dev/docs/explanation/thresholds/{utm}\">the Threshold documentation</a>.<br />", utm = self.utm_query()));
            html.push_str(&format!("To only post results if a Threshold exists, set <a href=\"https://bencher.dev/docs/explanation/bencher-run/{utm}#--ci-only-thresholds\">the <code lang=\"rust\">--ci-only-thresholds</code> flag</a>.</p>", utm = self.utm_query()));
        }

        html.push_str("</blockquote>");
    }

    fn html_alerts(&self, html: &mut String, truncated: bool) {
        if self.alerts().is_empty() {
            return;
        }
        let alerts_len = self.alerts().len();
        html.push_str(&format!(
            "<h3>🚨 {alerts_len} {alert}</h3>",
            alert = if alerts_len == 1 { "Alert" } else { "Alerts" },
        ));

        if !truncated {
            self.html_alerts_table(html);
        }
    }

    fn html_alerts_table(&self, html: &mut String) {
        html.push_str("<table>");
        self.html_alerts_table_header(html);
        self.html_alerts_table_body(html);
        html.push_str("</table>");
    }

    fn html_alerts_table_header(&self, html: &mut String) {
        html.push_str("<thead>");
        html.push_str("<tr>");
        if self.multiple_iterations {
            html.push_str("<th>Iteration</th>");
        }
        html.push_str("<th>Benchmark</th>");
        html.push_str("<th>Measure<br />Units</th>");
        html.push_str("<th>View</th>");
        html.push_str("<th>Benchmark Result<br />(Result Δ%)</th>");
        if self.has_lower_boundary_alert() {
            html.push_str("<th>Lower Boundary<br />(Limit %)</th>");
        }
        if self.has_upper_boundary_alert() {
            html.push_str("<th>Upper Boundary<br />(Limit %)</th>");
        }
        html.push_str("</tr>");
        html.push_str("</thead>");
    }

    fn html_alerts_table_body(&self, html: &mut String) {
        html.push_str("<tbody>");

        for alert in self.alerts() {
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

            html.push_str("<tr>");
            if self.multiple_iterations {
                html.push_str(&format!("<td>{}</td>", alert.iteration));
            }
            html.push_str(&format!(
                "<td><a href=\"{url}\">{benchmark}</a></td>",
                url = self.resource_url(Resource::Benchmark(alert.benchmark.slug.clone())),
                benchmark = self.benchmark_label(&alert.benchmark, &alert.parameter.set),
            ));
            html.push_str(&format!(
                "<td><a href=\"{url}\">{measure}<br />{units}</a></td>",
                url = self.resource_url(Resource::Measure(alert.threshold.measure.slug.clone())),
                measure =
                    Self::measure_label(&alert.threshold.measure, alert.threshold.metric.as_ref()),
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
            if self.has_lower_boundary_alert() {
                lower_limit_cell(
                    html,
                    alert.value,
                    alert.boundary.lower_limit,
                    factor,
                    &units_symbol,
                    alert.limit == BoundaryLimit::Lower,
                );
            }
            if self.has_upper_boundary_alert() {
                upper_limit_cell(
                    html,
                    alert.value,
                    alert.boundary.upper_limit,
                    factor,
                    &units_symbol,
                    alert.limit == BoundaryLimit::Upper,
                );
            }
            html.push_str("</tr>");
        }
        html.push_str("</tbody>");
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
            url = self.resource_url(Resource::Threshold {
                uuid: alert.threshold.uuid,
                model: alert.threshold.model.as_ref().map(|m| m.uuid),
            }),
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
        for iteration in self.results() {
            self.html_iteration_table(html, iteration, require_threshold);
        }
        html.push_str("</details>");
    }

    fn has_lower_boundary_alert(&self) -> bool {
        self.has_boundary_alert(BoundaryLimit::Lower)
    }

    fn has_upper_boundary_alert(&self) -> bool {
        self.has_boundary_alert(BoundaryLimit::Upper)
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
        let columns = measure_columns(iteration, require_threshold);

        html.push_str("<table>");
        self.html_iteration_table_header(html, &columns);
        self.html_iteration_table_body(html, iteration, &columns);
        html.push_str("</table>");
    }

    fn html_iteration_table_header(
        &self,
        html: &mut String,
        columns: &BTreeMap<Measure, MeasureColumns>,
    ) {
        html.push_str("<thead>");
        html.push_str("<tr>");
        html.push_str("<th>Benchmark</th>");
        for (measure, measure_columns) in columns {
            let units = Units::new(measure_columns.min.into(), measure.units.clone()).scale_units();

            html.push_str(&format!(
                "<th><a href=\"{url}\">{measure}</a></th>",
                url = self.resource_url(Resource::Measure(measure.slug.clone())),
                measure = measure.name,
            ));

            if let Some(boundary_limits) = measure_columns.point_estimate {
                html.push_str("<th>");
                if boundary_limits.has_limit() {
                    html.push_str("Benchmark Result<br />");
                }
                html.push_str(units.as_ref());
                if boundary_limits.has_limit() {
                    html.push_str("<br />(Result Δ%)");
                }
                html.push_str("</th>");

                if boundary_limits.lower {
                    html.push_str(&format!(
                        "<th>Lower Boundary<br />{units}<br />(Limit %)</th>"
                    ));
                }

                if boundary_limits.upper {
                    html.push_str(&format!(
                        "<th>Upper Boundary<br />{units}<br />(Limit %)</th>"
                    ));
                }
            }

            for name in &measure_columns.names {
                html.push_str(&format!(
                    "<th>{measure} ({name})<br />{units}</th>",
                    measure = measure.name,
                ));
            }
        }
        html.push_str("</tr>");
        html.push_str("</thead>");
    }

    fn html_iteration_table_body(
        &self,
        html: &mut String,
        iteration: &JsonReportIteration,
        columns: &BTreeMap<Measure, MeasureColumns>,
    ) {
        html.push_str("<tbody>");
        for result in iteration {
            html.push_str("<tr>");
            html.push_str(&format!(
                "<td><a href=\"{url}\">{name}</a></td>",
                url = self.resource_url(Resource::Benchmark(result.benchmark.slug.clone())),
                name = self.benchmark_label(&result.benchmark, &result.parameter.set),
            ));
            for (measure, measure_columns) in columns {
                self.html_iteration_table_measure_cells(html, result, measure, measure_columns);
            }
            html.push_str("</tr>");
        }
        html.push_str("</tbody>");
    }

    /// Every cell one measure contributes to one row: the view cell, the point
    /// estimate's cells when it has columns, and one cell per name of its own.
    fn html_iteration_table_measure_cells(
        &self,
        html: &mut String,
        result: &JsonReportResult,
        measure: &Measure,
        measure_columns: &MeasureColumns,
    ) {
        let (factor, units_symbol) = {
            let units = Units::new(measure_columns.min.into(), measure.units.clone());
            (units.scale_factor(), units.scale_units_symbol())
        };

        let report_measure = result
            .measures
            .iter()
            .find(|m| m.measure.slug == measure.slug);
        // The point estimate. A measure that named no `value` has nothing for
        // this table to draw, so its cells stay empty.
        let point_estimate = report_measure.and_then(|m| m.metric.as_ref());
        let alert = self.find_alert(result, measure, &MetricName::value());

        if let Some(report_measure) = report_measure {
            self.html_iteration_table_view_cell(
                html,
                result,
                report_measure,
                measure_columns.point_estimate.unwrap_or_default(),
                alert,
            );
        } else {
            html.push_str(EMPTY_CELL);
        }

        if let Some(boundary_limits) = measure_columns.point_estimate {
            if let (Some(report_measure), Some(metric)) = (report_measure, point_estimate) {
                value_cell(
                    html,
                    metric.value,
                    report_measure.boundary.and_then(|b| b.baseline),
                    factor,
                    &units_symbol,
                    alert.is_some(),
                );
            } else {
                html.push_str(EMPTY_CELL);
            }
            if boundary_limits.lower {
                if let (Some(report_measure), Some(metric)) = (report_measure, point_estimate) {
                    lower_limit_cell(
                        html,
                        metric.value,
                        report_measure.boundary.and_then(|b| b.lower_limit),
                        factor,
                        &units_symbol,
                        alert.is_some_and(|a| a.limit == BoundaryLimit::Lower),
                    );
                } else {
                    html.push_str(EMPTY_CELL);
                }
            }
            if boundary_limits.upper {
                if let (Some(report_measure), Some(metric)) = (report_measure, point_estimate) {
                    upper_limit_cell(
                        html,
                        metric.value,
                        report_measure.boundary.and_then(|b| b.upper_limit),
                        factor,
                        &units_symbol,
                        alert.is_some_and(|a| a.limit == BoundaryLimit::Upper),
                    );
                } else {
                    html.push_str(EMPTY_CELL);
                }
            }
        }

        for name in &measure_columns.names {
            let named =
                report_measure.and_then(|m| m.metrics.iter().find(|metric| metric.name == *name));
            if let Some(named) = named {
                // A named value has one column, so what a threshold computed
                // for it rides inside the cell the way a baseline does.
                //
                // The boundaries are in threshold creation order, oldest first, and
                // that order is not a ranking: the first is not the winner. It is
                // taken because a cell draws one baseline and the choice has to be
                // deterministic. This is display only and decides nothing.
                let baseline = named
                    .boundaries
                    .first()
                    .and_then(|boundary| boundary.boundary.baseline);
                let named_alert = self.find_alert(result, measure, name);
                value_cell(
                    html,
                    named.value,
                    baseline,
                    factor,
                    &units_symbol,
                    named_alert.is_some(),
                );
            } else {
                html.push_str(EMPTY_CELL);
            }
        }
    }

    fn html_iteration_table_view_cell(
        &self,
        html: &mut String,
        result: &JsonReportResult,
        report_measure: &JsonReportMeasure,
        boundary_limits: BoundaryLimits,
        alert: Option<&JsonAlert>,
    ) {
        html.push_str("<td>");
        html.push_str(&format!(
            "📈 <a href=\"{url}\">view plot</a>",
            url = self.perf_url(
                &result.benchmark,
                &report_measure.measure,
                Some(boundary_limits)
            )
        ));
        if let Some(threshold) = view_threshold(report_measure) {
            html.push_str("<br />");
            html.push_str(&format!(
                "🚷 <a href=\"{url}\">view threshold</a>",
                url = self.resource_url(Resource::Threshold {
                    uuid: threshold.uuid,
                    model: Some(threshold.model.uuid),
                }),
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

    fn html_footer(&self, html: &mut String) {
        html.push_str(&format!(
            r#"<a href="{url}">🐰 View full continuous benchmarking report in Bencher</a>"#,
            url = self.resource_url(Resource::Report(self.json_report.uuid)),
        ));
    }

    fn html_bencher_tag(&self, html: &mut String, id: Option<&str>) {
        html.push_str(&self.bencher_tag(id));
    }

    // The Bencher tag allows us to easily check whether a comment is a Bencher report when updating
    pub fn bencher_tag(&self, id: Option<&str>) -> String {
        let id = id.map_or_else(
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
        );
        format!(
            r#"<div id="bencher.dev/projects/{project}/id/{id}"></div>"#,
            project = self.json_report.project.slug,
        )
    }

    /// The name of the Project that the Report belongs to.
    /// Used by the `bencher` CLI to name the GitHub Check,
    /// so Reports for different Projects on the same commit do not collide.
    pub fn project_name(&self) -> &ResourceName {
        &self.json_report.project.name
    }

    pub fn has_threshold(&self) -> bool {
        for iteration in self.results() {
            for result in iteration {
                for report_measure in &result.measures {
                    if has_threshold(report_measure) {
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

    /// The alert this row's cell shows, if any.
    ///
    /// Four dimensions are matched: the benchmark, the grid point, the measure, and
    /// the name. Two grid points of one benchmark are two rows and two names of one
    /// measure are two columns, so a cell that matched fewer would show an alert
    /// that fired somewhere else.
    ///
    /// The iteration is a fifth dimension and is deliberately not matched, which is
    /// how this has always behaved. Matching it would change what an existing
    /// multi-iteration BMF v0 comment renders, so it stays as it is.
    pub fn find_alert(
        &self,
        result: &JsonReportResult,
        measure: &Measure,
        metric: &MetricName,
    ) -> Option<&JsonAlert> {
        self.alerts().iter().find(|alert| {
            alert.benchmark.slug == result.benchmark.slug
                && alert.parameter.uuid == result.parameter.uuid
                && alert.threshold.measure.slug == measure.slug
                && alert
                    .threshold
                    .metric
                    .clone()
                    .unwrap_or_else(MetricName::value)
                    == *metric
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
        let query_param = resource.query_param();
        let mut path = if self.public_links {
            format!("/perf/{project}", project = self.project_slug)
        } else {
            format!("/console/projects/{project}", project = self.project_slug)
        };
        if let Some((resource_name, id)) = resource.into_suffix() {
            path.push_str(&format!("/{resource_name}/{id}"));
        }
        let mut url = url.join(&path).unwrap_or(url);

        if let Some((key, value)) = query_param {
            url.query_pairs_mut().append_pair(key, &value);
        }

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

    fn alert_perf_url(&self, alert: &JsonAlert) -> Url {
        self.perf_url(
            &alert.benchmark,
            &alert.threshold.measure,
            Some(BoundaryLimits {
                lower: alert.limit == BoundaryLimit::Lower,
                upper: alert.limit == BoundaryLimit::Upper,
                ..Default::default()
            }),
        )
    }

    fn perf_url(
        &self,
        benchmark: &JsonBenchmark,
        measure: &JsonMeasure,
        boundary_limits: Option<BoundaryLimits>,
    ) -> Url {
        let mut url = self.console_url.clone();

        let path = if self.public_links {
            format!("/perf/{}", self.project_slug)
        } else {
            format!("/console/projects/{}/perf", self.project_slug)
        };
        url.set_path(&path);

        let json_perf_query = JsonPerfQuery {
            branches: vec![self.json_report.branch.uuid],
            heads: vec![Some(self.json_report.branch.head.uuid)],
            testbeds: vec![self.json_report.testbed.uuid],
            #[cfg(feature = "plus")]
            specs: vec![self.json_report.testbed.spec.as_ref().map(|s| s.uuid)],
            benchmarks: vec![benchmark.uuid],
            // The link plots every grid point of the benchmark, the way it did before
            // a benchmark could have more than one.
            parameters: Vec::new(),
            measures: vec![measure.uuid],
            start_time: Some(
                (self.json_report.start_time.into_inner() - DEFAULT_REPORT_HISTORY).into(),
            ),
            end_time: Some(self.json_report.end_time),
        };
        let mut query_string = vec![("report", Some(self.json_report.uuid.to_string()))];
        if boundary_limits.is_some_and(|bl| bl.lower) {
            query_string.push((LOWER_BOUNDARY, Some(true.to_string())));
        }
        if boundary_limits.is_some_and(|bl| bl.upper) {
            query_string.push((UPPER_BOUNDARY, Some(true.to_string())));
        }
        url.set_query(Some(
            &json_perf_query
                .to_query_string(&query_string)
                .unwrap_or_default(),
        ));

        url
    }
}

enum Resource {
    Project,
    Report(ReportUuid),
    Branch {
        slug: BranchSlug,
        head: HeadUuid,
    },
    Testbed {
        slug: TestbedSlug,
        #[cfg(feature = "plus")]
        spec: Option<SpecUuid>,
    },
    Benchmark(BenchmarkSlug),
    Measure(MeasureSlug),
    Threshold {
        uuid: ThresholdUuid,
        model: Option<ModelUuid>,
    },
    Alert(AlertUuid),
}

impl Resource {
    fn into_suffix(self) -> Option<(&'static str, String)> {
        match self {
            Resource::Project => None,
            Resource::Report(uuid) => Some(("reports", uuid.to_string())),
            Resource::Branch { slug, .. } => Some(("branches", slug.to_string())),
            Resource::Testbed { slug, .. } => Some(("testbeds", slug.to_string())),
            Resource::Benchmark(slug) => Some(("benchmarks", slug.to_string())),
            Resource::Measure(slug) => Some(("measures", slug.to_string())),
            Resource::Threshold { uuid, .. } => Some(("thresholds", uuid.to_string())),
            Resource::Alert(uuid) => Some(("alerts", uuid.to_string())),
        }
    }

    fn query_param(&self) -> Option<(&'static str, String)> {
        match self {
            Resource::Branch { head, .. } => Some(("head", head.to_string())),
            #[cfg(feature = "plus")]
            Resource::Testbed {
                spec: Some(spec), ..
            } => Some(("spec", spec.to_string())),
            Resource::Threshold {
                model: Some(model), ..
            } => Some(("model", model.to_string())),
            Resource::Project
            | Resource::Report(_)
            | Resource::Testbed { .. }
            | Resource::Benchmark(_)
            | Resource::Measure(_)
            | Resource::Threshold { model: None, .. }
            | Resource::Alert(_) => None,
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
    fn missing_threshold(json_report: &JsonReport) -> HashSet<Measure> {
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
                        .filter(|&report_measure| !has_threshold(report_measure))
                        .map(|report_measure| Measure::from(report_measure.measure.clone()))
                })
            })
            .collect()
    }
}

fn alert_status(alert: &JsonAlert) -> &str {
    match alert.status {
        AlertStatus::Active => "🔔",
        AlertStatus::Dismissed | AlertStatus::Silenced => "🔕",
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

impl Default for BoundaryLimits {
    fn default() -> Self {
        Self {
            min: 1.0.into(),
            lower: false,
            upper: false,
        }
    }
}

impl From<JsonBoundary> for BoundaryLimits {
    fn from(json_boundary: JsonBoundary) -> Self {
        Self {
            lower: json_boundary.lower_limit.is_some(),
            upper: json_boundary.upper_limit.is_some(),
            ..Default::default()
        }
    }
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

impl BitOrAssign for BoundaryLimits {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl BoundaryLimits {
    fn has_limit(self) -> bool {
        self.lower || self.upper
    }
}

/// The columns one measure contributes to an iteration table.
#[derive(Clone)]
struct MeasureColumns {
    /// The smallest number any column of this measure shows, which is what scales
    /// the units the whole measure is spelled in.
    min: OrderedFloat<f64>,
    /// The point estimate's columns: the value column, and whichever boundary
    /// columns the results produced.
    ///
    /// Absent when no result of this measure named a point estimate, which BMF v1
    /// permits. Such a measure renders its named columns instead of nothing.
    point_estimate: Option<BoundaryLimits>,
    /// Every name this measure carries beyond the conventional trio, in name order.
    ///
    /// One column each, after the point estimate's columns. The trio is what the
    /// existing columns already draw, so it never earns a column of its own.
    names: BTreeSet<MetricName>,
}

/// The threshold this measure's cell links to, if anything gated it.
///
/// The deprecated singular threshold is the bare one, which gates the conventional
/// `value` name of every grid point. Every threshold a BMF v0 payload can create is
/// bare, so a v0 measure that anything gated has it and this never looks any
/// further: the v0 cell is the cell it always was. A measure gated only by a
/// threshold that names a metric or filters grid points has to be read off the rows
/// themselves, which is the only way the cell, `--ci-only-thresholds`, and the no
/// threshold warning can agree about the same measure.
///
/// The rows are already in threshold creation order, oldest first, and nothing about
/// that order is a ranking. The first is taken because a cell links to one threshold
/// and the choice has to be deterministic, not because it won anything.
fn view_threshold(report_measure: &JsonReportMeasure) -> Option<&JsonThresholdModel> {
    report_measure.threshold.as_ref().or_else(|| {
        report_measure
            .metrics
            .iter()
            .flat_map(|metric| metric.boundaries.iter())
            .map(|boundary| &boundary.threshold)
            .next()
    })
}

/// Whether anything gated this measure of this grid point.
///
/// One predicate for the cell, `--ci-only-thresholds`, and the no threshold warning,
/// so no comment can shout `NO THRESHOLD` in a cell while staying silent about that
/// measure in the warning above it.
fn has_threshold(report_measure: &JsonReportMeasure) -> bool {
    view_threshold(report_measure).is_some()
}

/// Whether a name is one of the three the metric triple maps onto.
fn is_conventional(name: &MetricName) -> bool {
    *name == MetricName::value()
        || *name == MetricName::lower_value()
        || *name == MetricName::upper_value()
}

/// Every benchmark whose rows in this comment name the grid point they carry.
///
/// A benchmark is in here when any row of it, result or alert, carries a non-empty
/// parameter set. Results and alerts are counted together so the two tables of one
/// comment never disagree about how a benchmark is named.
fn grid_benchmarks(json_report: &JsonReport) -> HashSet<BenchmarkUuid> {
    let mut grid = HashSet::new();
    for iteration in json_report.results.as_deref().unwrap_or_default() {
        for result in iteration {
            if !result.parameter.set.is_empty() {
                grid.insert(result.benchmark.uuid);
            }
        }
    }
    for alert in json_report.alerts.as_deref().unwrap_or_default() {
        if !alert.parameter.set.is_empty() {
            grid.insert(alert.benchmark.uuid);
        }
    }
    grid
}

fn measure_columns(
    iteration: &JsonReportIteration,
    require_threshold: bool,
) -> BTreeMap<Measure, MeasureColumns> {
    let mut map: BTreeMap<Measure, MeasureColumns> = BTreeMap::new();
    for result in iteration {
        for report_measure in &result.measures {
            let point_estimate = report_measure.metric.as_ref().and_then(|metric| {
                let mut min = metric.value;
                if let Some(lower_limit) = report_measure.boundary.and_then(|b| b.lower_limit) {
                    min = min.min(lower_limit);
                }
                if let Some(upper_limit) = report_measure.boundary.and_then(|b| b.upper_limit) {
                    min = min.min(upper_limit);
                }
                let lower = report_measure
                    .boundary
                    .and_then(|b| b.lower_limit)
                    .is_some();
                let upper = report_measure
                    .boundary
                    .and_then(|b| b.upper_limit)
                    .is_some();
                let boundary_limits = BoundaryLimits { min, lower, upper };
                (!require_threshold || boundary_limits.has_limit()).then_some(boundary_limits)
            });

            // A name beyond the trio earns a column of its own. Under
            // `--ci-only-thresholds` only a gated name does, the same way only a
            // bounded point estimate does.
            let named = report_measure
                .metrics
                .iter()
                .filter(|metric| !is_conventional(&metric.name))
                .filter(|metric| !require_threshold || !metric.boundaries.is_empty())
                .collect::<Vec<_>>();

            // A measure with neither a point estimate nor a name of its own has
            // nothing for this table to draw.
            if point_estimate.is_none() && named.is_empty() {
                continue;
            }

            let measure = Measure::from(report_measure.measure.clone());
            let columns = map.entry(measure).or_insert_with(|| MeasureColumns {
                min: f64::INFINITY.into(),
                point_estimate: None,
                names: BTreeSet::new(),
            });
            if let Some(boundary_limits) = point_estimate {
                columns.min = columns.min.min(boundary_limits.min);
                match &mut columns.point_estimate {
                    Some(point_estimate) => *point_estimate |= boundary_limits,
                    None => columns.point_estimate = Some(boundary_limits),
                }
            }
            for metric in named {
                columns.min = columns.min.min(metric.value);
                columns.names.insert(metric.name.clone());
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use bencher_json::{
        DateTime, JsonBranch, JsonHead, JsonProject, JsonReport, JsonReportCounts, JsonTestbed,
        project::{
            Visibility,
            report::{Adapter, JsonReportAlerts, JsonReportResults},
        },
    };
    use ordered_float::OrderedFloat;

    use crate::{ReportComment, SubAdapter, percent_difference, percent_of, value_cell};

    const CONSOLE_URL: &str = "https://bencher.example.com";

    fn json_report(visibility: Visibility) -> JsonReport {
        let project_uuid = "11111111-1111-1111-1111-111111111111".parse().unwrap();
        JsonReport {
            uuid: "00000000-0000-0000-0000-000000000000".parse().unwrap(),
            user: None,
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
            adapter: Adapter::Magic,
            results: Some(Vec::new()),
            alerts: Some(Vec::new()),
            counts: JsonReportCounts::default(),
            #[cfg(feature = "plus")]
            job: None,
            created: DateTime::TEST,
        }
    }

    fn report_comment(visibility: Visibility) -> ReportComment {
        report_comment_for(json_report(visibility))
    }

    fn report_comment_for(report: JsonReport) -> ReportComment {
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

    /// One iteration with two measures of one benchmark: one that named a point
    /// estimate and one that named only a percentile, with nothing gating either.
    fn value_less_results() -> JsonReportResults {
        value_less_results_gated(&serde_json::json!([]))
    }

    /// The same iteration, with `boundaries` on the percentile row.
    ///
    /// A threshold that names a metric is never the bare one, so the deprecated
    /// singular `threshold` stays absent however many of these there are. That is
    /// the wire shape a BMF v1 report produces and the shape this pins.
    fn named_gate() -> serde_json::Value {
        let date = serde_json::to_value(DateTime::TEST).unwrap();
        serde_json::json!([{
            "threshold": {
                "uuid": "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee",
                "project": "11111111-1111-1111-1111-111111111111",
                "model": {
                    "uuid": "ffffffff-ffff-ffff-ffff-ffffffffffff",
                    "test": "t_test",
                    "min_sample_size": null,
                    "max_sample_size": null,
                    "window": null,
                    "lower_boundary": null,
                    "upper_boundary": 0.95,
                    "created": date,
                    "replaced": null,
                },
                "created": date,
            },
            "boundary": {
                "baseline": 1.0,
                "lower_limit": null,
                "upper_limit": 3.0,
            },
        }])
    }

    /// Built from JSON rather than constructors because the absent deprecated
    /// `metric` is the wire shape under test.
    fn value_less_results_gated(boundaries: &serde_json::Value) -> JsonReportResults {
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
            "parameter": {
                "uuid": "77777777-7777-7777-7777-777777777777",
                "set": {},
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
                        "boundaries": boundaries,
                    }],
                    "threshold": null,
                    "boundary": null,
                },
            ],
        }]]))
        .unwrap()
    }

    /// One iteration of one benchmark on `sets`, one grid point per set, each with
    /// one measure that named a point estimate.
    ///
    /// Built from JSON rather than constructors because the parameter set on the
    /// wire is what is under test.
    fn grid_results(sets: &[serde_json::Value]) -> JsonReportResults {
        let date = serde_json::to_value(DateTime::TEST).unwrap();
        let results = sets
            .iter()
            .enumerate()
            .map(|(index, set)| {
                serde_json::json!({
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
                    "parameter": {
                        "uuid": format!("77777777-7777-7777-7777-77777777777{index}"),
                        "set": set,
                    },
                    "measures": [{
                        "measure": {
                            "uuid": "88888888-8888-8888-8888-888888888888",
                            "project": "11111111-1111-1111-1111-111111111111",
                            "name": "Latency",
                            "slug": "latency",
                            "units": "nanoseconds (ns)",
                            "created": date,
                            "modified": date,
                            "archived": null,
                        },
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
                    }],
                })
            })
            .collect::<Vec<_>>();
        serde_json::from_value(serde_json::json!([results])).unwrap()
    }

    /// One alert on the `p99` row of one grid point of `bench`.
    fn named_alert(set: &serde_json::Value) -> JsonReportAlerts {
        let date = serde_json::to_value(DateTime::TEST).unwrap();
        serde_json::from_value(serde_json::json!([{
            "uuid": "dddddddd-dddd-dddd-dddd-dddddddddddd",
            "report": "00000000-0000-0000-0000-000000000000",
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
            "parameter": {
                "uuid": "77777777-7777-7777-7777-777777777770",
                "benchmark": "66666666-6666-6666-6666-666666666666",
                "set": set,
                "created": date,
                "modified": date,
                "archived": null,
            },
            "value": 2.0,
            "threshold": {
                "uuid": "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee",
                "project": "11111111-1111-1111-1111-111111111111",
                "branch": {
                    "uuid": "33333333-3333-3333-3333-333333333333",
                    "project": "11111111-1111-1111-1111-111111111111",
                    "name": "main",
                    "slug": "main",
                    "head": {
                        "uuid": "44444444-4444-4444-4444-444444444444",
                        "start_point": null,
                        "version": null,
                        "created": date,
                        "replaced": null,
                    },
                    "created": date,
                    "modified": date,
                    "archived": null,
                },
                "testbed": {
                    "uuid": "55555555-5555-5555-5555-555555555555",
                    "project": "11111111-1111-1111-1111-111111111111",
                    "name": "localhost",
                    "slug": "localhost",
                    "created": date,
                    "modified": date,
                    "archived": null,
                },
                "measure": {
                    "uuid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                    "project": "11111111-1111-1111-1111-111111111111",
                    "name": "Throughput",
                    "slug": "throughput",
                    "units": "nanoseconds (ns)",
                    "created": date,
                    "modified": date,
                    "archived": null,
                },
                "metric": "p99",
                "model": null,
                "created": date,
                "modified": date,
            },
            "boundary": {
                "baseline": 1.0,
                "lower_limit": null,
                "upper_limit": 1.5,
            },
            "limit": "upper",
            "status": "active",
            "created": date,
            "modified": date,
        }]))
        .unwrap()
    }

    // A measure that named no point estimate reaches the comment with the deprecated
    // `metric` absent. It has no point estimate column, and every name it did carry
    // beyond the conventional trio gets a column of its own.
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
            html.contains(">Throughput</a></th>"),
            "the measure that named no point estimate still has its named column: {html}"
        );
        assert!(
            html.contains("<th>Throughput (p99)<br />nanoseconds (ns)</th>"),
            "the named value has a column of its own: {html}"
        );
        assert!(
            html.contains("<td>2.00 ns</td>"),
            "the named value is drawn: {html}"
        );
        // It is still a measure of this report, so the threshold warning names it.
        assert!(
            html.contains("/measures/throughput\">Throughput"),
            "unexpected report: {html}"
        );
    }

    // A measure gated only by a threshold that names a metric has no bare threshold,
    // so the deprecated singular field is absent. The cell links to the threshold
    // that did gate it and never shouts NO THRESHOLD, which is what keeps the cell
    // and the report level warning telling the same story about one measure.
    #[test]
    fn report_table_named_threshold_gates_the_cell() {
        let mut report = json_report(Visibility::Public);
        report.results = Some(value_less_results_gated(&named_gate()));

        let comment = report_comment_for(report);
        let html = comment.html(false, None);

        // The measure the named threshold gated: linked, not warned about.
        assert!(
            html.contains(
                "/thresholds/eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee?model=ffffffff-ffff-ffff-ffff-ffffffffffff\">view threshold</a>"
            ),
            "the cell links the threshold that gated the named value: {html}"
        );
        // The warning lists a measure as `<name> (<units>)`, which the table header
        // never does, so this is the warning and not the column.
        assert!(
            !html.contains("/measures/throughput\">Throughput (nanoseconds (ns))</a>"),
            "the warning does not name a measure something gated: {html}"
        );
        // The measure nothing gated is still warned about, in the cell and above it.
        assert_eq!(
            html.matches("⚠️ NO THRESHOLD").count(),
            1,
            "only the ungated measure's cell says so: {html}"
        );
        assert!(
            html.contains("/measures/latency\">Latency (nanoseconds (ns))</a>"),
            "the warning still names the ungated measure: {html}"
        );
        // One gated measure is enough for `--ci-only-thresholds` to post.
        assert!(comment.has_threshold(), "unexpected report: {html}");
    }

    // Nothing gated either measure, so every reader agrees the other way too.
    #[test]
    fn report_table_ungated_measures_have_no_threshold() {
        let mut report = json_report(Visibility::Public);
        report.results = Some(value_less_results());

        let comment = report_comment_for(report);
        assert!(!comment.has_threshold());
        let html = comment.html(false, None);
        assert!(
            !html.contains("view threshold</a>"),
            "unexpected table: {html}"
        );
        assert!(html.contains("⚠️ NO THRESHOLD"), "unexpected table: {html}");
    }

    // A BMF v0 report has nothing but the conventional trio, so no measure earns a
    // named column and the point estimate keeps every column it always had.
    #[test]
    fn report_table_v0_has_no_named_columns() {
        let mut report = json_report(Visibility::Public);
        report.results = Some(grid_results(&[serde_json::json!({})]));

        let html = report_comment_for(report).html(false, None);
        assert!(
            html.contains(">Latency</a></th><th>nanoseconds (ns)</th>"),
            "unexpected table: {html}"
        );
        assert!(
            !html.contains("<th>Latency ("),
            "the conventional trio never earns a column of its own: {html}"
        );
    }

    // A benchmark whose every row in the comment carries the empty parameter set
    // keeps the bare benchmark name it has always had, in the table and in the human
    // text alike.
    #[test]
    fn report_labels_stay_bare_without_grid_points() {
        let mut report = json_report(Visibility::Public);
        report.results = Some(grid_results(&[serde_json::json!({})]));
        let comment = report_comment_for(report);

        let html = comment.html(false, None);
        assert!(
            html.contains("/benchmarks/bench\">bench</a>"),
            "unexpected table: {html}"
        );
        assert!(
            !html.contains("bench {}"),
            "a benchmark with no grid points is named the way it always was: {html}"
        );
        assert!(
            comment.human().contains("\n- bench (Latency): "),
            "unexpected text: {}",
            comment.human()
        );
    }

    // Two grid points of one benchmark are two rows, so each row names the set it
    // carries, and the empty set among them reads `{}`.
    #[test]
    fn report_labels_name_each_grid_point() {
        let mut report = json_report(Visibility::Public);
        report.results = Some(grid_results(&[
            serde_json::json!({}),
            serde_json::json!({ "size_mb": 16 }),
        ]));
        let comment = report_comment_for(report);

        let html = comment.html(false, None);
        assert!(
            html.contains("/benchmarks/bench\">bench {}</a>"),
            "unexpected table: {html}"
        );
        assert!(
            html.contains(r#"/benchmarks/bench">bench {"size_mb":16}</a>"#),
            "unexpected table: {html}"
        );
        let human = comment.human();
        assert!(human.contains("\n- bench {} (Latency): "), "{human}");
        assert!(
            human.contains("\n- bench {\"size_mb\":16} (Latency): "),
            "{human}"
        );
    }

    // The alert table reads the flat value, names the grid point the alert fired on,
    // and names the metric its threshold gates when that is not `value`.
    #[test]
    fn report_alert_names_the_grid_point_and_the_metric() {
        let mut report = json_report(Visibility::Public);
        report.results = Some(grid_results(&[serde_json::json!({ "size_mb": 16 })]));
        report.alerts = Some(named_alert(&serde_json::json!({ "size_mb": 16 })));

        let html = report_comment_for(report).html(false, None);
        assert!(
            html.contains(r#"/benchmarks/bench">bench {"size_mb":16}</a>"#),
            "the alert names the grid point it fired on: {html}"
        );
        assert!(
            html.contains(">Throughput (p99)<br />nanoseconds (ns)</a>"),
            "the alert names the metric its threshold gates: {html}"
        );
        // The flat `value`, drawn against the boundary's baseline.
        assert!(
            html.contains("<b>2.00 ns"),
            "the alert draws its scalar: {html}"
        );
    }

    // A threshold that names no metric gates the conventional `value`, so the alert
    // reads as the measure alone, which is every alert an older client could raise.
    #[test]
    fn report_alert_without_a_metric_names_the_measure_alone() {
        let mut alerts = named_alert(&serde_json::json!({}));
        alerts[0].threshold.metric = None;
        let mut report = json_report(Visibility::Public);
        report.results = Some(grid_results(&[serde_json::json!({})]));
        report.alerts = Some(alerts);

        let html = report_comment_for(report).html(false, None);
        assert!(
            html.contains(">Throughput<br />nanoseconds (ns)</a>"),
            "unexpected table: {html}"
        );
        assert!(
            html.contains("/benchmarks/bench\">bench</a>"),
            "unexpected table: {html}"
        );
    }

    #[test]
    fn report_table_public_project() {
        let html = report_comment(Visibility::Public).html(false, None);
        let project = r#"<tr><td>Project</td><td><a href="https://bencher.example.com/perf/my-project">My Project</a></td></tr>"#;
        let branch = r#"<tr><td>Branch</td><td><a href="https://bencher.example.com/perf/my-project/branches/main?head=44444444-4444-4444-4444-444444444444">main</a></td></tr>"#;
        let testbed = r#"<tr><td>Testbed</td><td><a href="https://bencher.example.com/perf/my-project/testbeds/localhost">localhost</a></td></tr>"#;
        let project_pos = html.find(project).expect("missing project row");
        let branch_pos = html.find(branch).expect("missing branch row");
        let testbed_pos = html.find(testbed).expect("missing testbed row");
        assert!(project_pos < branch_pos);
        assert!(branch_pos < testbed_pos);
    }

    #[cfg(feature = "plus")]
    #[test]
    fn report_table_private_project() {
        let html = report_comment(Visibility::Private).html(false, None);
        let project = r#"<tr><td>Project</td><td><a href="https://bencher.example.com/console/projects/my-project">My Project</a></td></tr>"#;
        let branch = r#"<tr><td>Branch</td><td><a href="https://bencher.example.com/console/projects/my-project/branches/main?head=44444444-4444-4444-4444-444444444444">main</a></td></tr>"#;
        let testbed = r#"<tr><td>Testbed</td><td><a href="https://bencher.example.com/console/projects/my-project/testbeds/localhost">localhost</a></td></tr>"#;
        let project_pos = html.find(project).expect("missing project row");
        let branch_pos = html.find(branch).expect("missing branch row");
        let testbed_pos = html.find(testbed).expect("missing testbed row");
        assert!(project_pos < branch_pos);
        assert!(branch_pos < testbed_pos);
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
