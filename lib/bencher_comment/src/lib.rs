use std::{
    collections::{btree_map::Entry, BTreeMap, HashSet},
    ops::{BitOr, BitOrAssign},
    time::Duration,
};

use bencher_json::{
    project::{
        alert::AlertStatus,
        boundary::BoundaryLimit,
        plot::{LOWER_BOUNDARY, UPPER_BOUNDARY},
        report::{JsonReportIteration, JsonReportMeasure, JsonReportResult},
    },
    AlertUuid, JsonAlert, JsonBenchmark, JsonBoundary, JsonMeasure, JsonPerfQuery, JsonReport,
    ReportUuid, ResourceName, Slug, ThresholdUuid, Units,
};
use ordered_float::OrderedFloat;
use url::Url;

// 30 days
const DEFAULT_REPORT_HISTORY: Duration = Duration::from_secs(30 * 24 * 60 * 60);

const EMPTY_CELL: &str = "<td></td>";

pub struct ReportComment {
    console_url: Url,
    project_slug: Slug,
    public_links: bool,
    multiple_iterations: bool,
    benchmark_count: usize,
    missing_threshold: HashSet<Measure>,
    json_report: JsonReport,
    source: String,
}

impl ReportComment {
    pub fn new(console_url: Url, json_report: JsonReport, source: String) -> Self {
        Self {
            console_url,
            project_slug: json_report.project.slug.clone(),
            public_links: json_report.project.visibility.is_public(),
            multiple_iterations: json_report.results.len() > 1,
            benchmark_count: json_report.results.iter().map(Vec::len).sum(),
            missing_threshold: Measure::missing_threshold(&json_report),
            json_report,
            source,
        }
    }

    pub fn human(&self) -> String {
        let mut text = String::new();
        self.human_results_list(&mut text);
        self.human_alerts_list(&mut text);
        text
    }

