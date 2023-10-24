use std::{collections::BTreeMap, time::Duration};

use bencher_json::{
    project::threshold::JsonThresholdStatistic, AlertUuid, BenchmarkName, BenchmarkUuid,
    BranchUuid, DateTime, JsonPerfQuery, JsonReport, NonEmpty, Slug, TestbedUuid,
};
use url::Url;

pub struct ReportUrls {
    endpoint_url: Url,
    project_slug: Slug,
    json_report: JsonReport,
    benchmark_urls: BenchmarkUrls,
    alert_urls: AlertUrls,
}

impl ReportUrls {
    pub fn new(endpoint_url: Url, json_report: JsonReport) -> Self {
        Self {
            alert_urls: AlertUrls::new(&endpoint_url, &json_report),
            benchmark_urls: BenchmarkUrls::new(endpoint_url.clone(), &json_report),
            project_slug: json_report.project.slug.clone(),
            json_report,
            endpoint_url,
        }
    }

    pub fn html(&self, require_threshold: bool, id: Option<&NonEmpty>) -> String {
        let mut html = String::new();
        let html_mut = &mut html;
        self.html_header(html_mut);
        self.html_report_table(html_mut);
        self.html_benchmarks_table(html_mut, require_threshold);
        self.html_footer(html_mut);
        // DO NOT MOVE: The Bencher tag must be the last thing in the HTML for updates to work
        self.html_bencher_tag(html_mut, id);
        html
    }

    fn html_header(&self, html: &mut String) {
        html.push_str(&format!(
            r#"<h1><a href="{endpoint_url}"><img src="https://s3.amazonaws.com/public.bencher.dev/bencher_rabbit.svg" width="32" height="32" alt="🐰" /></a>Bencher</h1>"#,
            endpoint_url = self.endpoint_url,
        ));
    }

    fn html_report_table(&self, html: &mut String) {
        html.push_str("<table>");
        for (row, name, path) in [
            (
                "Report",
                self.json_report
                    .start_time
                    .into_inner()
                    .format("%a, %B %e, %Y at %X %Z")
                    .to_string(),
                format!(
                    "/console/projects/{}/reports/{}",
                    self.project_slug, self.json_report.uuid
                ),
            ),
            (
                "Project",
                self.json_report.project.name.to_string(),
                format!("/console/projects/{}", self.project_slug),
            ),
            (
                "Branch",
                self.json_report.branch.name.to_string(),
                format!(
                    "/console/projects/{}/branches/{}",
                    self.project_slug, self.json_report.branch.slug
                ),
            ),
            (
                "Testbed",
                self.json_report.testbed.name.to_string(),
                format!(
                    "/console/projects/{}/testbeds/{}",
                    self.project_slug, self.json_report.testbed.slug
                ),
            ),
        ] {
            let url = self.endpoint_url.clone();
            let url = url.join(&path).unwrap_or(url);
            html.push_str(&format!(
                r#"<tr><td>{row}</td><td><a href="{url}">{name}</a></td></tr>"#
            ));
        }
        html.push_str("</table>");
    }

