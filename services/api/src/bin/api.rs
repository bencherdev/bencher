use bencher_api::{
    config::{config_tx::ConfigTx, Config},
    ApiError,
};
use dropshot::HttpServer;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    // Install global subscriber configured based on RUST_LOG envvar.
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)?;

    info!(
        "\u{1f430} Bencher API Server v{}",
        env!("CARGO_PKG_VERSION")
    );
    run().await
}

async fn run() -> Result<(), ApiError> {
    loop {
        let config = Config::load_or_default().await?;
        let (restart_tx, mut restart_rx) = tokio::sync::mpsc::channel(1);
        let config_tx = ConfigTx { config, restart_tx };

        let handle = tokio::spawn(async move {
            async fn run_http_server(config_tx: ConfigTx) -> Result<(), ApiError> {
                HttpServer::try_from(config_tx)?
                    .await
                    .map_err(ApiError::RunServer)
            }

            if let Err(e) = run_http_server(config_tx).await {
                error!("Server Failure: {e}");
            }
        });

        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            restart = restart_rx.recv() => {
                if restart.is_some() {
                    handle.abort();
                } else {
                    break;
                }
            },
        }
    }

    Ok(())
}
