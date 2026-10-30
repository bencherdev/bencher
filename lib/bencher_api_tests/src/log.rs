use std::{
    fmt::{self, Write as _},
    sync::{Arc, Mutex, PoisonError},
};

use bencher_json::{LinkSecret, SignInCode};
use slog::{Drain, KV as _, OwnedKVList, Record};

const SIGN_IN_SUBJECT: &str = "Subject: Your Bencher code is ";
const SIGN_IN_LINK: &str = "/auth/confirm#";

/// Keeps every line the server logs, its message and every key-value pair, as text.
#[derive(Clone, Default)]
pub struct LogCapture(Arc<Mutex<Vec<String>>>);

impl LogCapture {
    pub fn logger(&self) -> slog::Logger {
        slog::Logger::root(self.clone(), slog::o!())
    }

    pub fn lines(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The last email the server logged in place of sending it that has `button`.
    #[expect(clippy::expect_used, reason = "test helper reading a logged email")]
    pub fn email(&self, button: &str) -> String {
        let prefix = format!("\n{button}: ");
        self.lines()
            .into_iter()
            .rev()
            .find(|line| line.contains(&prefix))
            .expect("No email was logged")
    }

    /// The link behind `button` in the last email the server logged in place of sending it.
    #[expect(clippy::expect_used, reason = "test helper reading a logged email")]
    pub fn email_link(&self, button: &str) -> url::Url {
        let email = self.email(button);
        let prefix = format!("\n{button}: ");
        let (_, rest) = email.split_once(&prefix).expect("No link in the email");
        let href = rest.split_once('\n').map_or(rest, |(href, _)| href);
        href.parse().expect("Invalid link in the email")
    }
}

impl LogCapture {
    /// Every sign-in email the server logged in place of sending it, with a code or without one.
    pub fn sign_in_emails(&self) -> Vec<String> {
        self.lines()
            .into_iter()
            .filter(|line| line.contains("\nSubject: Your Bencher "))
            .collect()
    }

    /// The code and the link's secret in the last sign-in email the server logged.
    #[expect(clippy::expect_used, reason = "test helper reading a logged email")]
    pub fn sign_in(&self) -> (SignInCode, LinkSecret) {
        let email = self
            .lines()
            .into_iter()
            .rev()
            .find(|line| line.contains(SIGN_IN_SUBJECT))
            .expect("No sign-in email was logged");
        let (_, rest) = email
            .split_once(SIGN_IN_SUBJECT)
            .expect("No code in the email");
        let code = rest
            .chars()
            .take(6)
            .collect::<String>()
            .parse()
            .expect("Invalid code in the email");
        let (_, rest) = email
            .split_once(SIGN_IN_LINK)
            .expect("No link in the email");
        let secret = rest
            .split_once('\n')
            .map_or(rest, |(secret, _)| secret)
            .parse()
            .expect("Invalid link in the email");
        (code, secret)
    }
}

impl Drain for LogCapture {
    type Ok = ();
    type Err = slog::Never;

    fn log(&self, record: &Record<'_>, values: &OwnedKVList) -> Result<(), slog::Never> {
        let mut line = Line(record.msg().to_string());
        if let Err(e) = record
            .kv()
            .serialize(record, &mut line)
            .and_then(|()| values.serialize(record, &mut line))
        {
            line.0.push_str(" unformatted: ");
            line.0.push_str(&e.to_string());
        }
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(line.0);
        Ok(())
    }
}

struct Line(String);

impl slog::Serializer for Line {
    fn emit_arguments(&mut self, key: slog::Key, val: &fmt::Arguments<'_>) -> slog::Result {
        write!(self.0, " {key}={val}").map_err(slog::Error::Fmt)
    }
}
