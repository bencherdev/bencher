//! Litestream configuration types for Bencher Plus.
//!
//! The types now live in the [`bencher_litestream`] crate so they can be shared by
//! path. They are re-exported here to keep the historical
//! `bencher_json::system::config::...` paths (and the `OpenAPI` schema) stable.

pub use bencher_litestream::{JsonCheckpoint, JsonLitestream, JsonReplica, LitestreamLevel};

use crate::system::config::LogLevel;

impl From<LogLevel> for LitestreamLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace | LogLevel::Debug => Self::Debug,
            LogLevel::Info => Self::Info,
            LogLevel::Warn => Self::Warn,
            LogLevel::Error | LogLevel::Critical => Self::Error,
        }
    }
}
