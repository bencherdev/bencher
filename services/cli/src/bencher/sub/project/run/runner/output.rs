pub struct Output {
    pub result: String,
}

impl Output {
    pub fn as_str(&self) -> &str {
        &self.result
    }
}
