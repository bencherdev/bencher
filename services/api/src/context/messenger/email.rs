use std::{fmt, sync::Arc};

use bencher_json::system::config::JsonSmtp;
use bencher_json::Secret;
use mail_send::{mail_builder::MessageBuilder, SmtpClientBuilder};
use slog::{error, trace, Logger};
use tokio::sync::RwLock;

use super::body::FmtBody;
use super::Message;
use crate::config::DEFAULT_SMTP_PORT;

#[derive(Debug, Clone)]
pub struct Email {
    from_name: Option<String>,
    from_email: String,
    client: Arc<RwLock<Client>>,
}

impl From<JsonSmtp> for Email {
    fn from(smtp: JsonSmtp) -> Self {
        let JsonSmtp {
            hostname,
            port,
            starttls,
            username,
            secret,
            from_name,
            from_email,
        } = smtp;
        Self {
            from_name: Some(from_name.into()),
            from_email: from_email.into(),
            client: Arc::new(RwLock::new(Client::new(
                hostname.into(),
                port.unwrap_or(DEFAULT_SMTP_PORT),
                starttls.unwrap_or(true),
                username.into(),
                secret,
            ))),
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
            let mut client = send_client.write().await;
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

struct Client {
    hostname: String,
    port: u16,
    starttls: bool,
    username: String,
    secret: Secret,
    inner: Option<mail_send::SmtpClient<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("hostname", &self.hostname)
            .field("port", &self.port)
            .field("starttls", &self.starttls)
            .field("username", &self.username)
            .field("secret", &"************")
            .field("inner", &if self.inner.is_some() { "Some" } else { "None" })
            .finish()
    }
}

impl Client {
    fn new(hostname: String, port: u16, starttls: bool, username: String, secret: Secret) -> Self {
        Self {
            hostname,
            port,
            username,
            secret,
            starttls,
            inner: None,
        }
    }

    // Connect to an SMTP relay server over TLS and
    // authenticate using the provided credentials.
    fn new_client_builder(&self) -> SmtpClientBuilder<String> {
        SmtpClientBuilder::new(self.hostname.clone(), self.port)
            .credentials((self.username.clone(), String::from(self.secret.clone())))
            .implicit_tls(!self.starttls)
    }

    async fn send(
        &mut self,
        log: &Logger,
        message_builder: MessageBuilder<'_>,
    ) -> Result<(), mail_send::Error> {
        // If there isn't a client or if the client is no longer connected, create a new one.
        let client = if let Some(client) = self.inner.as_mut() {
            if client.noop().await.is_ok() {
                client
            } else {
                slog::debug!(log, "Refreshing client");
                let new_client = self.new_client_builder().connect().await?;
                self.inner.insert(new_client)
            }
        } else {
            slog::debug!(log, "Creating new client");
            let new_client = self.new_client_builder().connect().await?;
            self.inner.insert(new_client)
        };

        client.send(message_builder).await
    }
}
