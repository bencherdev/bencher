#![cfg(feature = "plus")]

use slog::Logger;

pub async fn register_startup(log: &Logger) {
    startup_inner()
        .await
        .inspect_err(|e| slog::error!(log, "Failed to register startup: {e}"));
}

async fn register_startup_inner() {}
