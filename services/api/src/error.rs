use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("the data for key `{0}` is not available")]
    Redaction(String),
    #[error("unknown data store error")]
    Unknown,
}
