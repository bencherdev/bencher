use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    time::Duration,
};

use bencher_json::{
    project::{
        boundary::BoundaryLimit,
        plot::{LOWER_BOUNDARY, UPPER_BOUNDARY},
        report::JsonReportMeasure,
    },
    AlertUuid, BenchmarkName, BenchmarkUuid, BranchUuid, DateTime, JsonBoundary, JsonPerfQuery,
    JsonReport, MeasureUuid, ResourceName, Slug, TestbedUuid,
};
use url::Url;

pub struct ReportComment {
    console_url: Url,
    project_slug: Slug,
    json_report: JsonReport,
    public_links: bool,
    benchmark_urls: BenchmarkUrls,
    alert_urls: AlertUrls,
    source: String,
}

impl ReportComment {
    pub fn new(console_url: Url, json_report: JsonReport, source: String) -> Self {
        Self {
            alert_urls: AlertUrls::new(&console_url, &json_report),
            benchmark_urls: BenchmarkUrls::new(console_url.clone(), &json_report),
            project_slug: json_report.project.slug.clone(),
            public_links: json_report.project.visibility.is_public(),
            json_report,
            console_url,
            source,
        }
    }

    pub fn human(&self) -> String {
        let mut comment = String::new();

        comment.push_str("View results:");
        let multiple_iterations = self.json_report.results.len() > 1;
        for (iter, benchmark_map) in self.benchmark_urls.0.iter().enumerate() {
            if multiple_iterations {
                comment.push_str(&format!("\nIteration {iter}"));
            }
            for (benchmark, measure_map) in benchmark_map {
                for (measure, MeasureData { console_url, .. }) in measure_map {
                    comment.push_str(&format!(
                        "\n- {benchmark_name} ({measure_name}): {console_url}",
                        benchmark_name = benchmark.name,
                        measure_name = measure.name
                    ));
                }
            }
            if multiple_iterations {
                comment.push_str("\n");
            }
        }

        if self.json_report.alerts.is_empty() {
            return comment;
        }

        comment.push_str("\nView alerts:");
        for ((benchmark, measure), AlertData { console_url, .. }) in &self.alert_urls.0 {
            comment.push_str(&format!(
                "\n- {benchmark_name} ({measure_name}): {console_url}",
                benchmark_name = benchmark.name,
                measure_name = measure.name,
            ));
        }

        comment
    }

