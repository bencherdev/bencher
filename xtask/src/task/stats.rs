#![cfg(feature = "plus")]

use bencher_json::{JsonServerStats, PROD_BENCHER_API_URL_STR};

use crate::parser::CliStats;

#[derive(Debug)]
pub struct Stats {
    stats: JsonServerStats,
}

impl TryFrom<CliStats> for Stats {
    type Error = anyhow::Error;

    fn try_from(stats: CliStats) -> Result<Self, Self::Error> {
        let CliStats { stats } = stats;
        Ok(Self {
            stats: serde_json::from_str(&stats)?,
        })
    }
}

impl Stats {
    pub async fn exec(&self) -> anyhow::Result<()> {
        let json_stats_str = serde_json::to_string(&self.stats)?;
        let client = reqwest::Client::new();
        let _resp = client
            .post(format!("{PROD_BENCHER_API_URL_STR}/v0/server/stats"))
            .body(json_stats_str)
            .send()
            .await?;
        Ok(())
    }
}
