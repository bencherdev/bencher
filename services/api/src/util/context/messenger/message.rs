pub struct Message {
    pub to_name: Option<String>,
    pub to_email: String,
    pub subject: Option<String>,
    pub html_body: Option<String>,
    pub text_body: Option<String>,
}