    pub fn json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.json_report)
    }

    pub fn html(&self, require_threshold: bool, id: Option<&str>) -> String {
        let mut html = String::new();
        let html_mut = &mut html;
        self.html_header(html_mut);
        self.html_report_table(html_mut);
        self.html_benchmarks(html_mut, require_threshold);
        self.html_footer(html_mut);
        // DO NOT MOVE: The Bencher tag must be the last thing in the HTML for updates to work
        self.html_bencher_tag(html_mut, id);
        html
    }

    fn utm_query(&self) -> String {
        format!(
            "utm_medium=referral&utm_source={source}&utm_content=comment&utm_campaign=pr+comments&utm_term={project}",
            source = self.source,
            project = self.project_slug,
        )
    }

    fn html_header(&self, html: &mut String) {
        let url = self.console_url.clone();
        let path = if self.public_links {
            format!(
                "/perf/{}/reports/{}",
                self.project_slug, self.json_report.uuid
            )
        } else {
            format!(
                "/console/projects/{}/reports/{}",
                self.project_slug, self.json_report.uuid
            )
        };
        let report_url = url.join(&path).unwrap_or(url);
        html.push_str(&format!(
            r#"<h1><a href="{report_url}?{utm}"><img src="https://bencher.dev/favicon.svg" width="32" height="32" alt="🐰" />Bencher Report</a></h1>"#,
            utm = self.utm_query(),
        ));
    }

    fn html_report_table(&self, html: &mut String) {
        html.push_str("<table>");
        for (row, name, path) in [
            (
                "Branch",
                self.json_report.branch.name.to_string(),
                if self.public_links {
                    format!(
                        "/perf/{}/branches/{}",
                        self.project_slug, self.json_report.branch.slug
                    )
                } else {
                    format!(
                        "/console/projects/{}/branches/{}",
                        self.project_slug, self.json_report.branch.slug
                    )
                },
            ),
            (
                "Testbed",
                self.json_report.testbed.name.to_string(),
                if self.public_links {
                    format!(
                        "/perf/{}/testbeds/{}",
                        self.project_slug, self.json_report.testbed.slug
                    )
                } else {
                    format!(
                        "/console/projects/{}/testbeds/{}",
                        self.project_slug, self.json_report.testbed.slug
                    )
                },
            ),
        ] {
            let url = self.console_url.clone();
            let url = url.join(&path).unwrap_or(url);
            html.push_str(&format!(
                r#"<tr><td>{row}</td><td><a href="{url}?{utm}">{name}</a></td></tr>"#,
                utm = self.utm_query()
            ));
        }
        html.push_str("</table>");
    }

    fn html_benchmarks(&self, html: &mut String, require_threshold: bool) {
        let no_benchmarks = self.benchmark_urls.0.iter().all(BTreeMap::is_empty);
        if no_benchmarks {
            html.push_str("<blockquote><b>⚠️ WARNING:</b> No benchmarks found!</blockquote>");
            return;
        }
        self.html_no_threshold_warning(html);

        let alerts_len = self.alert_urls.0.len();
        if alerts_len > 0 {
            html.push_str("<br />");
            let (capital, lower) = if alerts_len == 1 {
                ("", "")
            } else {
                ("S", "s")
            };
            html.push_str(&format!(
                "<blockquote><b>🚨 {alerts_len} ALERT{capital}:</b> Threshold Boundary Limit{lower} exceeded!</blockquote>",
            ));
            self.html_alerts_table(html, self.public_links);
            html.push_str("<br />");
        }

        html.push_str("<details><summary>Click to view all benchmark results</summary>");
        self.html_benchmarks_table(html, measures, require_threshold, self.public_links);
        html.push_str("</details>");
    }

    // Check to see if any measure has a threshold set
    fn html_no_threshold_warning(&self, html: &mut String) {
        let mut no_threshold = BTreeSet::new();
        for benchmark_map in &self.benchmark_urls.0 {
            for measure_map in benchmark_map.values() {
                for (measure, MeasureData { boundary, .. }) in measure_map {
                    if boundary.is_none() {
                        no_threshold.insert(measure);
                    }
                }
            }
        }

        if no_threshold.is_empty() {
            return;
        }
        let plural_measure = if no_threshold.len() == 1 {
            "Measure does"
        } else {
            "Measures do"
        };
        html.push_str(&format!("<blockquote><p><b>⚠️ WARNING:</b> The following {plural_measure} not have a Threshold. Without a Threshold, no Alerts will ever be generated!</p>"));
        html.push_str("<ul>");
        for measure in no_threshold {
            let url = self.console_url.clone();
            let path = if self.public_links {
                format!("/perf/{}/measures/{}", self.project_slug, measure.slug)
            } else {
                format!(
                    "/console/projects/{}/measures/{}",
                    self.project_slug, measure.slug
                )
            };
            let url = url.join(&path).unwrap_or(url);
            html.push_str(&format!(
                "<li><a href=\"{url}?{utm}\">{name}</a></li>",
                utm = self.utm_query(),
                name = measure.name,
            ));
        }
        html.push_str("</ul>");
        html.push_str(&format!("<p><a href=\"{console_url}console/projects/{project}/thresholds/add?{utm}\">Click here to create a new Threshold</a><br />", console_url = self.console_url, project = self.project_slug, utm = self.utm_query()));
        html.push_str(&format!("For more information, see <a href=\"https://bencher.dev/docs/explanation/thresholds/?{utm}\">the Threshold documentation</a>.<br />", utm = self.utm_query()));
        html.push_str(&format!("To only post results if a Threshold exists, set <a href=\"https://bencher.dev/docs/explanation/bencher-run/#--ci-only-thresholds?{utm}\">the <code lang=\"rust\">--ci-only-thresholds</code> CLI flag</a>.</p>", utm = self.utm_query()));
        html.push_str("</blockquote>");
    }

    fn html_alerts_table(&self, html: &mut String, public_links: bool) {
        html.push_str("<table>");
        html.push_str("<thead><tr><th>Benchmark</th><th>Measure (units)</th><th>View</th><th>Value</th><th>Lower Boundary</th><th>Upper Boundary</th></tr></thead>");
        html.push_str("<tbody>");
        for ((benchmark, measure), alert) in &self.alert_urls.0 {
            let Some(data) = self
                .benchmark_urls
                .0
                .get(benchmark)
                .and_then(|m| m.get(measure))
            else {
                continue;
            };

            html.push_str("<tr>");
            if public_links {
                html.push_str(&format!("<td>{name}</td>", name = benchmark.name));
                html.push_str(&format!(
                    "<td>{name} ({units})</td>",
                    name = measure.name,
                    units = measure.units
                ));
                html.push_str(&format!(
                    r#"<td>🚨 (<a href="{}">view plot</a> | <a href="{}">view alert</a>)</td>"#,
                    data.public_url, alert.public_url,
                ));
            } else {
                let benchmark_path = format!(
                    "/console/projects/{}/benchmarks/{}",
                    self.project_slug, benchmark.slug
                );
                let url = self.console_url.clone();
                let url = url.join(&benchmark_path).unwrap_or(url);
                html.push_str(&format!(
                    r#"<td><a href="{url}">{name}</a></td>"#,
                    name = benchmark.name,
                ));
                let measure_path = format!(
                    "/console/projects/{}/measure/{}",
                    self.project_slug, measure.slug
                );
                let url = self.console_url.clone();
                let url = url.join(&measure_path).unwrap_or(url);
                html.push_str(&format!(
                    r#"<td><a href="{url}">{name}</a></td>"#,
                    name = measure.name,
                ));
                html.push_str(&format!(
                    r#"<td>🚨 (<a href="{}">view plot</a> | <a href="{}">view alert</a>)</td>"#,
                    data.console_url, alert.console_url,
                ));
            }
            Self::html_metric_boundary_cells(
                html,
                data.value,
                data.boundary,
                Some(alert.limit),
                true,
            );
            html.push_str("</tr>");
        }
        html.push_str("</tbody>");
        html.push_str("</table>");
    }

    fn html_benchmarks_table(
        &self,
        html: &mut String,
        measures: &MeasuresMap,
        require_threshold: bool,
        public_links: bool,
    ) {
        html.push_str("<table>");
        self.html_benchmarks_table_header(html, measures, require_threshold, public_links);
        self.html_benchmarks_table_body(html, require_threshold, public_links);
        html.push_str("</table>");
    }

    fn html_benchmarks_table_header(
        &self,
        html: &mut String,
        measures: &MeasuresMap,
        require_threshold: bool,
        public_links: bool,
    ) {
        html.push_str("<thead><tr>");
        html.push_str("<th>Benchmark</th>");
        for (measure, MeasureData { boundary, .. }) in measures {
            if require_threshold && boundary.is_none() {
                continue;
            }
            let measure_name = &measure.name;
            if public_links {
                html.push_str(&format!("<th>{measure_name}</th>"));
            } else {
                let measure_path = format!(
                    "/console/projects/{}/measures/{}",
                    self.project_slug, measure.slug
                );
                let url = self.console_url.clone();
                let url = url.join(&measure_path).unwrap_or(url);
                html.push_str(&format!(r#"<th><a href="{url}">{measure_name}</a></th>"#));
            }
            Self::html_metric_boundary_header(html, measure, *boundary);
        }
        html.push_str("</tr></thead>");
    }

    fn html_metric_boundary_header(
        html: &mut String,
        measure: &Measure,
        boundary: Option<Boundary>,
    ) {
        let name = &measure.name;
        let units = &measure.units;
        // If there is a boundary then we will show the percentage difference
        if boundary.is_some() {
            html.push_str(&format!("<th>{name} Results<br/>{units} | (Δ%)</th>",));
        } else {
            html.push_str(&format!("<th>{name} Results<br/>{units}</th>",));
        }

        let Some(boundary) = boundary else {
            return;
        };
        if boundary.lower_limit.is_some() {
            html.push_str(&format!("<th>{name} Lower Boundary<br/>{units} | (%)</th>"));
        }
        if boundary.upper_limit.is_some() {
            html.push_str(&format!("<th>{name} Upper Boundary<br/>{units} | (%)</th>"));
        }
    }

    fn html_benchmarks_table_body(
        &self,
        html: &mut String,
        require_threshold: bool,
        public_links: bool,
    ) {
        html.push_str("<tbody>");
        for (benchmark, measures) in &self.benchmark_urls.0 {
            html.push_str("<tr>");
            if public_links {
                html.push_str(&format!("<td>{name}</td>", name = benchmark.name,));
            } else {
                let benchmark_path = format!(
                    "/console/projects/{}/benchmarks/{}",
                    self.project_slug, benchmark.slug
                );
                let url = self.console_url.clone();
                let url = url.join(&benchmark_path).unwrap_or(url);
                html.push_str(&format!(
                    r#"<td><a href="{url}">{name}</a></td>"#,
                    name = benchmark.name,
                ));
            }
            for (
                measure,
                MeasureData {
                    public_url,
                    console_url,
                    value,
                    boundary,
                },
            ) in measures
            {
                if require_threshold && boundary.is_none() {
                    continue;
                }
                let plot_url = if public_links {
                    public_url
                } else {
                    console_url
                };
                let (alert_url, limit) = if let Some(alert) =
                    self.alert_urls.0.get(&(benchmark.clone(), measure.clone()))
                {
                    let AlertData {
                        public_url,
                        console_url,
                        limit,
                    } = alert;
                    (
                        Some(if public_links {
                            public_url
                        } else {
                            console_url
                        }),
                        Some(*limit),
                    )
                } else {
                    (None, None)
                };
                let row = if let Some(alert_url) = alert_url {
                    format!(
                        r#"🚨 (<a href="{plot_url}">view plot</a> | <a href="{alert_url}">view alert</a>)"#,
                    )
                } else if boundary.is_some() {
                    format!(r#"✅ (<a href="{plot_url}">view plot</a>)"#)
                } else {
                    format!(r#"➖ (<a href="{plot_url}">view plot</a>)"#)
                };
                html.push_str(&format!(r#"<td>{row}</td>"#));

                Self::html_metric_boundary_cells(html, *value, *boundary, limit, false);
            }
            html.push_str("</tr>");
        }
        html.push_str("</tbody>");
    }

    fn html_metric_boundary_cells(
        html: &mut String,
        value: f64,
        boundary: Option<Boundary>,
        limit: Option<BoundaryLimit>,
        pad: bool,
    ) {
        // If there is a boundary with a baseline then show the percentage difference
        if let Some(Boundary {
            baseline: Some(baseline),
            ..
        }) = boundary
        {
            let value_percent = if value.is_normal() && baseline.is_normal() {
                ((value - baseline) / baseline) * 100.0
            } else {
                0.0
            };
            let value_plus = if value_percent > 0.0 { "+" } else { "" };

            let bold = limit.is_some();
            html.push_str(&format!(
                "<td>{}{} ({value_plus}{}%){}</td>",
                if bold { "<b>" } else { "" },
                format_number(value),
                format_number(value_percent),
                if bold { "</b>" } else { "" },
            ));
        } else {
            html.push_str(&format!("<td>{}</td>", format_number(value)));
        }

        let Some(boundary) = boundary else {
            return;
        };
        if let Some(lower_limit) = boundary.lower_limit {
            let limit_percent = if value.is_normal() && lower_limit.is_normal() {
                (lower_limit / value) * 100.0
            } else {
                0.0
            };
            let bold = matches!(limit, Some(BoundaryLimit::Lower));
            html.push_str(&format!(
                "<td>{}{} ({}%){}</td>",
                if bold { "<b>" } else { "" },
                format_number(lower_limit),
                format_number(limit_percent),
                if bold { "</b>" } else { "" },
            ));
        } else if pad {
            html.push_str("<td></td>");
        }
        if let Some(upper_limit) = boundary.upper_limit {
            let limit_percent = if value.is_normal() && upper_limit.is_normal() {
                (value / upper_limit) * 100.0
            } else {
                0.0
            };
            let bold = matches!(limit, Some(BoundaryLimit::Upper));
            html.push_str(&format!(
                "<td>{}{} ({}%){}</td>",
                if bold { "<b>" } else { "" },
                format_number(upper_limit),
                format_number(limit_percent),
                if bold { "</b>" } else { "" },
            ));
        } else if pad {
            html.push_str("<td></td>");
        }
    }

    fn html_footer(&self, html: &mut String) {
        html.push_str(&format!(r#"<br/><small><a href="https://bencher.dev">Bencher - Continuous Benchmarking</a></small>{}<br/><small><a href="https://bencher.dev/docs/">Docs</a> | <a href="https://bencher.dev/repo/">Repo</a> | <a href="https://bencher.dev/chat/">Chat</a> | <a href="https://bencher.dev/help/">Help</a></small>"#,
        if self.json_report.project.visibility.is_public() {
            let path = format!("/perf/{}", self.project_slug);
            let url = self.console_url.clone();
            let url = url.join(&path).unwrap_or(url);
            format!(r#"<br/><small><a href="{url}">View Public Perf Page</a></small>"#)
        } else {
            String::new()
        }
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
        self.benchmark_urls.has_threshold()
    }

    pub fn has_alert(&self) -> bool {
        !self.json_report.alerts.is_empty()
    }
}

pub struct BenchmarkUrls(Vec<BenchmarkMap>);
pub type BenchmarkMap = BTreeMap<Benchmark, MeasureMap>;
pub type MeasureMap = BTreeMap<Measure, MeasureData>;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct Benchmark {
    name: BenchmarkName,
    slug: Slug,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Measure {
    name: ResourceName,
    slug: Slug,
    units: ResourceName,
}

#[derive(Clone)]
pub struct MeasureData {
    pub public_url: Url,
    pub console_url: Url,
    pub value: f64,
    pub boundary: Option<Boundary>,
}

impl BenchmarkUrls {
    pub fn new(console_url: Url, json_report: &JsonReport) -> Self {
        let benchmark_url = BenchmarkUrl::new(
            console_url,
            json_report.project.slug.clone(),
            json_report.branch.uuid,
            json_report.testbed.uuid,
            json_report.start_time,
            json_report.end_time,
        );

        let mut benchmark_urls = Vec::with_capacity(json_report.results.len());

        for iteration in &json_report.results {
            let mut benchmark_map = BTreeMap::new();
            for result in iteration {
                let benchmark = Benchmark {
                    name: result.benchmark.name.clone(),
                    slug: result.benchmark.slug.clone(),
                };

                let mut measure_map = BTreeMap::new();
                for report_measure in &result.measures {
                    let measure = Measure {
                        name: report_measure.measure.name.clone(),
                        slug: report_measure.measure.slug.clone(),
                        units: report_measure.measure.units.clone(),
                    };
                    let boundary = report_measure.boundary.map(Into::into);

                    let data = MeasureData {
                        public_url: benchmark_url.to_public_url(
                            result.benchmark.uuid,
                            report_measure.measure.uuid,
                            boundary,
                        ),
                        console_url: benchmark_url.to_console_url(
                            result.benchmark.uuid,
                            report_measure.measure.uuid,
                            boundary,
                        ),
                        value: report_measure.metric.value.into(),
                        boundary,
                    };
                    measure_map.insert(measure, data);
                }
                benchmark_map.insert(benchmark, measure_map);
            }
            benchmark_urls.push(benchmark_map);
        }

        Self(benchmark_urls)
    }

    fn has_threshold(&self) -> bool {
        self.0.values().any(Self::benchmark_has_threshold)
    }

    fn benchmark_has_threshold(measures: &MeasureMap) -> bool {
        measures
            .values()
            .any(|MeasureData { boundary, .. }| boundary.is_some())
    }
}

struct BenchmarkUrl {
    console_url: Url,
    project_slug: Slug,
    branch: BranchUuid,
    testbed: TestbedUuid,
    start_time: DateTime,
    end_time: DateTime,
}

// 30 days
const DEFAULT_REPORT_HISTORY: Duration = Duration::from_secs(30 * 24 * 60 * 60);

impl BenchmarkUrl {
    fn new(
        console_url: Url,
        project_slug: Slug,
        branch: BranchUuid,
        testbed: TestbedUuid,
        start_time: DateTime,
        end_time: DateTime,
    ) -> Self {
        Self {
            console_url,
            project_slug,
            branch,
            testbed,
            start_time,
            end_time,
        }
    }

    fn to_public_url(
        &self,
        benchmark: BenchmarkUuid,
        measure: MeasureUuid,
        boundary: Option<Boundary>,
    ) -> Url {
        self.to_url(benchmark, measure, boundary, true)
    }

    fn to_console_url(
        &self,
        benchmark: BenchmarkUuid,
        measure: MeasureUuid,
        boundary: Option<Boundary>,
    ) -> Url {
        self.to_url(benchmark, measure, boundary, false)
    }

    fn to_url(
        &self,
        benchmark: BenchmarkUuid,
        measure: MeasureUuid,
        boundary: Option<Boundary>,
        public_links: bool,
    ) -> Url {
        let json_perf_query = JsonPerfQuery {
            branches: vec![self.branch],
            testbeds: vec![self.testbed],
            benchmarks: vec![benchmark],
            measures: vec![measure],
            start_time: Some((self.start_time.into_inner() - DEFAULT_REPORT_HISTORY).into()),
            end_time: Some(self.end_time),
        };

        let mut url = self.console_url.clone();
        let path = if public_links {
            format!("/perf/{}", self.project_slug)
        } else {
            format!("/console/projects/{}/perf", self.project_slug)
        };
        url.set_path(&path);
        url.set_query(Some(
            &json_perf_query
                .to_query_string(&boundary.map(Boundary::to_query_string).unwrap_or_default())
                .unwrap_or_default(),
        ));

        url
    }
}

#[derive(Clone, Copy)]
pub struct Boundary {
    baseline: Option<f64>,
    lower_limit: Option<f64>,
    upper_limit: Option<f64>,
}

impl From<JsonBoundary> for Boundary {
    fn from(json_boundary: JsonBoundary) -> Self {
        Self {
            baseline: json_boundary.baseline.map(Into::into),
            lower_limit: json_boundary.lower_limit.map(Into::into),
            upper_limit: json_boundary.upper_limit.map(Into::into),
        }
    }
}

impl Boundary {
    fn to_query_string(self) -> Vec<(&'static str, Option<String>)> {
        let mut query_string = Vec::new();
        if self.lower_limit.is_some() {
            query_string.push((LOWER_BOUNDARY, Some(true.to_string())));
        }
        if self.upper_limit.is_some() {
            query_string.push((UPPER_BOUNDARY, Some(true.to_string())));
        }
        query_string
    }

    pub fn is_empty(self) -> bool {
        self.lower_limit.is_none() && self.upper_limit.is_none()
    }
}

pub struct AlertUrls(BTreeMap<(Benchmark, Measure), AlertData>);

#[derive(Clone)]
pub struct AlertData {
    pub public_url: Url,
    pub console_url: Url,
    pub limit: BoundaryLimit,
}

impl AlertUrls {
    pub fn new(console_url: &Url, json_report: &JsonReport) -> Self {
        let mut urls = BTreeMap::new();

        for alert in &json_report.alerts {
            let benchmark = Benchmark {
                name: alert.benchmark.name.clone(),
                slug: alert.benchmark.slug.clone(),
            };
            let measure = Measure {
                name: alert.threshold.measure.name.clone(),
                slug: alert.threshold.measure.slug.clone(),
                units: alert.threshold.measure.units.clone(),
            };
            let public_url =
                Self::to_public_url(console_url.clone(), &json_report.project.slug, alert.uuid);
            let console_url =
                Self::to_console_url(console_url.clone(), &json_report.project.slug, alert.uuid);
            let data = AlertData {
                public_url,
                console_url,
                limit: alert.limit,
            };
            urls.insert((benchmark, measure), data);
        }

        Self(urls)
    }

    fn to_public_url(mut console_url: Url, project_slug: &Slug, alert: AlertUuid) -> Url {
        console_url.set_path(&format!("/perf/{project_slug}/alerts/{alert}"));
        console_url
    }

    fn to_console_url(mut console_url: Url, project_slug: &Slug, alert: AlertUuid) -> Url {
        console_url.set_path(&format!("/console/projects/{project_slug}/alerts/{alert}"));
        console_url
    }
}

enum Position {
    Whole(usize),
    Point,
    Decimal,
}

fn format_number(number: f64) -> String {
    let mut number_str = String::new();
    let mut position = Position::Decimal;
    for c in format!("{:.2}", number.abs()).chars().rev() {
        match position {
            Position::Whole(place) => {
                if place % 3 == 0 {
                    number_str.push(',');
                }
                position = Position::Whole(place + 1);
            },
            Position::Point => {
                position = Position::Whole(1);
            },
            Position::Decimal => {
                if c == '.' {
                    position = Position::Point;
                }
            },
        }
        number_str.push(c);
    }
    if number < 0.0 {
        number_str.push('-');
    }
    number_str.chars().rev().collect()
}
