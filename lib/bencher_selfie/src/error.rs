use std::fmt;

use thiserror::Error;

#[derive(Debug)]
pub struct HeadlessChromeError(pub anyhow::Error);

impl fmt::Display for HeadlessChromeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

macro_rules! map_err {
    ($headless_chrome:expr) => {
        $headless_chrome
            .map_err(|e| SelfieError::HeadlessChrome(HeadlessChromeError(anyhow::anyhow!(e))))
    };
    ($headless_chrome:expr, $arg:ident) => {{
        $headless_chrome.map_err(|e| {
            SelfieError::HeadlessChrome(HeadlessChromeError(anyhow::anyhow!("{}: {}", $arg, e)))
        })
    }};
}

pub(crate) use map_err;

#[derive(Debug, Error)]
pub enum SelfieError {
    #[error("Failed to take screenshot: {0}")]
    HeadlessChrome(HeadlessChromeError),
    #[error("Failed to close tab for: {0}")]
    CloseTab(String),
}
