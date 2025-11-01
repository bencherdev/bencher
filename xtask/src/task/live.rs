use bencher_client::BencherClient;
use bencher_json::JsonApiVersion;

use crate::parser::TaskLive;

#[derive(Debug)]
pub struct Live {
    client: BencherClient,
}

impl TryFrom<TaskLive> for Live {
    type Error = anyhow::Error;

    fn try_from(live: TaskLive) -> Result<Self, Self::Error> {
        let TaskLive { host } = live;
        let mut builder = BencherClient::builder();
        if let Some(host) = host {
            builder = builder.host(host);
        }
        let client = builder.build();
        Ok(Self { client })
    }
}

impl Live {
    pub async fn exec(&self) -> anyhow::Result<()> {
        let _json_api_version: JsonApiVersion = self
            .client
            .clone()
            .into_builder()
            .log(true)
            .build()
            .send_with(|client| async move { client.server_version_get().send().await })
            .await?;
        Ok(())
    }
}
