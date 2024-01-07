use crate::{bencher::sub::SubCmd, parser::docker::CliLogs, CliError};
use bollard::{
    container::{LogOutput, LogsOptions},
    Docker,
};
use futures_util::stream::StreamExt;

use crate::{cli_eprintln, cli_println};

use super::{DockerError, BENCHER_API_CONTAINER, BENCHER_UI_CONTAINER};

#[derive(Debug, Clone)]
pub struct Logs {
    container: Option<String>,
}

impl From<CliLogs> for Logs {
    fn from(logs: CliLogs) -> Self {
        let CliLogs { container } = logs;
        Self { container }
    }
}

impl SubCmd for Logs {
    async fn exec(&self) -> Result<(), CliError> {
        let docker = Docker::connect_with_local_defaults().map_err(DockerError::Daemon)?;

        if let Some(container) = &self.container {
            let mut logs = container_logs(&docker, container);
            cli_println!("🐰 Bencher Self-Hosted (`{container}`) logs...");

            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        cli_println!("🐰 Bencher Self-Hosted (`{container}`) logs closed.");
                        break;
                    }
                    Some(log) = logs.next() => {
                        match log {
                            Ok(log) => cli_println!("{log}"),
                            Err(err) => {
                                cli_eprintln!("🐰 Bencher Self-Hosted (`{container}`) logs closed: {err}");
                                break;
                            }
                        }
                    },
                }
            }
        } else {
            watch_container_logs(&docker).await;
        }

        Ok(())
    }
}

pub async fn watch_container_logs(docker: &Docker) {
    let mut api_logs = container_logs(docker, BENCHER_API_CONTAINER);
    let mut ui_logs = container_logs(docker, BENCHER_UI_CONTAINER);
    cli_println!("🐰 Bencher Self-Hosted logs...");

    let mut closed = false;
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                cli_println!("🐰 Bencher Self-Hosted logs closed.");
                break;
            }
            Some(log) = api_logs.next() => {
                match log {
                    Ok(log) => cli_println!("{log}"),
                    Err(err) => {
                        cli_eprintln!("🐰 Bencher Self-Hosted API logs closed: {err}");
                        if closed {
                            break;
                        }
                        closed = true;
                    }
                }
            },
            Some(log) = ui_logs.next() => {
                match log {
                    Ok(log) => cli_println!("{log}"),
                    Err(err) => {
                        cli_eprintln!("🐰 Bencher Self-Hosted UI logs closed: {err}");
                        if closed {
                            break;
                        }
                        closed = true;
                    }
                }
            },
        }
    }
}

fn container_logs(
    docker: &Docker,
    container: &str,
) -> impl futures_util::Stream<Item = Result<LogOutput, bollard::errors::Error>> {
    let options = Some(LogsOptions {
        follow: true,
        stdout: true,
        stderr: true,
        tail: "all".to_owned(),
        ..Default::default()
    });
    docker.logs(container, options)
}
