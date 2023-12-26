use bencher_valid::Sanitize;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod console;
mod database;
mod logging;
#[cfg(feature = "plus")]
mod plus;
mod security;
mod server;
mod smtp;

pub use console::JsonConsole;
pub use database::{DataStore, JsonDatabase};
pub use logging::{IfExists, JsonLogging, LogLevel, ServerLog};
#[cfg(feature = "plus")]
pub use plus::{
    cloud::{JsonBilling, JsonCloud, JsonProduct, JsonProducts},
    stats::JsonStats,
    JsonPlus,
};
pub use security::JsonSecurity;
pub use server::{JsonServer, JsonTls};
pub use smtp::JsonSmtp;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateConfig {
    pub config: JsonConfig,
    pub delay: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConfig {
    pub console: JsonConsole,
    pub security: JsonSecurity,
    pub server: JsonServer,
    pub logging: JsonLogging,
    pub database: JsonDatabase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smtp: Option<JsonSmtp>,
    #[cfg(feature = "plus")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plus: Option<JsonPlus>,
}

impl Sanitize for JsonConfig {
    fn sanitize(&mut self) {
        self.security.sanitize();
        self.database.sanitize();
        self.smtp.sanitize();
        #[cfg(feature = "plus")]
        self.plus.sanitize();
    }
}