    fn html_benchmarks_table(&self, html: &mut String, require_threshold: bool) {
        let Some((_benchmark, metric_kinds)) = self.benchmark_urls.0.first_key_value() else {
            html.push_str("<b>No benchmarks found!</b>");
            return;
        };

        html.push_str("<table>");

        html.push_str("<tr>");
        html.push_str("<th>Benchmark</th>");
        for (metric_kind, (_url, boundary)) in metric_kinds {
            if require_threshold && !BenchmarkUrls::boundary_has_threshold(*boundary) {
                continue;
            }
            let metric_kind_path = format!(
                "/console/projects/{}/metric-kinds/{}",
                self.project_slug, metric_kind.slug
            );
            let url = self.endpoint_url.clone();
            let url = url.join(&metric_kind_path).unwrap_or(url);
            html.push_str(&format!(
                r#"<th><a href="{url}">{name}</a></th>"#,
                name = metric_kind.name,
            ));
        }
        html.push_str("</tr>");

        for (benchmark, metric_kinds) in &self.benchmark_urls.0 {
            html.push_str("<tr>");
            let benchmark_path = format!(
                "/console/projects/{}/benchmarks/{}",
                self.project_slug, benchmark.slug
            );
            let url = self.endpoint_url.clone();
            let url = url.join(&benchmark_path).unwrap_or(url);
            html.push_str(&format!(
                r#"<td><a href="{url}">{name}</a></td>"#,
                name = benchmark.name,
            ));
            for (metric_kind, (url, boundary)) in metric_kinds {
                if require_threshold && !BenchmarkUrls::boundary_has_threshold(*boundary) {
                    continue;
                }
                let alert_url = self
                    .alert_urls
                    .0
                    .get(&(benchmark.clone(), metric_kind.clone()));

                let row = if let Some(alert_url) = alert_url {
                    format!(
                        r#"🚨 (<a href="{url}">view plot</a> | <a href="{alert_url}">view alert</a>)"#,
                    )
                } else if boundary.map(BoundaryParam::is_empty).unwrap_or(true) {
                    format!(r#"➖ (<a href="{url}">view plot</a>)"#)
                } else {
                    format!(r#"✅ (<a href="{url}">view plot</a>)"#)
                };
                html.push_str(&format!(r#"<td>{row}</td>"#));
            }
            html.push_str("</tr>");
        }

        html.push_str("</table>");
    }

    fn html_footer(&self, html: &mut String) {
        html.push_str(&format!(r#"<br/><small><a href="https://bencher.dev">Bencher - Continuous Benchmarking</a></small>{}<br/><small><a href="https://bencher.dev/docs">Docs</a> | <a href="https://bencher.dev/repo">Repo</a> | <a href="https://bencher.dev/chat">Chat</a> | <a href="https://bencher.dev/help">Help</a></small>"#,
        if self.json_report.project.visibility.is_public() {
            let path = format!("/perf/{}", self.project_slug);
            let url = self.endpoint_url.clone();
            let url = url.join(&path).unwrap_or(url);
            format!(r#"<br/><small><a href="{url}">View Public Perf Page</a></small>"#)
        } else {
            String::new()
        }
        ));
    }

    fn html_bencher_tag(&self, html: &mut String, id: Option<&NonEmpty>) {
        html.push_str(&self.bencher_tag(id));
    }

    // The Bencher tag allows us to easily check whether a comment is a Bencher report when updating
    pub fn bencher_tag(&self, id: Option<&NonEmpty>) -> String {
        let id = id.map_or_else(
            || {
                format!(
                    "{branch}/{testbed}/{adapter:?}",
                    branch = self.json_report.branch.uuid,
                    testbed = self.json_report.testbed.uuid,
                    adapter = self.json_report.adapter
                )
            },
            ToString::to_string,
        );
        format!(
            r#"<div id="bencher.dev/projects/{project}/id/{id}"></div>"#,
            project = self.json_report.project.uuid,
        )
    }

    pub fn has_threshold(&self) -> bool {
        self.benchmark_urls.has_threshold()
    }

    pub fn has_alert(&self) -> bool {
        !self.json_report.alerts.is_empty()
    }
}

impl std::fmt::Display for ReportUrls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "\nView results:")?;
        for (benchmark, metric_kinds) in &self.benchmark_urls.0 {
            for (metric_kind, (url, _boundary)) in metric_kinds {
                writeln!(
                    f,
                    "- {benchmark_name} ({metric_kind_name}): {url}",
                    benchmark_name = benchmark.name,
                    metric_kind_name = metric_kind.name
                )?;
            }
        }

        if self.json_report.alerts.is_empty() {
            return Ok(());
        }

        writeln!(f, "\nView alerts:")?;
        for ((benchmark, metric_kind), url) in &self.alert_urls.0 {
            writeln!(
                f,
                "- {benchmark_name} ({metric_kind_name}): {url}",
                benchmark_name = benchmark.name,
                metric_kind_name = metric_kind.name,
            )?;
        }
        writeln!(f, "\n")?;

        Ok(())
    }
}

pub struct BenchmarkUrls(BTreeMap<Benchmark, MetricKindsMap>);
pub type MetricKindsMap = BTreeMap<MetricKind, (Url, Option<BoundaryParam>)>;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct Benchmark {
    name: BenchmarkName,
    slug: Slug,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MetricKind {
    name: NonEmpty,
    slug: Slug,
}

impl BenchmarkUrls {
    pub fn new(endpoint_url: Url, json_report: &JsonReport) -> Self {
        let benchmark_url = BenchmarkUrl::new(
            endpoint_url,
            json_report.project.slug.clone(),
            json_report.branch.uuid,
            json_report.testbed.uuid,
            json_report.start_time,
            json_report.end_time,
        );

        let mut urls = BTreeMap::new();
        if let Some(iteration) = json_report.results.first() {
            for result in iteration {
                let metric_kind = MetricKind {
                    name: result.metric_kind.name.clone(),
                    slug: result.metric_kind.slug.clone(),
                };
                let boundary = result.threshold.as_ref().map(Into::into);
                for benchmark_metric in &result.benchmarks {
                    let benchmark = Benchmark {
                        name: benchmark_metric.name.clone(),
                        slug: benchmark_metric.slug.clone(),
                    };
                    let benchmark_urls = urls.entry(benchmark).or_insert_with(BTreeMap::new);

                    benchmark_urls.insert(
                        metric_kind.clone(),
                        (
                            benchmark_url.to_url(
                                result.metric_kind.slug.clone(),
                                benchmark_metric.uuid,
                                boundary,
                            ),
                            boundary,
                        ),
                    );
                }
            }
        }

        Self(urls)
    }

    fn has_threshold(&self) -> bool {
        self.0.values().any(Self::benchmark_has_threshold)
    }

    fn benchmark_has_threshold(metric_kinds: &MetricKindsMap) -> bool {
        metric_kinds
            .values()
            .any(|(_, boundary)| Self::boundary_has_threshold(*boundary))
    }

    fn boundary_has_threshold(boundary: Option<BoundaryParam>) -> bool {
        boundary.is_some_and(|b| !b.is_empty())
    }
}

struct BenchmarkUrl {
    endpoint: Url,
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
        endpoint: Url,
        project_slug: Slug,
        branch: BranchUuid,
        testbed: TestbedUuid,
        start_time: DateTime,
        end_time: DateTime,
    ) -> Self {
        Self {
            endpoint,
            project_slug,
            branch,
            testbed,
            start_time,
            end_time,
        }
    }

    fn to_url(
        &self,
        metric_kind: Slug,
        benchmark: BenchmarkUuid,
        boundary: Option<BoundaryParam>,
    ) -> Url {
        let json_perf_query = JsonPerfQuery {
            metric_kind: metric_kind.into(),
            branches: vec![self.branch],
            testbeds: vec![self.testbed],
            benchmarks: vec![benchmark],
            start_time: Some((self.start_time.into_inner() - DEFAULT_REPORT_HISTORY).into()),
            end_time: Some(self.end_time),
        };

        let mut url = self.endpoint.clone();
        url.set_path(&format!("/console/projects/{}/perf", self.project_slug));
        url.set_query(Some(
            &json_perf_query
                .to_query_string(
                    &boundary
                        .map(BoundaryParam::to_query_string)
                        .unwrap_or_default(),
                )
                .unwrap_or_default(),
        ));

        url
    }
}

#[derive(Clone, Copy)]
pub struct BoundaryParam {
    lower_boundary: bool,
    upper_boundary: bool,
}

impl From<&JsonThresholdStatistic> for BoundaryParam {
    fn from(json_threshold_statistic: &JsonThresholdStatistic) -> Self {
        Self {
            lower_boundary: json_threshold_statistic.statistic.lower_boundary.is_some(),
            upper_boundary: json_threshold_statistic.statistic.upper_boundary.is_some(),
        }
    }
}

impl BoundaryParam {
    fn to_query_string(self) -> Vec<(&'static str, Option<String>)> {
        let mut query_string = Vec::new();
        if self.lower_boundary {
            query_string.push(("lower_boundary", Some(true.to_string())));
        }
        if self.upper_boundary {
            query_string.push(("upper_boundary", Some(true.to_string())));
        }
        query_string
    }

    pub fn is_empty(self) -> bool {
        !self.lower_boundary && !self.upper_boundary
    }
}

pub struct AlertUrls(BTreeMap<(Benchmark, MetricKind), Url>);

impl AlertUrls {
    pub fn new(endpoint_url: &Url, json_report: &JsonReport) -> Self {
        let mut urls = BTreeMap::new();

        for alert in &json_report.alerts {
            let benchmark = Benchmark {
                name: alert.benchmark.name.clone(),
                slug: alert.benchmark.slug.clone(),
            };
            let metric_kind = MetricKind {
                name: alert.threshold.metric_kind.name.clone(),
                slug: alert.threshold.metric_kind.slug.clone(),
            };
            let alert_url =
                Self::to_url(endpoint_url.clone(), &json_report.project.slug, alert.uuid);
            urls.insert((benchmark, metric_kind), alert_url);
        }

        Self(urls)
    }

    fn to_url(mut endpoint: Url, project_slug: &Slug, alert: AlertUuid) -> Url {
        endpoint.set_path(&format!("/console/projects/{project_slug}/alerts/{alert}"));
        endpoint
    }
}
