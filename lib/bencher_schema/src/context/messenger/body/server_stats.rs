#![cfg(feature = "plus")]

use slog::Logger;

use super::FmtBody;

#[derive(Debug)]
pub struct ServerStatsBody {
    pub server_stats: String,
}

impl FmtBody for ServerStatsBody {
    fn text(&self) -> String {
        let Self { server_stats } = self;
        server_stats.clone()
    }

    fn html(&self, _log: &Logger) -> String {
        self.text()
    }
}
