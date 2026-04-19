use std::collections::BTreeMap;

use bencher_json::{RunnerResourceId, Secret};
use camino::Utf8PathBuf;
use serde::Deserialize;

type Servers = BTreeMap<RunnerResourceId, Server>;

#[derive(Debug, Deserialize)]
#[expect(clippy::struct_field_names)]
pub struct Server {
    pub server: String,
    pub ssh: Option<Utf8PathBuf>,
    pub user: Option<String>,
    pub key: Option<Secret>,
    pub host: Option<url::Url>,
}

pub fn load_server(runner: &RunnerResourceId) -> anyhow::Result<Option<Server>> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/runners.json");
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    let mut servers: Servers = serde_json::from_str(&contents)?;
    Ok(servers.remove(runner))
}
