use dropshot::HttpError;
use thiserror::Error;

use crate::{Endpoint, WordStr};

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Failed to set global default logger")]
    SetGlobalDefault(#[from] tracing::subscriber::SetGlobalDefaultError),
    #[error("Failed to import .env file")]
    DotEnv(#[from] dotenvy::Error),
    #[error("Failed to create database connection")]
    Connection(#[from] diesel::result::ConnectionError),
    #[error("Failed to run database migrations: {0}")]
    Migrations(Box<dyn std::error::Error + Send + Sync>),
    #[error("Failed to create server logger: {0}")]
    CreateLogger(std::io::Error),
    #[error("Failed to create server: {0}")]
    CreateServer(Box<dyn std::error::Error + Send + Sync>),
    #[error("Shutting down server: {0}")]
    RunServer(String),

    #[cfg(feature = "swagger")]
    #[error("Failed to create swagger file: {0}")]
    CreateSwaggerFile(std::io::Error),
    #[cfg(feature = "swagger")]
    #[error("Failed to create swagger file: {0}")]
    WriteSwaggerFile(serde_json::Error),

    #[error("{0}")]
    Endpoint(String),
    // TODO impl display
    #[error("{0}")]
    IntoEndpoint(Endpoint),

    #[error("Failed to GET {}", _0.singular())]
    GetOne(Endpoint),
    #[error("Failed to GET {}", _0.plural())]
    GetLs(Endpoint),
    #[error("Failed to POST {}", _0.singular())]
    Post(Endpoint),
    #[error("Failed to PUT {}", _0.singular())]
    Put(Endpoint),
    #[error("Failed to DELETE {}", _0.singular())]
    Delete(Endpoint),
}

impl From<ApiError> for HttpError {
    fn from(error: ApiError) -> Self {
        dropshot::HttpError::for_bad_request(
            Some(http::status::StatusCode::BAD_REQUEST.to_string()),
            error.to_string(),
        )
    }
}
