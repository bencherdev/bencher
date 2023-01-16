use mail_send::{mail_builder::MessageBuilder, Transport};

use crate::ApiError;

use super::body::FmtBody;
use super::Message;

pub struct Email {
    pub hostname: String,
    pub username: String,
    pub secret: String,
    pub from_name: Option<String>,
    pub from_email: String,
}

impl Email {
    pub async fn send(&self, message: Message) {
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
        let transport = Transport::new(self.hostname.clone())
            .credentials(self.username.clone(), self.secret.clone());

        tokio::spawn(async move {
            async fn send<'x>(
                transport: Transport<'x>,
                message_builder: MessageBuilder<'x>,
            ) -> Result<(), ApiError> {
                transport
                    .connect_tls()
                    .await
                    .map_err(ApiError::MailTls)?
                    .send(message_builder)
                    .await
                    .map_err(ApiError::MailSend)
            }

            match send(transport, message_builder).await {
                Ok(_) => tracing::trace!("Email sent email from {from_email} to {to_email}"),
                Err(e) => {
                    tracing::error!("Failed to send email from {from_email} to {to_email}: {e}");
                },
            }
        });
    }
}
