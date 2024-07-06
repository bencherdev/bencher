mod body;
mod email;
mod message;

use bencher_json::system::config::JsonSmtp;
#[cfg(feature = "plus")]
pub use body::ServerStatsBody;
pub use body::{Body, ButtonBody, NewUserBody};
pub use email::Email;
pub use message::Message;
use slog::{info, Logger};

#[derive(Debug, Clone, Default)]
pub enum Messenger {
    #[default]
    StdOut,
    Email(Email),
}

impl From<Option<JsonSmtp>> for Messenger {
    fn from(smtp: Option<JsonSmtp>) -> Self {
        smtp.map(Into::into).map(Self::Email).unwrap_or_default()
    }
}

impl Messenger {
    pub fn send(&self, log: &Logger, message: Message) {
        slog::debug!(log, "Sending message: {message:?}");
        match self {
            Self::StdOut => info!(log, "{message}"),
            Self::Email(email) => email.send(log, message),
        }
    }
}
