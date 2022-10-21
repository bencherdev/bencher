use derive_more::Display;

use crate::WordStr;

pub mod config;
pub mod restart;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    Restart,
    Config,
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Restart => "restart",
            Self::Config => "config",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Restart => "restarts",
            Self::Config => "configs",
        }
    }
}
