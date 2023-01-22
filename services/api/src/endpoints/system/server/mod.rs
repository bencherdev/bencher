use derive_more::Display;

use crate::WordStr;

pub mod backup;
pub mod config;
pub mod ping;
pub mod restart;
pub mod version;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    Ping,
    Version,
    Restart,
    Config,
    Backup,
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Ping => "ping",
            Self::Version => "version",
            Self::Restart => "restart",
            Self::Config => "config",
            Self::Backup => "backup",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Ping => "pings",
            Self::Version => "versions",
            Self::Restart => "restarts",
            Self::Config => "configs",
            Self::Backup => "backups",
        }
    }
}
