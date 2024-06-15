use crate::{
    bencher::sub::SubCmd,
    parser::docker::{CliContainer, CliLogs},
    CliError,
};
use bollard::{
    container::{LogOutput, LogsOptions},
    Docker,
};
use futures_util::stream::StreamExt;

use crate::{cli_eprintln, cli_println};

use super::{Container, DockerError};

#[derive(Debug, Clone)]
pub struct Logs {
    container: CliContainer,
}

impl From<CliLogs> for Logs {
    fn from(logs: CliLogs) -> Self {
        let CliLogs { container } = logs;
        Self {
            container: container.unwrap_or_default(),
        }
    }
}

impl SubCmd for Logs {
    async fn exec(&self) -> Result<(), CliError> {
        let docker = Docker::connect_with_local_defaults().map_err(DockerError::Daemon)?;
        tail_container_logs(&docker, self.container).await;
        Ok(())
    }
}

pub(super) async fn tail_container_logs(docker: &Docker, container: CliContainer) {
    let mut api_logs = if let CliContainer::All | CliContainer::Api = container {
        Some(container_logs(docker, Container::Api))
    } else {
        None
    };
    let mut console_logs = if let CliContainer::All | CliContainer::Console = container {
        Some(container_logs(docker, Container::Console))
    } else {
        None
    };
    cli_println!("🐰 Bencher Self-Hosted logs...");
    cli_println!("");

    let mut closed = false;
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                cli_println!("");
                cli_println!("🐰 Bencher Self-Hosted logs closed.");
                break;
            }
            Some(log) = async {
                if let Some(logs) = api_logs.as_mut() {
                    logs.next().await
                } else {
                    None
                }
            } => {
                match log {
                    Ok(log) => cli_println!("{log}"),
                    Err(err) => {
                        cli_println!("");
                        cli_eprintln!("🐰 Bencher Self-Hosted API logs closed: {err}");
                        if closed {
                            break;
                        }
                        closed = true;
                    }
                }
            },
            Some(log) = async {
                if let Some(logs) = console_logs.as_mut() {
                    logs.next().await
                } else {
                    None
                }
            } => {
                match log {
                    Ok(log) => cli_println!("{log}"),
                    Err(err) => {
                        cli_println!("");
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
    container: Container,
) -> impl futures_util::Stream<Item = Result<LogOutput, bollard::errors::Error>> {
    let options = Some(LogsOptions {
        follow: true,
        stdout: true,
        stderr: true,
        tail: "all".to_owned(),
        ..Default::default()
    });
    docker.logs(container.as_ref(), options)
}
