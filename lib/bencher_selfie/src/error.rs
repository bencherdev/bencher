use std::fmt;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SelfieError {
    #[error("Failed to take screenshot: {0}")]
    Chrome(#[from] bencher_chrome::ChromeError),
    #[error("Failed to close tab for: {0}")]
    CloseTab(String),
}
