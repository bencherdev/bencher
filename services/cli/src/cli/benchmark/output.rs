use chrono::{DateTime, Utc};

pub struct Output {
    pub start: Option<DateTime<Utc>>,
    pub result: String,
}

impl Output {
    pub fn as_str(&self) -> &str {
        &self.result
    }
}
