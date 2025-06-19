use std::{fmt, sync::Arc};

use bencher_json::Secret;
use bencher_json::system::config::JsonSmtp;
use mail_send::{SmtpClientBuilder, mail_builder::MessageBuilder};
use slog::{Logger, error, trace};
use tokio::sync::Mutex;

use super::Message;
use super::body::FmtBody as _;

pub const DEFAULT_SMTP_PORT: u16 = 587;

#[derive(Debug, Clone)]
pub struct Email {
    client: Arc<Mutex<Client>>,
    from_name: Option<String>,
    #[expect(clippy::struct_field_names, reason = "from_email is more descriptive")]
    from_email: String,
}

impl From<JsonSmtp> for Email {
    fn from(smtp: JsonSmtp) -> Self {
        let JsonSmtp {
            hostname,
            port,
            insecure_host,
            starttls,
            username,
            secret,
            from_name,
            from_email,
        } = smtp;
        let client_builder = ClientBuilder {
            hostname: hostname.into(),
            port: port.unwrap_or(DEFAULT_SMTP_PORT),
            insecure_host: insecure_host.unwrap_or_default(),
            starttls: starttls.unwrap_or(true),
            username: username.into(),
            secret,
        };
        Self {
            client: Arc::new(Mutex::new(client_builder.into())),
            from_name: Some(from_name.into()),
            from_email: from_email.into(),
        }
    }
}

impl Email {
    pub fn send(&self, log: &Logger, message: Message) {
        let mut message_builder = MessageBuilder::new();

        message_builder = if let Some(name) = self.from_name.clone() {
            message_builder.from((name, self.from_email.clone()))
        } else {
            message_builder.from(self.from_email.clone())
        };

        let from_email = self.from_email.clone();
        let to_email = message.to_email.clone();
        message_builder = if let Some(name) = message.to_name {
            message_builder.to((name, message.to_email))
        } else {
            message_builder.to(message.to_email)
        };

        if let Some(subject) = message.subject {
            message_builder = message_builder.subject(subject);
        }

        if let Some(body) = message.body {
            slog::debug!(log, "Setting email body: {body:?}");
            message_builder = message_builder
                .text_body(body.text())
                .html_body(body.html(log));
        }

        slog::debug!(log, "Spawning email send task");
        let send_log = log.clone();
        let send_client = self.client.clone();
        tokio::spawn(async move {
            let mut client = send_client.lock().await;
            match client.send(&send_log, message_builder).await {
                Ok(()) => trace!(send_log, "Email sent email from {from_email} to {to_email}"),
                Err(err) => {
                    error!(
                        send_log,
                        "Failed to send email from {from_email} to {to_email}: {err}"
                    );
                    #[cfg(feature = "sentry")]
                    sentry::capture_error(&err);
                },
            }
        });
    }
}

#[derive(Debug, Clone)]
struct ClientBuilder {
    hostname: String,
    port: u16,
    insecure_host: bool,
    starttls: bool,
    username: String,
    secret: Secret,
}

impl ClientBuilder {
    // Connect to an SMTP relay server and authenticate using the provided credentials.
    fn build(&self) -> SmtpClientBuilder<String> {
        let mut builder = SmtpClientBuilder::new(self.hostname.clone(), self.port);

        if self.insecure_host {
            builder = builder.allow_invalid_certs();
        }

        builder
            .implicit_tls(!self.starttls)
            .credentials((self.username.clone(), String::from(self.secret.clone())))
    }
}

struct Client {
    builder: ClientBuilder,
    handle: Option<mail_send::SmtpClient<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("client_builder", &self.builder)
            .field(
                "handle",
                &if self.handle.is_some() {
                    "Connected"
                } else {
                    "Disconnected"
                },
            )
            .finish()
    }
}

impl From<ClientBuilder> for Client {
    fn from(builder: ClientBuilder) -> Self {
        Self {
            builder,
            handle: None,
        }
    }
}

impl Client {
    async fn send(
        &mut self,
        log: &Logger,
        message_builder: MessageBuilder<'_>,
    ) -> Result<(), mail_send::Error> {
        // If there isn't a client or if the client is no longer connected, create a new one.
        let client = if let Some(client) = self.handle.as_mut() {
            if client.noop().await.is_ok() {
                client
            } else {
                slog::debug!(log, "Refreshing client");
                let new_client = self.builder.build().connect().await?;
                self.handle.insert(new_client)
            }
        } else {
            slog::debug!(log, "Creating new client");
            let new_client = self.builder.build().connect().await?;
            self.handle.insert(new_client)
        };

        client.send(message_builder).await
    }
}
