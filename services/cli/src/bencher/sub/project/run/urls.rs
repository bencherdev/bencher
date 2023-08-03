use std::collections::BTreeMap;

use bencher_json::{
    project::threshold::JsonThresholdStatistic, BenchmarkName, JsonPerfQuery, JsonReport, NonEmpty,
    Slug,
};
use url::Url;
use uuid::Uuid;

pub struct ReportUrls {
    endpoint_url: Url,
    json_report: JsonReport,
    benchmark_urls: BenchmarkUrls,
    alert_urls: AlertUrls,
}

impl ReportUrls {
    pub fn new(endpoint_url: Url, json_report: JsonReport) -> Self {
        Self {
            alert_urls: AlertUrls::new(endpoint_url.clone(), &json_report),
            benchmark_urls: BenchmarkUrls::new(endpoint_url.clone(), &json_report),
            json_report,
            endpoint_url,
        }
    }

    pub fn html(&self) -> String {
        let mut html = String::new();

        // 🐰 Bencher Header
        html.push_str(&format!(
            r#"<h2><a href="{endpoint_url}"><img src="https://s3.amazonaws.com/public.bencher.dev/bencher_rabbit.svg" width="32" height="32" alt="🐰" /></a>Bencher</h2>"#,
            endpoint_url = self.endpoint_url,
        ));

        // Report Table
        html.push_str("<table>");
        for (row, name, path) in [
            (
                "Report",
                self.json_report
                    .start_time
                    .format("%a, %B %e, %Y at %X %Z")
                    .to_string(),
                format!(
                    "/console/projects/{}/reports/{}",
                    self.json_report.project.slug, self.json_report.uuid
                ),
            ),
            (
                "Project",
                self.json_report.project.name.to_string(),
                format!("/console/projects/{}", self.json_report.project.slug),
            ),
            (
                "Branch",
                self.json_report.branch.name.to_string(),
                format!(
                    "/console/projects/{}/branches/{}",
                    self.json_report.project.slug, self.json_report.branch.slug
                ),
            ),
            (
                "Testbed",
                self.json_report.testbed.name.to_string(),
                format!(
                    "/console/projects/{}/testbeds/{}",
                    self.json_report.project.slug, self.json_report.testbed.slug
                ),
            ),
        ] {
            let url = self.endpoint_url.clone().join(&path).unwrap();
            html.push_str(&format!(
                r#"<tr><td>{row}</td><td><a href="{url}">{name}</a></td></tr>"#
            ));
        }
        html.push_str("</table>");

        // Per Metric Kind Benchmarks Table
        for (metric_kind, benchmarks) in &self.benchmark_urls.0 {
            let metric_kind_path = format!(
                "/console/projects/{}/metric-kinds/{}",
                self.json_report.project.slug, metric_kind.slug
            );
            html.push_str(&format!(
                r#"<br/><h3><a href="{url}">{name}</a></h3>"#,
                name = metric_kind.name,
                url = self.endpoint_url.clone().join(&metric_kind_path).unwrap()
            ));
            html.push_str("<table>");
            for (benchmark, url) in benchmarks {
                let alert_url = self
                    .alert_urls
                    .0
                    .get(&(metric_kind.clone(), benchmark.clone()));

                let row = if let Some(alert_url) = alert_url {
                    format!(
                        r#"🚨 (<a href="{url}">view plot</a> | <a href="{alert_url}">view alert</a>)"#,
                    )
                } else {
                    format!(r#"✅ (<a href="{url}">view plot</a>)"#)
                };

                html.push_str(&format!(r#"<tr><td>{benchmark}</td><td>{row}</td></tr>"#,));
            }
            html.push_str("</table>");
        }

        // Footer
        html.push_str(r#"<br/><small><a href="https://bencher.dev">Bencher - Continuous Benchmarking</a></small><br/><small><a href="https://bencher.dev/docs">Docs</a> | <a href="https://bencher.dev/repo">Repo</a> | <a href="https://bencher.dev/chat">Chat</a> | <a href="https://bencher.dev/help">Help</a></small>"#);

        // DO NOT MOVE: The Bencher tag must be the last thing in the HTML for updates to work
        html.push_str(&self.bencher_tag());
        html
    }

    // The Bencher tag allows us to easily check whether a comment is a Bencher report when updating
    pub fn bencher_tag(&self) -> String {
        format!(
            r#"<div id="bencher.dev/projects/{project}/testbeds/{testbed}"></div>"#,
            project = self.json_report.project.uuid,
            testbed = self.json_report.testbed.uuid
        )
    }

    pub fn has_alerts(&self) -> bool {
        !self.json_report.alerts.is_empty()
    }
}

impl std::fmt::Display for ReportUrls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "\nView results:")?;
        for (metric_kind, benchmarks) in &self.benchmark_urls.0 {
            writeln!(f, "\n{}:", metric_kind.name)?;
            for (benchmark, url) in benchmarks {
                writeln!(f, "- {benchmark}: {url}")?;
            }
        }

        if self.json_report.alerts.is_empty() {
            return Ok(());
        }

        writeln!(f, "\nView alerts:")?;
        for ((_metric_kind, benchmark), url) in &self.alert_urls.0 {
            writeln!(f, "- {benchmark}: {url}")?;
        }
        writeln!(f, "\n")?;

        Ok(())
    }
}

pub struct BenchmarkUrls(BTreeMap<MetricKind, BTreeMap<BenchmarkName, Url>>);

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
        );

        let mut urls = BTreeMap::new();
        if let Some(iteration) = json_report.results.first() {
            for result in iteration {
                let mut metric_kind_urls = BTreeMap::new();
                let boundary = result.threshold.as_ref().map(Into::into);
                for benchmark_metric in &result.benchmarks {
                    metric_kind_urls.insert(
                        benchmark_metric.name.clone(),
                        benchmark_url.to_url(
                            result.metric_kind.slug.clone(),
                            benchmark_metric.uuid,
                            boundary.as_ref(),
                        ),
                    );
                }
                let metric_kind = MetricKind {
                    name: result.metric_kind.name.clone(),
                    slug: result.metric_kind.slug.clone(),
                };
                urls.insert(metric_kind, metric_kind_urls);
            }
        }

        Self(urls)
    }
}

