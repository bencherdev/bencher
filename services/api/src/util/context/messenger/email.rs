use mail_send::{mail_builder::MessageBuilder, Transport};

use crate::ApiError;

use super::Message;

pub struct Email {
    pub hostname: String,
    pub username: String,
    pub secret: String,
    pub from_name: Option<String>,
    pub from_email: String,
}

impl Email {
    pub async fn send(&self, message: Message) -> Result<(), ApiError> {
        let mut message_builder = MessageBuilder::new();

        message_builder = if let Some(name) = &self.from_name {
            message_builder.from((name.as_str(), self.from_email.as_str()))
        } else {
            message_builder.from(self.from_email.as_str())
        };

        message_builder = if let Some(name) = message.to_name {
            message_builder.to((name, message.to_email))
        } else {
            message_builder.to(message.to_email)
        };

        if let Some(subject) = message.subject {
            message_builder = message_builder.subject(subject);
        }

        if let Some(body) = message.html_body {
            message_builder = message_builder.html_body(body);
        }

        if let Some(body) = message.text_body {
            message_builder = message_builder.text_body(body);
        }

        // Connect to an SMTP relay server over TLS and
        // authenticate using the provided credentials.
        Transport::new(&self.hostname)
            .credentials(&self.username, &self.secret)
            .connect_tls()
            .await
            .map_err(ApiError::MailSend)?
            .send(message_builder)
            .await
            .map_err(ApiError::MailSend)
    }
}
