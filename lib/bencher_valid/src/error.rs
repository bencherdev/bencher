use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidError {
    #[error("Failed to validate user name: {0}")]
    UserName(String),
}
