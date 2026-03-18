use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use serde::Deserialize;

type Servers = BTreeMap<String, Server>;

#[derive(Debug, Deserialize)]
#[expect(clippy::struct_field_names)]
pub struct Server {
    pub server: String,
    pub key: Option<Utf8PathBuf>,
    pub user: Option<String>,
    pub runner: Option<String>,
    pub token: Option<String>,
    pub host: Option<url::Url>,
}

pub fn load_server(name: &str) -> anyhow::Result<Server> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/servers.json");
    let contents =
        std::fs::read_to_string(path).map_err(|e| anyhow::anyhow!("Failed to read {path}: {e}"))?;
    let mut servers: Servers = serde_json::from_str(&contents)?;
    let key = servers
        .keys()
        .find(|k| k.eq_ignore_ascii_case(name))
        .cloned();
    key.and_then(|k| servers.remove(&k))
        .ok_or_else(|| anyhow::anyhow!("Server {name:?} not found in {path}"))
}
