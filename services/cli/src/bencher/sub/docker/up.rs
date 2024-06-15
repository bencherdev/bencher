use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    image::CreateImageOptions,
    service::{HostConfig, PortBinding},
    Docker,
};
use futures_util::TryStreamExt;

use super::DockerError;
use crate::{
    bencher::sub::{
        docker::{down::stop_containers, logs::tail_container_logs, Container},
        SubCmd,
    },
    cli_eprintln, cli_println,
    parser::docker::{CliService, CliUp, CliUpPull},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Up {
    service: CliService,
    detach: bool,
    pull: CliUpPull,
    tag: Option<String>,
    api_env: Option<Vec<String>>,
    console_env: Option<Vec<String>>,
    api_volume: Option<Vec<String>>,
    console_volume: Option<Vec<String>>,
}

impl From<CliUp> for Up {
    fn from(up: CliUp) -> Self {
        let CliUp {
            service,
            detach,
            pull,
            tag,
            api_env,
            console_env,
            api_volume,
            console_volume,
        } = up;
        Self {
            service: service.unwrap_or_default(),
            detach,
            pull: pull.unwrap_or_default(),
            tag,
            api_env,
            console_env,
            api_volume,
            console_volume,
        }
    }
}

impl SubCmd for Up {
    async fn exec(&self) -> Result<(), CliError> {
        let docker = Docker::connect_with_local_defaults().map_err(DockerError::Daemon)?;
        stop_containers(&docker, self.service).await?;
        self.pull_images(&docker).await?;
        self.start_containers(&docker).await?;

        cli_println!("🐰 Bencher Self-Hosted is up and running!");
        if let CliService::All | CliService::Console = self.service {
            cli_println!("Web Console: {}", Container::Console.url());
        }
        if let CliService::All | CliService::Api = self.service {
            cli_println!("API Server: {}", Container::Api.url());
        }
        cli_println!("");

        if self.detach {
            cli_println!("Run `bencher down` to stop Bencher Self-Hosted.");
        } else {
            cli_println!("Press Ctrl+C to stop Bencher Self-Hosted.");
            cli_println!("");
            tail_container_logs(&docker, self.service).await;
            stop_containers(&docker, self.service).await?;
        }

        Ok(())
    }
}

impl Up {
    async fn pull_images(&self, docker: &Docker) -> Result<(), DockerError> {
        if let CliService::All | CliService::Console = self.service {
            pull_image(docker, Container::Console, self.pull, self.tag.as_deref()).await?;
        }
        if let CliService::All | CliService::Api = self.service {
            pull_image(docker, Container::Api, self.pull, self.tag.as_deref()).await?;
        }
        Ok(())
    }

    async fn start_containers(&self, docker: &Docker) -> Result<(), DockerError> {
        if let CliService::All | CliService::Api = self.service {
            start_container(
                docker,
                Container::Api,
                self.tag.as_deref(),
                self.api_env.clone(),
                self.api_volume.clone(),
            )
            .await?;
        }
        if let CliService::All | CliService::Console = self.service {
            start_container(
                docker,
                Container::Console,
                self.tag.as_deref(),
                self.console_env.clone(),
                self.console_volume.clone(),
            )
            .await?;
        }
        Ok(())
    }
}

async fn pull_image(
    docker: &Docker,
    container: Container,
    pull: CliUpPull,
    tag: Option<&str>,
) -> Result<(), DockerError> {
    let image = container.image(tag);
    match pull {
        CliUpPull::Always => {},
        CliUpPull::Missing => {
            if docker.inspect_image(&image).await.is_ok() {
                return Ok(());
            }
        },
        CliUpPull::Never => return Ok(()),
    }

    cli_println!("Pulling `{image}` image...");
    let options = Some(CreateImageOptions {
        from_image: image.as_str(),
        ..Default::default()
    });
    docker
        .create_image(options, None, None)
        .try_collect::<Vec<_>>()
        .await
        .map_err(|err| {
            if let bollard::errors::Error::DockerStreamError { error } = &err {
                cli_eprintln!("{error}");
                cli_eprintln!("Are you on Windows? Are you running in Linux container mode?");
                cli_eprintln!(r#"Try running: & 'C:\Program Files\Docker\Docker\DockerCli.exe' -SwitchLinuxEngine"#);
            }
            DockerError::CreateImage {
                image,
                err,
            }
        })?;
    Ok(())
}

async fn start_container(
    docker: &Docker,
    container: Container,
    tag: Option<&str>,
    env: Option<Vec<String>>,
    volume: Option<Vec<String>>,
) -> Result<(), DockerError> {
    let tcp_port = format!("{port}/tcp", port = container.port());

    cli_println!("Creating `{container}` container...");
    let options = Some(CreateContainerOptions {
        name: container.as_ref(),
        platform: None,
    });
    let host_config = Some(HostConfig {
        port_bindings: Some(literally::hmap! {
            tcp_port.clone() => Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".to_owned()),
                host_port: Some(container.port().to_string()),
            }]),
        }),
        publish_all_ports: Some(true),
        binds: volume,
        ..Default::default()
    });

    let config = Config {
        image: Some(container.image(tag)),
        host_config,
        env,
        exposed_ports: Some(literally::hmap! {
            tcp_port.as_str() => literally::hmap! {}
        }),
        ..Default::default()
    };
    docker
        .create_container(options, config)
        .await
        .map_err(|err| DockerError::CreateContainer { container, err })?;

    cli_println!("Starting `{container}` container...");
    docker
        .start_container(container.as_ref(), None::<StartContainerOptions<String>>)
        .await
        .map_err(|err| DockerError::StartContainer { container, err })?;

    cli_println!("");

    Ok(())
}
