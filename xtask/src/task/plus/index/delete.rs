use crate::parser::TaskIndexDelete;

use super::engine::SearchEngine;

#[derive(Debug)]
pub struct Delete {
    engine: Option<SearchEngine>,
    url: Vec<url::Url>,
}

impl TryFrom<TaskIndexDelete> for Delete {
    type Error = anyhow::Error;

    fn try_from(delete: TaskIndexDelete) -> Result<Self, Self::Error> {
        let TaskIndexDelete { engine, url } = delete;
        if url.is_empty() {
            anyhow::bail!("URL is empty");
        }
        Ok(Self {
            engine: engine.map(Into::into),
            url,
        })
    }
}

impl Delete {
    pub async fn exec(&self) -> anyhow::Result<()> {
        for url in &self.url {
            if let Some(engine) = self.engine {
                engine.delete(url.clone()).await?;
            } else {
                for engine in SearchEngine::all() {
                    engine.delete(url.clone()).await?;
                }
            }
        }
        Ok(())
    }
}
