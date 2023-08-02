use std::collections::BTreeMap;

use bencher_json::{project::threshold::JsonThresholdStatistic, JsonPerfQuery, JsonReport, Slug};
use url::Url;
use uuid::Uuid;

pub struct ReportUrls {
    json_report: JsonReport,
    benchmark_urls: BenchmarkUrls,
    alert_urls: AlertUrls,
}

impl ReportUrls {
    pub fn new(endpoint_url: Url, json_report: JsonReport) -> Self {
        Self {
            benchmark_urls: BenchmarkUrls::new(endpoint_url.clone(), &json_report),
            alert_urls: AlertUrls::new(endpoint_url, &json_report),
            json_report,
        }
    }
}

impl std::fmt::Display for ReportUrls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "\nView results:");
        for (name, url) in &self.benchmark_urls.0 {
            writeln!(f, "- {name}: {url}");
        }

        if self.json_report.alerts.is_empty() {
            return Ok(());
        }

        writeln!(f, "\nView alerts:");
        for (name, url) in &self.alert_urls.0 {
            writeln!(f, "- {name}: {url}");
        }
        writeln!(f, "\n");

        Ok(())
    }
}

pub struct BenchmarkUrls(BTreeMap<String, Url>);

impl BenchmarkUrls {
    pub fn new(endpoint_url: Url, json_report: &JsonReport) -> Self {
        let benchmark_url = BenchmarkUrl::new(
            endpoint_url,
            json_report.project.slug.clone(),
            json_report.branch.uuid,
            json_report.testbed.uuid,
        );

        let mut urls = BTreeMap::new();
        for iteration in &json_report.results {
            for result in iteration {
                let boundary = result.threshold.as_ref().map(Into::into);
                for benchmark_metric in &result.benchmarks {
                    urls.insert(
                        benchmark_metric.name.to_string(),
                        benchmark_url.to_url(
                            result.metric_kind.slug.clone(),
                            benchmark_metric.uuid,
                            boundary.as_ref(),
                        ),
                    );
                }
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

pub struct AlertUrls(Vec<(String, Url)>);

impl AlertUrls {
    pub fn new(endpoint_url: Url, json_report: &JsonReport) -> Self {
        let mut urls = Vec::new();

        for alert in &json_report.alerts {
            let alert_url =
                Self::to_url(endpoint_url.clone(), &json_report.project.slug, alert.uuid);
            urls.push((alert.benchmark.name.to_string(), alert_url));
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
