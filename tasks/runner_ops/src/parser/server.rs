use std::collections::BTreeMap;

use bencher_json::{RunnerResourceId, Secret, UpdateChannel};
use camino::Utf8PathBuf;
use serde::Deserialize;

use super::scrub_day::ScrubDay;

pub type Servers = BTreeMap<RunnerResourceId, Server>;

#[derive(Debug, Deserialize)]
#[expect(
    clippy::struct_field_names,
    reason = "server field mirrors config key name"
)]
pub struct Server {
    pub server: String,
    pub ssh: Option<Utf8PathBuf>,
    pub user: Option<String>,
    pub key: Option<Secret>,
    pub host: Option<url::Url>,
    pub update_channel: Option<UpdateChannel>,
    pub scrub_day: Option<ScrubDay>,
}

pub fn load_server(runner: &RunnerResourceId) -> anyhow::Result<Option<Server>> {
    Ok(load_servers()?.remove(runner))
}

pub fn load_servers() -> anyhow::Result<Servers> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/runners.json");
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Ok(Servers::new());
    };
    Ok(serde_json::from_str(&contents)?)
}
