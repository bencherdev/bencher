use mail_send::{mail_builder::MessageBuilder, Transport};

use crate::ApiError;

pub async fn send_email() -> Result<(), ApiError> {
    // Build a simple multipart message
    let message = MessageBuilder::new()
        .from(("John Doe", "john@example.com"))
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
