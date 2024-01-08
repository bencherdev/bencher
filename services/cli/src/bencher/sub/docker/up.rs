use bencher_json::{
    BENCHER_API_PORT, BENCHER_UI_PORT, LOCALHOST_BENCHER_API_URL_STR, LOCALHOST_BENCHER_URL_STR,
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
    parser::docker::CliUp,
    CliError,
};

use super::{
    DockerError, BENCHER_API_CONTAINER, BENCHER_API_IMAGE, BENCHER_UI_CONTAINER, BENCHER_UI_IMAGE,
};

#[derive(Debug, Clone)]
pub struct Up {
    detach: bool,
}

impl From<CliUp> for Up {
    fn from(up: CliUp) -> Self {
        let CliUp { detach } = up;
        Self { detach }
    }
}

impl SubCmd for Up {
    async fn exec(&self) -> Result<(), CliError> {
        let docker = Docker::connect_with_local_defaults().map_err(DockerError::Daemon)?;

        stop_container(&docker, BENCHER_UI_CONTAINER).await?;
        stop_container(&docker, BENCHER_API_CONTAINER).await?;

        start_container(
            &docker,
            BENCHER_API_IMAGE,
            BENCHER_API_CONTAINER,
            BENCHER_API_PORT,
        )
        .await?;
        start_container(
            &docker,
            BENCHER_UI_IMAGE,
            BENCHER_UI_CONTAINER,
            BENCHER_UI_PORT,
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
            stop_container(&docker, BENCHER_UI_CONTAINER).await?;
            stop_container(&docker, BENCHER_API_CONTAINER).await?;
        }

        Ok(())
    }
}

async fn start_container(
    docker: &Docker,
    image: &str,
    container: &str,
    port: u16,
) -> Result<(), DockerError> {
    pull_image(docker, image).await?;

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
            }
            DockerError::CreateImage {
                image: image.to_owned(),
                err,
            }
        })?;
    Ok(())
}
