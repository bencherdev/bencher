use std::collections::BTreeMap;

use bencher_json::{
    project::JsonVisibility, JsonPerfQuery, JsonProject, JsonReport, ResourceId, Slug,
};
use url::Url;
use uuid::Uuid;

use crate::{bencher::backend::Backend, CliError};

pub struct BenchmarkUrls(pub BTreeMap<String, Url>);

impl BenchmarkUrls {
    pub async fn new(
        backend: &Backend,
        project: &ResourceId,
        json_report: &JsonReport,
    ) -> Result<Self, CliError> {
        let json_value = backend.get_quiet("/v0/server/config/endpoint").await?;
        let endpoint_url: Url = serde_json::from_value(json_value)?;

        let json_value = backend
            .get_quiet(&format!("/v0/projects/{project}"))
            .await?;
        let json_project: JsonProject = serde_json::from_value(json_value)?;

        let benchmark_url = BenchmarkUrl::new(
            endpoint_url,
            json_project,
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

        Ok(Self(urls))
    }
}

struct BenchmarkUrl {
    endpoint: Url,
    project_visibility: JsonVisibility,
    project_slug: Slug,
    branch: Uuid,
    testbed: Uuid,
}

impl BenchmarkUrl {
    fn new(endpoint: Url, project: JsonProject, branch: Uuid, testbed: Uuid) -> Self {
        Self {
            endpoint,
            project_visibility: project.visibility,
            project_slug: project.slug,
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
        let path = match self.project_visibility {
            JsonVisibility::Public => format!("/perf/{}", self.project_slug),
            #[cfg(feature = "plus")]
            JsonVisibility::Private => format!("/console/projects/{}/perf", self.project_slug),
        };
        url.set_path(&path);
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
