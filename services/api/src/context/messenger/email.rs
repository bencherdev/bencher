use bencher_json::Secret;
use mail_send::{mail_builder::MessageBuilder, SmtpClientBuilder};
use slog::Logger;

use crate::ApiError;

use super::body::FmtBody;
use super::Message;

pub struct Email {
    pub hostname: String,
    pub port: u16,
    pub starttls: bool,
    pub username: String,
    pub secret: Secret,
    pub from_name: Option<String>,
    pub from_email: String,
}

impl Email {
    pub async fn send(&self, _log: &Logger, message: Message) {
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
            message_builder = message_builder
                .text_body(body.text())
                .html_body(body.html());
        }

        // Connect to an SMTP relay server over TLS and
        // authenticate using the provided credentials.
        let client_builder = SmtpClientBuilder::new(self.hostname.clone(), self.port)
            .credentials((self.username.clone(), String::from(self.secret.clone())))
            .implicit_tls(!self.starttls);

        tokio::spawn(async move {
            async fn send(
                client_builder: SmtpClientBuilder<String>,
                message_builder: MessageBuilder<'_>,
            ) -> Result<(), ApiError> {
                client_builder
                    .connect()
                    .await
                    .map_err(ApiError::MailTls)?
                    .send(message_builder)
                    .await
                    .map_err(ApiError::MailSend)
            }

            match send(client_builder, message_builder).await {
                Ok(_) => tracing::trace!("Email sent email from {from_email} to {to_email}"),
                Err(e) => {
                    tracing::error!("Failed to send email from {from_email} to {to_email}: {e}");
                },
            }
        });
    }
}
