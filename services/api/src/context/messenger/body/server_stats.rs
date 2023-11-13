#![cfg(feature = "plus")]

use super::FmtBody;

pub struct ServerStatsBody {
    pub server_stats: String,
}

impl FmtBody for ServerStatsBody {
    fn text(&self) -> String {
        let Self { server_stats } = self;
        server_stats.clone()
    }

    fn html(&self) -> String {
        self.text()
    }
}
