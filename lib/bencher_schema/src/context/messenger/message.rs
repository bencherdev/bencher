use std::fmt;

use super::body::{Body, FmtBody};

pub struct Message {
    pub to_name: Option<String>,
    pub to_email: String,
    pub subject: Option<String>,
    pub body: Option<Body>,
}

// Never prints the subject or the body, either of which may carry a code or a link.
impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Message")
            .field("to_email", &self.to_email)
            .finish_non_exhaustive()
    }
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
            self.body.as_ref().map(FmtBody::text).unwrap_or_default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Body, Message};
    use crate::context::SignInBody;

    #[test]
    fn debug_leaves_out_the_subject_and_the_body() {
        let message = Message {
            to_name: Some("Ada".to_owned()),
            to_email: "ada@example.com".to_owned(),
            subject: Some("Your Bencher code is 123456".to_owned()),
            body: Some(Body::SignIn(SignInBody::Code {
                sign_up: false,
                code: "123456".parse().unwrap(),
                link: "https://bencher.dev/auth/confirm#secret".parse().unwrap(),
            })),
        };
        let debug = format!("{message:?}");
        assert!(debug.contains("ada@example.com"), "{debug}");
        for secret in ["123456", "/auth/confirm", "secret"] {
            assert!(!debug.contains(secret), "{debug}");
        }
    }
}
