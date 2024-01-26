use std::str::FromStr;

use bencher_bing_index::BingIndex;
use bencher_google_index::GoogleIndex;
use url::Url;

use crate::parser::TaskSearchEngine;

#[derive(Debug, Clone, Copy)]
pub enum SearchEngine {
    Bing,
    Google,
}

impl From<TaskSearchEngine> for SearchEngine {
    fn from(engine: TaskSearchEngine) -> Self {
        match engine {
            TaskSearchEngine::Bing => Self::Bing,
            TaskSearchEngine::Google => Self::Google,
        }
    }
}

impl SearchEngine {
    pub fn all() -> Vec<Self> {
        vec![Self::Bing, Self::Google]
    }

    pub async fn update(&self, url: Url) -> anyhow::Result<()> {
        match self {
            Self::Bing => Self::bing()?.index_now(&url).await.map_err(Into::into),
            Self::Google => Self::google()?.url_updated(url).await.map_err(Into::into),
        }
    }

    pub async fn delete(&self, url: Url) -> anyhow::Result<()> {
        match self {
            Self::Bing => Self::bing()?.index_now(&url).await.map_err(Into::into),
            Self::Google => Self::google()?.url_deleted(url).await.map_err(Into::into),
        }
    }

    fn bing() -> anyhow::Result<BingIndex> {
        let key = "bbf489cef4c24ba2b54c991e8b97a1c8".to_owned();
        let key_location = Some(Url::parse("https://bencher.dev/indexnow.txt")?);
        Ok(BingIndex::new(key, key_location))
    }

    fn google() -> anyhow::Result<GoogleIndex> {
        let service_key = std::fs::read_to_string("./plus/bencher_google/google.json")?;
        GoogleIndex::from_str(&service_key).map_err(Into::into)
    }
}
