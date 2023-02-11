use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

#[cfg(feature = "plus")]
mod bencher;
mod database;
mod logging;
mod security;
mod server;
mod smtp;

#[cfg(feature = "plus")]
pub use bencher::JsonBencher;
pub use database::JsonDatabase;
pub use logging::{IfExists, JsonLogging, LogLevel, ServerLog};
pub use security::JsonSecurity;
pub use server::JsonServer;
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
    pub endpoint: Url,
    // TODO Remove deprecated secret_key
    pub secret_key: Option<Secret>,
    // TODO Remove deprecated secret_key
    pub security: Option<JsonSecurity>,
    pub server: JsonServer,
    pub logging: JsonLogging,
    pub database: JsonDatabase,
    pub smtp: Option<JsonSmtp>,
    #[cfg(feature = "plus")]
    pub bencher: Option<JsonBencher>,
}

impl Sanitize for JsonConfig {
    fn sanitize(&mut self) {
        self.secret_key.sanitize();
        self.database.sanitize();
        self.smtp.sanitize();
        #[cfg(feature = "plus")]
        self.bencher.sanitize();
    }
}
