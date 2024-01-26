use crate::parser::TaskIndexUpdate;

use super::engine::SearchEngine;

#[derive(Debug)]
pub struct Update {
    engine: Option<SearchEngine>,
    url: Vec<url::Url>,
}

impl TryFrom<TaskIndexUpdate> for Update {
    type Error = anyhow::Error;

    fn try_from(update: TaskIndexUpdate) -> Result<Self, Self::Error> {
        let TaskIndexUpdate { engine, url } = update;
        if url.is_empty() {
            anyhow::bail!("URL is empty");
        }
        Ok(Self {
            engine: engine.map(Into::into),
            url,
        })
    }
}

impl Update {
    pub async fn exec(&self) -> anyhow::Result<()> {
        for url in &self.url {
            if let Some(engine) = self.engine {
                engine.update(url.clone()).await?;
            } else {
                for engine in SearchEngine::all() {
                    engine.update(url.clone()).await?;
                }
            }
        }
        Ok(())
    }
}
