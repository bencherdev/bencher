use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Failed to set global default logger")]
    SetGlobalDefault(#[from] tracing::subscriber::SetGlobalDefaultError),
    #[error("Failed to import .env file")]
    DotEnv(#[from] dotenvy::Error),
    #[error("Failed to create database connection")]
    Connection(#[from] diesel::result::ConnectionError),
    #[error("Failed to run database migrations")]
    Migrations(Box<dyn std::error::Error + Send + Sync>),
    #[error("Failed to create server logger")]
    CreateLogger(std::io::Error),
    #[error("Failed to create server")]
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
}
