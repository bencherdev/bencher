use serde::Serialize;
use url::Url;

use crate::parser::CliNotify;

const NTFY_URL: &str = "https://ntfy.sh";
const NTFY_TOPIC: &str = "bencherdev";

#[derive(Debug, Serialize)]
pub struct Notify {
    topic: String,
    message: String,
    title: Option<String>,
    tags: Option<String>,
    priority: Option<u8>,
    click: Option<Url>,
    attach: Option<Url>,
}

impl TryFrom<CliNotify> for Notify {
    type Error = anyhow::Error;

    fn try_from(stats: CliNotify) -> Result<Self, Self::Error> {
        let CliNotify {
            topic,
            message,
            title,
            tag,
            priority,
            click,
            attach,
        } = stats;
        Ok(Self {
            topic: topic.unwrap_or_else(|| NTFY_TOPIC.to_owned()),
            message,
            title,
            tags: tag,
            priority,
            click,
            attach,
        })
    }
}

impl Notify {
    pub async fn exec(&self) -> anyhow::Result<()> {
        let notify_json = serde_json::to_string(&self)?;
        let client = reqwest::Client::new();
        let _resp = client.post(NTFY_URL).body(notify_json).send().await?;
        Ok(())
    }
}
