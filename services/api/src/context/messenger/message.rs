use std::fmt;

use super::body::{Body, FmtBody};

pub struct Message {
    pub to_name: Option<String>,
    pub to_email: String,
    pub subject: Option<String>,
    pub body: Option<Body>,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let to_email = format!("<{}>", self.to_email);
        write!(
            f,
            "\nTo: {}\nSubject: {}\nBody: {}",
            self.to_name
                .clone()
                .map(|name| format!("{name} {to_email}>"))
                .unwrap_or(to_email),
            self.subject.clone().unwrap_or_default(),
            self.body
                .as_ref()
                .map(|body| body.text())
                .unwrap_or_default(),
        )
    }
}
