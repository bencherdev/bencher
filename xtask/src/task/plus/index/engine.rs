use std::str::FromStr;

use bencher_google::Google;

use crate::parser::TaskSearchEngine;

#[derive(Debug, Clone, Copy)]
pub enum SearchEngine {
    Google,
}

impl From<TaskSearchEngine> for SearchEngine {
    fn from(engine: TaskSearchEngine) -> Self {
        match engine {
            TaskSearchEngine::Google => Self::Google,
        }
    }
}

impl SearchEngine {
    pub fn all() -> Vec<Self> {
        vec![Self::Google]
    }

    pub async fn update(&self, url: url::Url) -> anyhow::Result<()> {
        match self {
            Self::Google => Self::google()?.url_updated(url).await.map_err(Into::into),
        }
    }

    pub async fn delete(&self, url: url::Url) -> anyhow::Result<()> {
        match self {
            Self::Google => Self::google()?.url_deleted(url).await.map_err(Into::into),
        }
    }

    fn google() -> anyhow::Result<Google> {
        let service_key = std::fs::read_to_string("./plus/bencher_google/google.json")?;
        Google::from_str(&service_key).map_err(Into::into)
    }
}
