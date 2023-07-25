use std::collections::BTreeMap;

use bencher_json::{project::threshold::JsonThresholdStatistic, JsonPerfQuery, JsonReport, Slug};
use url::Url;
use uuid::Uuid;

pub struct BenchmarkUrls(pub BTreeMap<String, Url>);

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
