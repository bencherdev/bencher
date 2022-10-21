use std::convert::TryFrom;

use bencher_json::{
    config::{JsonServer, JsonSmtp, JsonTls},
    JsonConfig,
};
use bencher_rbac::init_rbac;
use dropshot::{ConfigDropshot, ConfigTls, HttpServer};
use tokio::sync::Mutex;

use crate::{
    util::{
        context::{Email, Messenger},
        ApiContext, Context,
    },
    ApiError,
};

use super::Config;

impl TryFrom<Config> for HttpServer<Context> {
    type Error = ApiError;

    fn try_from(config: Config) -> Result<Self, Self::Error> {
        let Config(JsonConfig {
            endpoint,
            secret_key,
            server,
            database,
            smtp,
        }) = config;

        let JsonServer {
            bind_address,
            request_body_max_bytes,
            tls,
        } = server;
        let config_dropshot = ConfigDropshot {
            bind_address,
            request_body_max_bytes,
            tls: tls.map(
                |JsonTls {
                     cert_file,
                     key_file,
                 }| ConfigTls {
                    cert_file,
                    key_file,
                },
            ),
        };

        let private = Mutex::new(ApiContext {
            endpoint,
            secret_key: secret_key
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
                .into(),
            rbac: init_rbac().map_err(ApiError::Polar)?.into(),
            messenger: smtp.map(
                |JsonSmtp {
                     hostname,
                     username,
                     secret,
                     from_name,
                     from_email,
                 }| {
                    Messenger::Email(Email {
                        hostname,
                        username,
                        secret,
                        from_name: Some(from_name),
                        from_email,
                    })
                },
            ),
            database,
        });

        Ok(
            dropshot::HttpServerStarter::new(&config_dropshot, api, private, &log)
                .map_err(ApiError::CreateServer)?
                .start(),
        )
    }
}
