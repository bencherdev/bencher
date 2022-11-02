mod body;
mod email;
mod message;

pub use body::{Body, ButtonBody};
pub use email::Email;
pub use message::Message;

pub enum Messenger {
    StdOut,
    Email(Email),
}

impl Messenger {
    pub async fn send(&self, message: Message) {
        match self {
            Self::StdOut => tracing::info!("{message}"),
            Self::Email(email) => email.send(message).await,
        }
    }
}
