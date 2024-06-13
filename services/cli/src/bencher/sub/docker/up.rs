use bencher_json::{
    BENCHER_API_PORT, BENCHER_CONSOLE_PORT, LOCALHOST_BENCHER_API_URL_STR,
    LOCALHOST_BENCHER_URL_STR,
};
use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    image::CreateImageOptions,
    service::{HostConfig, PortBinding},
    Docker,
};
use futures_util::TryStreamExt;

use crate::{
    bencher::sub::{
        docker::{down::stop_container, logs::tail_container_logs},
        SubCmd,
    },
    cli_eprintln, cli_println,
    parser::docker::{CliUp, CliUpPull},
    CliError,
};

use super::{
    DockerError, BENCHER_API_CONTAINER, BENCHER_API_IMAGE, BENCHER_CONSOLE_CONTAINER,
    BENCHER_CONSOLE_IMAGE,
};

#[derive(Debug, Clone)]
pub struct Up {
    detach: bool,
    pull: Pull,
    env: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum Pull {
    Always,
    Missing,
    Never,
}

impl From<CliUp> for Up {
    fn from(up: CliUp) -> Self {
        let CliUp { detach, pull, env } = up;
        Self {
            detach,
            pull: pull.unwrap_or_default().into(),
            env,
        }
    }
}

impl From<CliUpPull> for Pull {
    fn from(pull: CliUpPull) -> Self {
        match pull {
            CliUpPull::Always => Self::Always,
            CliUpPull::Missing => Self::Missing,
            CliUpPull::Never => Self::Never,
        }
    }
}

impl SubCmd for Up {
    async fn exec(&self) -> Result<(), CliError> {
        let docker = Docker::connect_with_local_defaults().map_err(DockerError::Daemon)?;

        stop_container(&docker, BENCHER_CONSOLE_CONTAINER).await?;
        stop_container(&docker, BENCHER_API_CONTAINER).await?;

        let env: Vec<&str> = self.env.iter().map(|s| &**s).collect();
        start_container(
            &docker,
            self.pull,
            env.clone(),
            BENCHER_API_IMAGE,
            BENCHER_API_CONTAINER,
            BENCHER_API_PORT,
        )
        .await?;
        start_container(
            &docker,
            self.pull,
            env,
            BENCHER_CONSOLE_IMAGE,
            BENCHER_CONSOLE_CONTAINER,
            BENCHER_CONSOLE_PORT,
        )
        .await?;

        cli_println!("🐰 Bencher Self-Hosted is up and running!");
        cli_println!("Web Console: {LOCALHOST_BENCHER_URL_STR}");
        cli_println!("API Server: {LOCALHOST_BENCHER_API_URL_STR}");
        cli_println!("");

        if self.detach {
            cli_println!("Run `bencher down` to stop Bencher Self-Hosted.");
        } else {
            cli_println!("Press Ctrl+C to stop Bencher Self-Hosted.");
            cli_println!("");
            tail_container_logs(&docker).await;
            stop_container(&docker, BENCHER_CONSOLE_CONTAINER).await?;
            stop_container(&docker, BENCHER_API_CONTAINER).await?;
        }

        Ok(())
    }
}

async fn start_container(
    docker: &Docker,
    pull: Pull,
    env: Vec<&str>,
    image: &str,
    container: &str,
    port: u16,
) -> Result<(), DockerError> {
    match pull {
        Pull::Always => {
            pull_image(docker, image).await?;
        },
        Pull::Missing => {
            if docker.inspect_image(image).await.is_err() {
                pull_image(docker, image).await?;
            }
        },
        Pull::Never => {},
    }

    let tcp_port = format!("{port}/tcp");

    cli_println!("Creating `{container}` container...");
    let options = Some(CreateContainerOptions {
        name: container,
        platform: None,
    });
    let host_config = Some(HostConfig {
        port_bindings: Some(literally::hmap! {
            tcp_port.clone() => Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".to_owned()),
                host_port: Some(port.to_string()),
            }]),
        }),
        publish_all_ports: Some(true),
        ..Default::default()
    });
    let config = Config {
        image: Some(image),
        host_config,
        env: Some(env),
        exposed_ports: Some(literally::hmap! {
            tcp_port.as_str() => literally::hmap! {}
        }),
        ..Default::default()
    };
    docker
        .create_container(options, config)
        .await
        .map_err(|err| DockerError::CreateContainer {
            container: container.to_owned(),
            err,
        })?;

    cli_println!("Starting `{container}` container...");
    docker
        .start_container(container, None::<StartContainerOptions<String>>)
        .await
        .map_err(|err| DockerError::StartContainer {
            container: container.to_owned(),
            err,
        })?;

    cli_println!("");

    Ok(())
}

async fn pull_image(docker: &Docker, image: &str) -> Result<(), DockerError> {
    cli_println!("Pulling `{image}` image...");
    let options = Some(CreateImageOptions {
        from_image: image,
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
                image: image.to_owned(),
                err,
            }
        })?;
    Ok(())
}