struct BenchmarkUrl {
    endpoint: Url,
    project_slug: Slug,
    branch: Uuid,
    testbed: Uuid,
}

impl BenchmarkUrl {
    fn new(endpoint: Url, project_slug: Slug, branch: Uuid, testbed: Uuid) -> Self {
        Self {
            endpoint,
            project_slug,
            branch,
            testbed,
        }
    }

    fn to_url(&self, metric_kind: Slug, benchmark: Uuid, boundary: Option<&BoundaryParam>) -> Url {
        let json_perf_query = JsonPerfQuery {
            metric_kind: metric_kind.into(),
            branches: vec![self.branch],
            testbeds: vec![self.testbed],
            benchmarks: vec![benchmark],
            start_time: None,
            end_time: None,
        };

        let mut url = self.endpoint.clone();
        url.set_path(&format!("/console/projects/{}/perf", self.project_slug));
        url.set_query(Some(
            &json_perf_query
                .to_query_string(
                    &boundary
                        .map(BoundaryParam::to_query_string)
                        .unwrap_or(vec![]),
                )
                .unwrap_or_default(),
        ));

        url
    }
}

struct BoundaryParam {
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
    fn to_query_string(&self) -> Vec<(&str, Option<String>)> {
        let mut query_string = Vec::new();
        if self.lower_boundary {
            query_string.push(("lower_boundary", Some(true.to_string())));
        }
        if self.upper_boundary {
            query_string.push(("upper_boundary", Some(true.to_string())));
        }
        query_string
    }
}

pub struct AlertUrls(BTreeMap<(MetricKind, BenchmarkName), Url>);

impl AlertUrls {
    pub fn new(endpoint_url: Url, json_report: &JsonReport) -> Self {
        let mut urls = BTreeMap::new();

        for alert in &json_report.alerts {
            let metric_kind = MetricKind {
                name: alert.threshold.metric_kind.name.clone(),
                slug: alert.threshold.metric_kind.slug.clone(),
            };
            let alert_url =
                Self::to_url(endpoint_url.clone(), &json_report.project.slug, alert.uuid);
            urls.insert((metric_kind, alert.benchmark.name.clone()), alert_url);
        }

        Self(urls)
    }

    fn to_url(mut endpoint: Url, project_slug: &Slug, alert: Uuid) -> Url {
        endpoint.set_path(&format!(
            "/console/projects/{}/alerts/{}",
            project_slug, alert
        ));
        endpoint
    }
}
