use core::fmt;
use std::fs;

use camino::Utf8PathBuf;

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

const PATHS: &[(&str, Method)] = &[("/v0/organizations", Method::Get)];

impl ApiDocs {
    pub fn exec(&self) -> anyhow::Result<()> {
        let file = fs::File::open(&self.spec)?;
        let spec: openapiv3::OpenAPI = serde_json::from_reader(file)?;
        fs::write("xtask/spec.rs", format!("{spec:#?}"))?;

        for (path, method) in PATHS {
            let path_spec = spec
                .paths
                .paths
                .get(*path)
                .ok_or_else(|| anyhow::anyhow!("Path not found in spec: {path}"))?;

            let path_spec = path_spec
                .as_item()
                .ok_or_else(|| anyhow::anyhow!("Path not found in spec: {path}"))?;
            match method {
                Method::Get => {
                    let get_spec = path_spec.get.as_ref().ok_or_else(|| {
                        anyhow::anyhow!("Method not found in {path} spec: {method}")
                    })?;

                    let summary = get_spec.summary.as_ref().ok_or_else(|| {
                        anyhow::anyhow!("Summary not found in {path} spec: {method}")
                    })?;
                    let description = get_spec.description.as_ref().ok_or_else(|| {
                        anyhow::anyhow!("Description not found in {path} spec: {method}")
                    })?;

                    let relative_path = path.strip_prefix('/').unwrap_or(path);
                    let full_path = self.path.join(relative_path);
                    println!("{}", full_path);
                    fs::create_dir_all(&full_path)?;
                    fs::write(
                        full_path.join(format!("{method}.mdx")),
                        format!("{summary}\n\n{description}"),
                    )?;
                },
                Method::Post => {
                    let post_spec = path_spec.post.as_ref().ok_or_else(|| {
                        anyhow::anyhow!("Method not found in {path} spec: {method}")
                    })?;
                },
                Method::Patch => {
                    let path_spec = path_spec.patch.as_ref().ok_or_else(|| {
                        anyhow::anyhow!("Method not found in {path} spec: {method}")
                    })?;
                },
                Method::Put => {
                    let put_spec = path_spec
                        .put
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Method not found in spec: {method}"))?;
                },
                Method::Delete => {
                    let delete_spec = path_spec.delete.as_ref().ok_or_else(|| {
                        anyhow::anyhow!("Method not found in {path} spec: {method}")
                    })?;
                },
            }
        }

        Ok(())
    }
}
