#![cfg(feature = "plus")]

use bencher_json::JsonServerStats;

use super::FmtBody;

pub struct ServerStatsBody {
    pub server_stats: JsonServerStats,
}

impl FmtBody for ServerStatsBody {
    fn text(&self) -> String {
        let Self { server_stats } = self;
        format!("{server_stats:#?}")
    }

    fn html(&self) -> String {
        self.text()
    }
}
