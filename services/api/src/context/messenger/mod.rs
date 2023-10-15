mod body;
mod email;
mod message;

pub use body::{Body, ButtonBody, NewUserBody};
pub use email::Email;
pub use message::Message;
use slog::{info, Logger};

pub enum Messenger {
    StdOut,
    Email(Email),
}

#[derive(Debug, thiserror::Error)]
pub enum MessengerError {
    #[error("Failed to inline CSS: {0}")]
    CssInline(#[from] css_inline::InlineError),
}

impl Messenger {
    pub fn send(&self, log: &Logger, message: Message) {
        match self {
            Self::StdOut => info!(log, "{message}"),
            Self::Email(email) => email.send(log, message),
        }
    }
}
