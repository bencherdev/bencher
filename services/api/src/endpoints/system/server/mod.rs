use derive_more::Display;

use crate::WordStr;

pub mod backup;
pub mod config;
pub mod endpoint;
pub mod ping;
pub mod restart;
pub mod version;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    Backup,
    Config,
    Endpoint,
    Ping,
    Restart,
    Version,
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Backup => "backup",
            Self::Config => "config",
            Self::Endpoint => "endpoint",
            Self::Ping => "ping",
            Self::Restart => "restart",
            Self::Version => "version",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Backup => "backups",
            Self::Config => "configs",
            Self::Endpoint => "endpoints",
            Self::Ping => "pings",
            Self::Restart => "restarts",
            Self::Version => "versions",
        }
    }
}
