use crate::{
    bencher::sub::SubCmd,
    parser::docker::{CliLogs, CliService},
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
    service: CliService,
}

impl From<CliLogs> for Logs {
    fn from(logs: CliLogs) -> Self {
        let CliLogs { service } = logs;
        Self { service }
    }
}

impl SubCmd for Logs {
    async fn exec(&self) -> Result<(), CliError> {
        let docker = Docker::connect_with_local_defaults().map_err(DockerError::Daemon)?;
        // https://github.com/fussybeaver/bollard/issues/383
        docker.ping().await.map_err(DockerError::Ping)?;

        tail_container_logs(&docker, self.service).await;

        Ok(())
    }
}

pub(super) async fn tail_container_logs(docker: &Docker, service: CliService) {
    let mut api_logs = if let CliService::All | CliService::Api = service {
        Some(container_logs(docker, Container::Api))
    } else {
        None
    };
    let mut console_logs = if let CliService::All | CliService::Console = service {
        Some(container_logs(docker, Container::Console))
    } else {
        None
    };
    cli_println!("🐰 Bencher Self-Hosted logs...");
    cli_println!("");

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
                        break;
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
                        break;
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