    fn human_results_list(&self, text: &mut String) {
        text.push_str("View results:");
        for (i, iteration) in self.json_report.results.iter().enumerate() {
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
                        benchmark = result.benchmark.name,
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
        if self.json_report.alerts.is_empty() {
            return;
        }

        text.push_str("\n\nView alerts:");
        for alert in &self.json_report.alerts {
            text.push_str(&format!(
                "\n- {benchmark_name} ({measure_name}){iter}: {console_url}",
                benchmark_name = alert.benchmark.name,
                measure_name = alert.threshold.measure.name,
                iter = if self.multiple_iterations {
                    format!(" (Iteration {iteration})", iteration = alert.iteration)
                } else {
                    String::new()
                },
                console_url = self.alert_perf_url(alert)
            ));
        }
    }

    pub fn json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.json_report)
    }

    pub fn html(&self, require_threshold: bool, id: Option<&str>) -> String {
        self.html_inner(require_threshold, id, true)
    }

    pub fn html_with_max_length(
        &self,
        require_threshold: bool,
        id: Option<&str>,
        max_length: usize,
    ) -> String {
        let html = self.html_inner(require_threshold, id, true);
        if html.len() > max_length {
            self.html_inner(require_threshold, id, false)
        } else {
            html
        }
    }

    fn html_inner(
        &self,
        require_threshold: bool,
        id: Option<&str>,
        include_benchmarks: bool,
    ) -> String {
        let mut html = String::new();
        let html_mut = &mut html;
        self.html_header(html_mut);
        self.html_report_table(html_mut);
        if include_benchmarks {
            self.html_benchmarks(html_mut, require_threshold);
        }
        self.html_footer(html_mut);
        // DO NOT MOVE: The Bencher tag must be the last thing in the HTML for updates to work
        self.html_bencher_tag(html_mut, id);
        html
    }

    fn html_header(&self, html: &mut String) {
        html.push_str(&format!(
            r#"<h2><a href="{url}"><img src="https://bencher.dev/favicon.svg" width="24" height="24" alt="🐰" /> Bencher Report</a></h2>"#,
            url = self.resource_url(Resource::Report(self.json_report.uuid)),
        ));
    }

    fn html_report_table(&self, html: &mut String) {
        html.push_str("<table>");
        for (row, name, url) in [
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
            html.push_str(&format!(
                r#"<tr><td>{row}</td><td><a href="{url}">{name}</a></td></tr>"#,
            ));
        }
        html.push_str("</table>");
    }

    fn html_benchmarks(&self, html: &mut String, require_threshold: bool) {
        self.html_no_benchmarks(html);
        self.html_no_threshold(html);
        self.html_alerts(html);
        self.html_benchmark_details(html, require_threshold);
    }

    fn html_no_benchmarks(&self, html: &mut String) {
        if self.benchmark_count == 0 {
            html.push_str("<blockquote><h3>⚠️ WARNING: No benchmarks found!</h3></blockquote>");
        }
    }

    fn html_no_threshold(&self, html: &mut String) {
        if self.benchmark_count == 0 || self.missing_threshold.is_empty() {
            return;
        }

        html.push_str("<blockquote>");
        html.push_str("<h3>⚠️ WARNING: No Threshold found!</h3>");
        html.push_str("<p>Without a Threshold, no Alerts will ever be generated.</p>");

        html.push_str("<ul>");
        for Measure { name, slug, units } in &self.missing_threshold {
            let url = self.resource_url(Resource::Measure(slug.clone()));
            html.push_str(&format!("<li><a href=\"{url}\">{name} ({units})</a></li>"));
        }
        html.push_str("</ul>");

        html.push_str(&format!("<p><a href=\"{console_url}console/projects/{project}/thresholds/add{utm}\">Click here to create a new Threshold</a><br />", console_url = self.console_url, project = self.project_slug, utm = self.utm_query()));
        html.push_str(&format!("For more information, see <a href=\"https://bencher.dev/docs/explanation/thresholds/{utm}\">the Threshold documentation</a>.<br />", utm = self.utm_query()));
        html.push_str(&format!("To only post results if a Threshold exists, set <a href=\"https://bencher.dev/docs/explanation/bencher-run/#--ci-only-thresholds{utm}\">the <code lang=\"rust\">--ci-only-thresholds</code> CLI flag</a>.</p>", utm = self.utm_query()));
        html.push_str("</blockquote>");
    }

    fn html_alerts(&self, html: &mut String) {
        if self.json_report.alerts.is_empty() {
            return;
        }
        let alerts_len = self.json_report.alerts.len();
        html.push_str(&format!(
            "<h3>🚨 {alerts_len} {alert}</h3>",
            alert = if alerts_len == 1 { "Alert" } else { "Alerts" },
        ));
        self.html_alerts_table(html);
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

        for alert in &self.json_report.alerts {
            let (factor, units) = {
                let mut min = alert.metric.value;
                if let Some(lower_limit) = alert.boundary.lower_limit {
                    min = min.min(lower_limit);
                }
                if let Some(upper_limit) = alert.boundary.upper_limit {
                    min = min.min(upper_limit);
                }
                let units = Units::new(min.into(), alert.threshold.measure.units.clone());
                (units.scale_factor(), units.scale_units())
            };

            html.push_str("<tr>");
            if self.multiple_iterations {
                html.push_str(&format!("<td>{}</td>", alert.iteration));
            }
            html.push_str(&format!(
                "<td><a href=\"{url}\">{benchmark}</a></td>",
                url = self.resource_url(Resource::Benchmark(alert.benchmark.slug.clone())),
                benchmark = alert.benchmark.name,
            ));
            html.push_str(&format!(
                "<td><a href=\"{url}\">{measure}<br />{units}</a></td>",
                url = self.resource_url(Resource::Measure(alert.threshold.measure.slug.clone())),
                measure = alert.threshold.measure.name,
            ));
            self.html_alerts_table_view_cell(html, alert);
            value_cell(
                html,
                alert.metric.value,
                alert.boundary.baseline,
                factor,
                true,
            );
            if self.has_lower_boundary_alert() {
                lower_limit_cell(
                    html,
                    alert.metric.value,
                    alert.boundary.lower_limit,
                    factor,
                    alert.limit == BoundaryLimit::Lower,
                );
            }
            if self.has_upper_boundary_alert() {
                upper_limit_cell(
                    html,
                    alert.metric.value,
                    alert.boundary.upper_limit,
                    factor,
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
            "🚨 <a href=\"{url}\">alert ({status})</a>",
            url = self.resource_url(Resource::Alert(alert.uuid)),
            status = alert_status(alert),
        ));
        html.push_str("<br />");
        html.push_str(&format!(
            "🚷 <a href=\"{url}\">threshold</a>",
            url = self.resource_url(Resource::Threshold(alert.threshold.uuid)),
        ));
        html.push_str("</td>");
    }

    fn html_benchmark_details(&self, html: &mut String, require_threshold: bool) {
        if self.benchmark_count == 0 {
            return;
        }

        html.push_str("<details><summary>Click to view all benchmark results</summary>");
        html.push_str("<br />");
        for iteration in &self.json_report.results {
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
        self.json_report
            .alerts
            .iter()
            .any(|alert| alert.limit == boundary_limit)
    }

    fn html_iteration_table(
        &self,
        html: &mut String,
        iteration: &JsonReportIteration,
        require_threshold: bool,
    ) {
        let mbl = boundary_limits_map(iteration, require_threshold);

        html.push_str("<table>");
        self.html_iteration_table_header(html, &mbl);
        self.html_iteration_table_body(html, iteration, &mbl);
        html.push_str("</table>");
    }

    fn html_iteration_table_header(
        &self,
        html: &mut String,
        mbl: &BTreeMap<Measure, BoundaryLimits>,
    ) {
        html.push_str("<thead>");
        html.push_str("<tr>");
        html.push_str("<th>Benchmark</th>");
        for (measure, boundary_limits) in mbl {
            let units = Units::new(boundary_limits.min.into(), measure.units.clone()).scale_units();

            html.push_str(&format!(
                "<th><a href=\"{url}\">{measure}</a></th>",
                url = self.resource_url(Resource::Measure(measure.slug.clone())),
                measure = measure.name,
            ));

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
        html.push_str("</tr>");
        html.push_str("</thead>");
    }

    fn html_iteration_table_body(
        &self,
        html: &mut String,
        iteration: &JsonReportIteration,
        mbl: &BTreeMap<Measure, BoundaryLimits>,
    ) {
        html.push_str("<tbody>");
        for result in iteration {
            html.push_str("<tr>");
            html.push_str(&format!(
                "<td><a href=\"{url}\">{name}</a></td>",
                url = self.resource_url(Resource::Benchmark(result.benchmark.slug.clone())),
                name = result.benchmark.name,
            ));
            for (measure, boundary_limits) in mbl {
                let factor =
                    Units::new(boundary_limits.min.into(), measure.units.clone()).scale_factor();

                let report_measure = result
                    .measures
                    .iter()
                    .find(|m| m.measure.slug == measure.slug);
                let alert = self.find_alert(result, measure);

                if let Some(report_measure) = report_measure {
                    self.html_iteration_table_view_cell(
                        html,
                        result,
                        report_measure,
                        *boundary_limits,
                        alert,
                    );
                } else {
                    html.push_str(EMPTY_CELL);
                }
                if let Some(report_measure) = report_measure {
                    value_cell(
                        html,
                        report_measure.metric.value,
                        report_measure.boundary.and_then(|b| b.baseline),
                        factor,
                        alert.is_some(),
                    );
                } else {
                    html.push_str(EMPTY_CELL);
                }
                if boundary_limits.lower {
                    if let Some(report_measure) = report_measure {
                        lower_limit_cell(
                            html,
                            report_measure.metric.value,
                            report_measure.boundary.and_then(|b| b.lower_limit),
                            factor,
                            alert.is_some_and(|a| a.limit == BoundaryLimit::Lower),
                        );
                    } else {
                        html.push_str(EMPTY_CELL);
                    }
                }
                if boundary_limits.upper {
                    if let Some(report_measure) = report_measure {
                        upper_limit_cell(
                            html,
                            report_measure.metric.value,
                            report_measure.boundary.and_then(|b| b.upper_limit),
                            factor,
                            alert.is_some_and(|a| a.limit == BoundaryLimit::Upper),
                        );
                    } else {
                        html.push_str(EMPTY_CELL);
                    }
                }
            }
            html.push_str("</tr>");
        }
        html.push_str("</tbody>");
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
        if let Some(alert) = alert {
            html.push_str("<br />");
            html.push_str(&format!(
                "🚨 <a href=\"{url}\">view alert ({status})</a>",
                url = self.resource_url(Resource::Alert(alert.uuid)),
                status = alert_status(alert),
            ));
        }
        if let Some(threshold) = &report_measure.threshold {
            html.push_str("<br />");
            html.push_str(&format!(
                "🚷 <a href=\"{url}\">view threshold</a>",
                url = self.resource_url(Resource::Threshold(threshold.uuid)),
            ));
        } else {
            html.push_str("<br />");
            html.push_str("⚠️ NO THRESHOLD");
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
                    "{branch}/{testbed}/{adapter}",
                    branch = self.json_report.branch.slug,
                    testbed = self.json_report.testbed.slug,
                    adapter = self.json_report.adapter
                )
            },
            ToString::to_string,
        );
        format!(
            r#"<div id="bencher.dev/projects/{project}/id/{id}"></div>"#,
            project = self.json_report.project.slug,
        )
    }

    pub fn has_threshold(&self) -> bool {
        for iteration in &self.json_report.results {
            for result in iteration {
                for report_measure in &result.measures {
                    if report_measure.threshold.is_some() {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn has_alert(&self) -> bool {
        !self.json_report.alerts.is_empty()
    }

    pub fn find_alert(&self, result: &JsonReportResult, measure: &Measure) -> Option<&JsonAlert> {
        self.json_report.alerts.iter().find(|alert| {
            alert.benchmark.slug == result.benchmark.slug
                && alert.threshold.measure.slug == measure.slug
        })
    }

    #[cfg_attr(not(feature = "plus"), allow(clippy::unused_self))]
    fn is_bencher_cloud(&self) -> bool {
        #[cfg(feature = "plus")]
        {
            bencher_json::is_bencher_cloud(&self.console_url)
        }
        #[cfg(not(feature = "plus"))]
        false
    }

    fn resource_url(&self, resource: Resource) -> Url {
        let url = self.console_url.clone();
        let path = if self.public_links {
            format!(
                "/perf/{project}/{resource_name}/{id}",
                project = self.project_slug,
                resource_name = resource.name(),
                id = resource.into_id()
            )
        } else {
            format!(
                "/console/projects/{project}/{resource_name}/{id}",
                project = self.project_slug,
                resource_name = resource.name(),
                id = resource.into_id()
            )
        };
        let mut url = url.join(&path).unwrap_or(url);

        if self.is_bencher_cloud() {
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
            benchmarks: vec![benchmark.uuid],
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
    Report(ReportUuid),
    Branch(Slug),
    Testbed(Slug),
    Benchmark(Slug),
    Measure(Slug),
    Threshold(ThresholdUuid),
    Alert(AlertUuid),
}

impl Resource {
    fn name(&self) -> &'static str {
        match self {
            Resource::Report(_) => "reports",
            Resource::Branch(_) => "branches",
            Resource::Testbed(_) => "testbeds",
            Resource::Benchmark(_) => "benchmarks",
            Resource::Measure(_) => "measures",
            Resource::Threshold(_) => "thresholds",
            Resource::Alert(_) => "alerts",
        }
    }

    fn into_id(self) -> String {
        match self {
            Resource::Report(uuid) => uuid.to_string(),
            Resource::Branch(slug)
            | Resource::Testbed(slug)
            | Resource::Benchmark(slug)
            | Resource::Measure(slug) => slug.into(),
            Resource::Threshold(uuid) => uuid.to_string(),
            Resource::Alert(uuid) => uuid.to_string(),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Measure {
    name: ResourceName,
    slug: Slug,
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
            .iter()
            .flat_map(|iteration| {
                iteration.iter().flat_map(|result| {
                    result
                        .measures
                        .iter()
                        .filter(|&report_measure| report_measure.threshold.is_none())
                        .cloned()
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
    bold: bool,
) {
    fn value_cell_inner(
        value: OrderedFloat<f64>,
        baseline: Option<OrderedFloat<f64>>,
        factor: OrderedFloat<f64>,
    ) -> String {
        let mut cell = Units::format_number((value / factor).into());

        if let Some(baseline) = baseline {
            let percent = if value.is_normal() && baseline.is_normal() {
                ((value - baseline) / baseline) * 100.0
            } else {
                0.0.into()
            };
            let plus = if percent > 0.0.into() { "+" } else { "" };
            let percent = Units::format_number(percent.into());
            cell.push_str(&format!("<br />({plus}{percent}%)"));
        }

        cell
    }

    html.push_str("<td>");
    if bold {
        html.push_str(&format!(
            "<b>{}</b>",
            value_cell_inner(value, baseline, factor)
        ));
    } else {
        html.push_str(&value_cell_inner(value, baseline, factor));
    }
    html.push_str("</td>");
}

fn lower_limit_cell(
    html: &mut String,
    value: OrderedFloat<f64>,
    lower_limit: Option<OrderedFloat<f64>>,
    factor: OrderedFloat<f64>,
    bold: bool,
) {
    let Some(limit) = lower_limit else {
        html.push_str(EMPTY_CELL);
        return;
    };

    let percent = if value.is_normal() && limit.is_normal() {
        (limit / value) * 100.0
    } else {
        0.0.into()
    };

    limit_cell(html, limit, percent, factor, bold);
}

fn upper_limit_cell(
    html: &mut String,
    value: OrderedFloat<f64>,
    upper_limit: Option<OrderedFloat<f64>>,
    factor: OrderedFloat<f64>,
    bold: bool,
) {
    let Some(limit) = upper_limit else {
        html.push_str(EMPTY_CELL);
        return;
    };

    let percent = if value.is_normal() && limit.is_normal() {
        (value / limit) * 100.0
    } else {
        0.0.into()
    };

    limit_cell(html, limit, percent, factor, bold);
}

fn limit_cell(
    html: &mut String,
    limit: OrderedFloat<f64>,
    percent: OrderedFloat<f64>,
    factor: OrderedFloat<f64>,
    bold: bool,
) {
    fn limit_cell_inner(
        limit: OrderedFloat<f64>,
        percent: OrderedFloat<f64>,
        factor: OrderedFloat<f64>,
    ) -> String {
        let mut cell = Units::format_number((limit / factor).into());
        let percent = Units::format_number(percent.into());
        cell.push_str(&format!("<br />({percent}%)"));
        cell
    }

    html.push_str("<td>");
    if bold {
        html.push_str(&format!(
            "<b>{}</b>",
            limit_cell_inner(limit, percent, factor)
        ));
    } else {
        html.push_str(&limit_cell_inner(limit, percent, factor));
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

fn boundary_limits_map(
    iteration: &JsonReportIteration,
    require_threshold: bool,
) -> BTreeMap<Measure, BoundaryLimits> {
    let mut map = BTreeMap::new();
    for result in iteration {
        for report_measure in &result.measures {
            let measure = Measure::from(report_measure.measure.clone());
            let min = {
                let mut min = report_measure.metric.value;
                if let Some(lower_limit) = report_measure.boundary.and_then(|b| b.lower_limit) {
                    min = min.min(lower_limit);
                }
                if let Some(upper_limit) = report_measure.boundary.and_then(|b| b.upper_limit) {
                    min = min.min(upper_limit);
                }
                min
            };
            let lower = report_measure
                .boundary
                .and_then(|b| b.lower_limit)
                .is_some();
            let upper = report_measure
                .boundary
                .and_then(|b| b.upper_limit)
                .is_some();
            let boundary_limits = BoundaryLimits { min, lower, upper };
            if require_threshold && !boundary_limits.has_limit() {
                continue;
            }
            match map.entry(measure) {
                Entry::Occupied(mut entry) => {
                    let entry = entry.get_mut();
                    *entry |= boundary_limits;
                },
                Entry::Vacant(entry) => {
                    entry.insert(boundary_limits);
                },
            }
        }
    }
    map
}
