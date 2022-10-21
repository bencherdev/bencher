use derive_more::Display;

use crate::WordStr;

pub mod config;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    Config,
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Config => "config",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Config => "configs",
        }
    }
}
