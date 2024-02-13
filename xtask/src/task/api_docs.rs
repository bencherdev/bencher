use core::fmt;
use std::fs;

use camino::Utf8PathBuf;
use openapiv3::{Parameter, ReferenceOr};

use crate::parser::TaskApiDocs;

const SWAGGER_PATH: &str = "services/api/swagger.json";
const API_DOCS_PATH: &str = "services/console/src/chunks/api";

#[derive(Debug)]
pub struct ApiDocs {
    spec: Utf8PathBuf,
    path: Utf8PathBuf,
}

impl TryFrom<TaskApiDocs> for ApiDocs {
    type Error = anyhow::Error;

    fn try_from(api_docs: TaskApiDocs) -> Result<Self, Self::Error> {
        let TaskApiDocs { spec, path } = api_docs;
        Ok(Self {
            spec: spec.unwrap_or_else(|| Utf8PathBuf::from(SWAGGER_PATH)),
            path: path.unwrap_or_else(|| Utf8PathBuf::from(API_DOCS_PATH)),
        })
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum Method {
    Get,
    Post,
    Patch,
    Put,
    Delete,
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Method::Get => write!(f, "GET"),
            Method::Post => write!(f, "POST"),
            Method::Patch => write!(f, "PATCH"),
            Method::Put => write!(f, "PUT"),
            Method::Delete => write!(f, "DELETE"),
        }
    }
}

impl Method {
    fn color(self) -> &'static str {
        match self {
            Method::Get => "is-info",
            Method::Post => "is-success",
            Method::Patch => "is-warning",
            Method::Put => "is-warning",
            Method::Delete => "is-danger",
        }
    }
}

const PATHS: &[(&str, Method, &str)] = &[("/v0/organizations", Method::Get, "organization list")];

impl ApiDocs {
    pub fn exec(&self) -> anyhow::Result<()> {
        let file = fs::File::open(&self.spec)?;
        let spec: openapiv3::OpenAPI = serde_json::from_reader(file)?;
        fs::write("xtask/spec.rs", format!("{spec:#?}"))?;

        for (path, method, cli_cmd) in PATHS {
            let path_spec = spec
                .paths
                .paths
                .get(*path)
                .ok_or_else(|| anyhow::anyhow!("Path not found in spec: {path}"))?;

            let path_spec = path_spec
                .as_item()
                .ok_or_else(|| anyhow::anyhow!("Path not found in spec: {path}"))?;

            let id = slug::slugify(format!("{method} {path}"));
            let spec = match method {
                Method::Get => path_spec.get.as_ref(),
                Method::Post => path_spec.post.as_ref(),
                Method::Patch => path_spec.patch.as_ref(),
                Method::Put => path_spec.put.as_ref(),
                Method::Delete => path_spec.delete.as_ref(),
            }
            .ok_or_else(|| anyhow::anyhow!("Method not found in {path} spec: {method}"))?;
            let method_color = method.color();

            let summary = spec
                .summary
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Summary not found in {path} spec: {method}"))?;
            let description = spec
                .description
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Description not found in {path} spec: {method}"))?;

            let relative_path = path.strip_prefix('/').unwrap_or(path);
            let full_path = self.path.join(relative_path);
            fs::create_dir_all(&full_path)?;

            let query_params = query_params(path, *method, &spec.parameters)?;

            fs::write(
                full_path.join(format!("{method}.mdx")),
                format!(
                    r##"
<h2 id="{id}" class="title is-4">{summary}<a href="#{id}"><i class="fas fa-link" aria-hidden="true" style="padding-left: 0.3em; color: #fdb07e;"></i></a></h2>
<hr />
<div class="columns">
<div class="column">
<p>{description}</p>
{query_params}
</div>
<div class="column">
<div class="level">
<div class="level-left">
    <div class="level-item">
        <span class="tag {method_color} is-medium is-rounded">{method}</span>
    </div>
    <div class="level-item">
        <p>{path}</p>
    </div>
</div>
<div class="level-right">
    <div class="level-item">
        <a class="button" href="/download/openapi.json">View OpenAPI Spec</a>
    </div>
</div>
</div>
<div class="card"><header class="card-header"><p class="card-header-title">Bencher CLI</p></header><pre><code>bencher {cli_cmd}</code></pre></div>
</div>
</div>
                    "##
                ),
            )?;
        }

        Ok(())
    }
}

fn query_params(
    path: &str,
    method: Method,
    parameters: &[ReferenceOr<Parameter>],
) -> anyhow::Result<String> {
    if parameters.is_empty() {
        return Ok(String::new());
    }

    let mut query_params = r#"<h3 class="title is-5">Query Parameters</h3>"#.to_owned();
    for param in parameters {
        let param = param.as_item().ok_or_else(|| {
            anyhow::anyhow!("Query parameter {param:?} not found in {path} spec: {method}")
        })?;
        if let Parameter::Query { parameter_data, .. } = param {
            let description = parameter_data.description.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "Description not found for query parameter {param:?} in {path} spec: {method}"
                )
            })?;

            query_params.push_str(&format!(
                r#"
<hr />
<p><strong>{name}</strong></p>
<p>{description}</p>
                "#,
                name = parameter_data.name,
            ));
        }
    }

    Ok(query_params)
}
