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
        // Build a simple multipart message
        let mut message = MessageBuilder::new();
        message = if let Some(name) = &self.from_name {
            message.from((name.as_str(), self.from_email.as_str()))
        } else {
            message.from(self.from_email.as_str())
        };
        message = message
            .to(vec![
                ("Jane Doe", "jane@example.com"),
                ("James Smith", "james@test.com"),
            ])
            .subject("Hi!")
            .html_body("<h1>Hello, world!</h1>")
            .text_body("Hello world!");

        // Connect to an SMTP relay server over TLS and
        // authenticate using the provided credentials.
        Transport::new("smtp.gmail.com")
            .credentials("john", "p4ssw0rd")
            .connect_tls()
            .await
            .map_err(ApiError::MailSend)?
            .send(message)
            .await
            .map_err(ApiError::MailSend)
    }
}
