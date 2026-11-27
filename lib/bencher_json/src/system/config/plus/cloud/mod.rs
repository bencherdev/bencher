use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod billing;
pub mod index;
pub mod otel;
pub mod turnstile;

use billing::JsonBilling;
use index::JsonIndex;
use otel::JsonOtel;
use turnstile::JsonTurnstile;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCloud {
    pub billing: JsonBilling,
    pub license_pem: Secret,
    pub sentry: Option<Secret>,
    pub otel: Option<JsonOtel>,
    pub index: Option<JsonIndex>,
    pub turnstile: Option<JsonTurnstile>,
}

impl Sanitize for JsonCloud {
    fn sanitize(&mut self) {
        self.billing.sanitize();
        self.license_pem.sanitize();
        self.sentry.sanitize();
        self.index.sanitize();
        self.turnstile.sanitize();
    }
}

#[cfg(test)]
mod tests {
    use crate::system::config::JsonConfig;

    // Kills a config that no longer loads once it names a key the server dropped, such as
    // reCAPTCHA's, which an operator's config may still hold after an upgrade.
    #[test]
    fn recaptcha_is_ignored() {
        let config: JsonConfig = serde_json::from_str(
            r#"{
                "console": { "url": "https://bencher.dev" },
                "security": { "secret_key": "secret-key" },
                "server": { "bind_address": "0.0.0.0:61016", "request_body_max_bytes": 1048576 },
                "logging": { "name": "bencher_api", "log": { "stderr_terminal": { "level": "info" } } },
                "database": { "file": "/var/lib/bencher/data/bencher.db" },
                "plus": {
                    "cloud": {
                        "billing": {
                            "secret_key": "billing-secret",
                            "products": {
                                "pro": { "id": "prod_pro" },
                                "team": { "id": "prod_team" },
                                "enterprise": { "id": "prod_enterprise" },
                                "metrics": { "id": "prod_metrics" },
                                "bare_metal": { "id": "prod_bare_metal" }
                            }
                        },
                        "license_pem": "license-pem",
                        "recaptcha": { "secret": "recaptcha-secret", "min_score": 0.5 },
                        "turnstile": { "secret": "turnstile-secret" }
                    }
                }
            }"#,
        )
        .unwrap();
        let turnstile = config.plus.unwrap().cloud.unwrap().turnstile.unwrap();
        assert_eq!(turnstile.secret.as_ref(), "turnstile-secret");
    }
}
