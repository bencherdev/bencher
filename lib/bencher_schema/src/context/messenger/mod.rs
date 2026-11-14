mod body;
mod email;
mod message;

use bencher_json::system::config::JsonSmtp;
pub use body::{Body, Button, ButtonBody, Clipboard, NewUserBody, SignInBody};
#[cfg(feature = "plus")]
pub use body::{RunnerStatusBody, ServerStatsBody};
pub use email::Email;
pub use message::Message;
use slog::{Logger, info};

#[cfg(feature = "plus")]
pub const GITHUB_OAUTH2: &str = "GitHub OAuth2";

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

    /// Send a message and wait until it is sent.
    /// With no SMTP server, the message is logged in its place, as every message is.
    /// With one, unlike [`Self::send`], no log line carries the message's body.
    pub async fn send_and_wait(
        &self,
        log: &Logger,
        message: Message,
    ) -> Result<(), mail_send::Error> {
        match self {
            Self::StdOut => {
                info!(log, "{message}");
                Ok(())
            },
            Self::Email(email) => email.send_and_wait(log, message).await,
        }
    }
}
