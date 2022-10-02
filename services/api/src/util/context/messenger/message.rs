use std::fmt;

pub struct Message {
    pub to_name: Option<String>,
    pub to_email: String,
    pub subject: Option<String>,
    pub html_body: Option<String>,
    pub text_body: Option<String>,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let to_email = format!("<{}>", self.to_email);
        write!(
            f,
            "To: {}\nSubject: {}\nBody: {}",
            self.to_name
                .clone()
                .map(|name| format!("{name} {to_email}>"))
                .unwrap_or(to_email),
            self.subject.clone().unwrap_or_default(),
            self.text_body
                .clone()
                .or(self.html_body.clone())
                .unwrap_or_default()
        )
    }
}
