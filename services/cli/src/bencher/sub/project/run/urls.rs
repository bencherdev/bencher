use std::collections::BTreeMap;

use bencher_json::{JsonPerfQuery, JsonReport, Slug};
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
                let metric_kind = result.metric_kind.slug.clone();
                for benchmark_metric in &result.benchmarks {
                    urls.insert(
                        benchmark_metric.name.to_string(),
                        benchmark_url.to_url(metric_kind.clone(), benchmark_metric.uuid),
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

    fn to_url(&self, metric_kind: Slug, benchmark: Uuid) -> Url {
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
                .to_query_string(&[
                    // ("tab", Some("reports".into()))
                ])
                .unwrap_or_default(),
        ));

        url
    }
}
