use derive_more::Display;

use crate::WordStr;

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
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Ping => "ping",
            Self::Version => "version",
            Self::Restart => "restart",
            Self::Config => "config",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Ping => "pings",
            Self::Version => "versions",
            Self::Restart => "restarts",
            Self::Config => "configs",
        }
    }
}
