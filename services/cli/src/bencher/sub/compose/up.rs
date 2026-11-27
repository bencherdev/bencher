use bollard::{
    Docker,
    errors::Error as BollardError,
    models::{
        ContainerCreateBody, ContainerInspectResponse, HostConfig, NetworkConnectRequest,
        PortBinding,
    },
    query_parameters::{CreateContainerOptions, CreateImageOptions, StartContainerOptions},
};
use futures_util::TryStreamExt as _;

use super::DockerError;
use crate::{
    CliError,
    bencher::sub::{
        SubCmd,
        compose::{
            Container,
            down::stop_containers,
            logs::tail_container_logs,
            network::{NETWORK, create_network, remove_network},
        },
    },
    cli_eprintln, cli_println,
    parser::compose::{CliService, CliUp, CliUpPull},
};

#[derive(Debug, Clone)]
pub struct Up {
    service: CliService,
    detach: bool,
    pull: CliUpPull,
    tag: String,
    api_port: u16,
    console_port: u16,
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
            api_port,
            console_port,
            api_env,
            console_env,
            api_volume,
            console_volume,
        } = up;
        Self {
            service,
            detach,
            pull,
            tag,
            api_port,
            console_port,
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
        // https://github.com/fussybeaver/bollard/issues/383
        docker.ping().await.map_err(DockerError::Ping)?;

        // Before anything stops, so a network that is not Bencher's leaves the containers running.
        create_network(&docker).await?;
        stop_containers(&docker, self.service).await?;
        self.pull_images(&docker).await?;
        self.start_containers(&docker).await?;

        cli_println!("🐰 Bencher Self-Hosted is up and running!");
        if let CliService::All | CliService::Console = self.service {
            cli_println!("Console Server: {}", Container::url(self.console_port));
        }
        if let CliService::All | CliService::Api = self.service {
            cli_println!("API Server: {}", Container::url(self.api_port));
        }
        cli_println!("");

        if self.detach {
            cli_println!("Run `bencher down` to stop Bencher Self-Hosted.");
        } else {
            cli_println!("Press Ctrl+C to stop Bencher Self-Hosted.");
            cli_println!("");
            tail_container_logs(&docker, self.service).await;
            stop_containers(&docker, self.service).await?;
            remove_network(&docker).await?;
        }

        Ok(())
    }
}

impl Up {
    async fn pull_images(&self, docker: &Docker) -> Result<(), DockerError> {
        if let CliService::All | CliService::Console = self.service {
            pull_image(docker, Container::Console, self.pull, &self.tag).await?;
        }
        if let CliService::All | CliService::Api = self.service {
            pull_image(docker, Container::Api, self.pull, &self.tag).await?;
        }
        Ok(())
    }

    async fn start_containers(&self, docker: &Docker) -> Result<(), DockerError> {
        if let CliService::All | CliService::Api = self.service {
            let config = container_config(
                Container::Api,
                &self.tag,
                self.api_port,
                self.api_env.clone(),
                self.api_volume.clone(),
            );
            start_container(docker, Container::Api, config).await?;
        }
        if let CliService::Console = self.service {
            connect_api(docker).await?;
        }
        if let CliService::All | CliService::Console = self.service {
            let config = container_config(
                Container::Console,
                &self.tag,
                self.console_port,
                self.console_env.clone(),
                self.console_volume.clone(),
            );
            start_container(docker, Container::Console, config).await?;
        }
        Ok(())
    }
}

async fn pull_image(
    docker: &Docker,
    container: Container,
    pull: CliUpPull,
    tag: &str,
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
        from_image: Some(image.clone()),
        ..Default::default()
    });
    docker
        .create_image(options, None, None)
        .try_collect::<Vec<_>>()
        .await
        .map_err(|err| {
            if let BollardError::DockerStreamError { error } = &err {
                cli_eprintln!("{error}");
                cli_eprintln!("Are you on Windows? Are you running in Linux container mode?");
                cli_eprintln!(r"Try running: & 'C:\Program Files\Docker\Docker\DockerCli.exe' -SwitchLinuxEngine");
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
    config: ContainerCreateBody,
) -> Result<(), DockerError> {
    cli_println!("Creating `{container}` container...");
    let options = Some(CreateContainerOptions {
        name: Some(container.as_ref().to_owned()),
        ..Default::default()
    });
    docker
        .create_container(options, config)
        .await
        .map_err(|err| DockerError::CreateContainer { container, err })?;

    cli_println!("Starting `{container}` container...");
    docker
        .start_container(container.as_ref(), None::<StartContainerOptions>)
        .await
        .map_err(|err| DockerError::StartContainer { container, err })?;

    cli_println!("");

    Ok(())
}

/// Connect a running API container that is off the network, as an older `bencher up`, Docker
/// Compose, or `docker run` leaves it, so the console reaches it by name.
async fn connect_api(docker: &Docker) -> Result<(), DockerError> {
    let container = Container::Api;
    let Ok(api) = docker.inspect_container(container.as_ref(), None).await else {
        return Ok(());
    };
    if !needs_network(&api) {
        return Ok(());
    }
    cli_println!("Connecting `{container}` container to `{NETWORK}` network...");
    let config = NetworkConnectRequest {
        container: container.as_ref().to_owned(),
        ..Default::default()
    };
    if let Err(err) = docker.connect_network(NETWORK, config).await
        && !already_connected(&err)
    {
        return Err(DockerError::ConnectNetwork { container, err });
    }
    cli_println!("");
    Ok(())
}

fn needs_network(api: &ContainerInspectResponse) -> bool {
    let running = api
        .state
        .as_ref()
        .and_then(|state| state.running)
        .unwrap_or_default();
    let on_network = api
        .network_settings
        .as_ref()
        .and_then(|settings| settings.networks.as_ref())
        .is_some_and(|networks| networks.contains_key(NETWORK));
    running && !on_network
}

/// Docker's answer to connecting a container already on the network, by another run since the
/// inspect.
fn already_connected(err: &BollardError) -> bool {
    let BollardError::DockerResponseServerError {
        status_code: 403,
        message,
    } = err
    else {
        return false;
    };
    message.contains("already exists")
}

/// The container `bencher up` creates, on the network the two containers share.
fn container_config(
    container: Container,
    tag: &str,
    port: u16,
    env: Option<Vec<String>>,
    volume: Option<Vec<String>>,
) -> ContainerCreateBody {
    let tcp_port = format!("{port}/tcp", port = container.internal_port());
    let host_config = Some(HostConfig {
        port_bindings: Some(literally::hmap! {
            tcp_port.clone() => Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".to_owned()),
                host_port: Some(port.to_string()),
            }]),
        }),
        publish_all_ports: Some(true),
        binds: volume,
        network_mode: Some(NETWORK.to_owned()),
        ..Default::default()
    });
    ContainerCreateBody {
        image: Some(container.image(tag)),
        host_config,
        env: container.env(env),
        exposed_ports: Some(vec![tcp_port]),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use bollard::{
        errors::Error as BollardError,
        models::{ContainerInspectResponse, ContainerState, EndpointSettings, NetworkSettings},
    };

    use super::{Container, already_connected, container_config, needs_network};

    const RELAY_TO_API: &str = "INTERNAL_API_URL=http://bencher_api:6610";
    const USER_RELAY: &str = "INTERNAL_API_URL=https://api.example.com";

    fn env(container: Container, env: &[&str]) -> Option<Vec<String>> {
        container_config(
            container,
            "latest",
            1234,
            (!env.is_empty()).then(|| env.iter().copied().map(ToOwned::to_owned).collect()),
            None,
        )
        .env
    }

    fn api(running: bool, networks: &[&str]) -> ContainerInspectResponse {
        ContainerInspectResponse {
            state: Some(ContainerState {
                running: Some(running),
                ..Default::default()
            }),
            network_settings: Some(NetworkSettings {
                networks: Some(
                    networks
                        .iter()
                        .map(|network| ((*network).to_owned(), EndpointSettings::default()))
                        .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    // Kills a container left on Docker's default bridge, where the console cannot reach the API
    // container by its name.
    #[test]
    fn both_containers_join_the_shared_network() {
        for container in [Container::Api, Container::Console] {
            let network_mode = container_config(container, "latest", 1234, None, None)
                .host_config
                .and_then(|host_config| host_config.network_mode);
            assert_eq!(network_mode.as_deref(), Some("bencher"), "{container}");
        }
    }

    // Kills a console left to relay to its own `localhost`, where no API listens, one that relays
    // to the API container in place of a remote API its user named, one that reads the first of
    // two values where Docker takes the last, and one that overrides the relay its user named.
    #[test]
    fn the_console_relays_to_the_api_container_unless_its_api_is_remote() {
        assert_eq!(
            env(Container::Console, &[]),
            Some(vec![RELAY_TO_API.to_owned()])
        );
        for local in [
            "BENCHER_API_URL=http://localhost:6610",
            "BENCHER_API_URL=http://LOCALHOST:7000",
            "BENCHER_API_URL=http://127.0.0.1:6610",
            "BENCHER_API_URL=http://127.5.6.7:6610",
            "BENCHER_API_URL=http://[::1]:6610",
        ] {
            assert_eq!(
                env(Container::Console, &[local]),
                Some(vec![local.to_owned(), RELAY_TO_API.to_owned()]),
                "{local}"
            );
        }
        for remote in [
            "BENCHER_API_URL=https://api.example.com",
            "BENCHER_API_URL=http://10.0.0.5:6610",
            "BENCHER_API_URL=http://[2001:db8::1]:6610",
            "BENCHER_API_URL=http://localhost.example.com",
        ] {
            assert_eq!(
                env(Container::Console, &[remote]),
                Some(vec![remote.to_owned()]),
                "{remote}"
            );
        }
        let remote = "BENCHER_API_URL=https://api.example.com";
        let local = "BENCHER_API_URL=http://localhost:6610";
        assert_eq!(
            env(Container::Console, &[remote, local]),
            Some(vec![
                remote.to_owned(),
                local.to_owned(),
                RELAY_TO_API.to_owned()
            ])
        );
        assert_eq!(
            env(Container::Console, &[local, remote]),
            Some(vec![local.to_owned(), remote.to_owned()])
        );
        for named in [
            vec![USER_RELAY],
            vec!["BENCHER_API_URL=http://localhost:6610", USER_RELAY],
            vec!["BENCHER_API_URL=https://api.example.com", USER_RELAY],
        ] {
            assert_eq!(
                env(Container::Console, &named),
                Some(named.iter().copied().map(ToOwned::to_owned).collect()),
                "{named:?}"
            );
        }
        assert_eq!(env(Container::Api, &[]), None);
    }

    // Kills a console handed an empty API URL, which Docker passes over the image's default and
    // the console rejects as missing, one handed an empty relay, which the console ignores in
    // place of the API container, one that keeps an earlier value Docker would not take, and one
    // that changes the API's environment.
    #[test]
    fn an_empty_api_url_is_left_out_for_the_default() {
        let remote = "BENCHER_API_URL=https://api.example.com";
        for (user, console) in [
            (vec!["BENCHER_API_URL="], vec![RELAY_TO_API]),
            (vec![remote, "BENCHER_API_URL="], vec![RELAY_TO_API]),
            (vec!["INTERNAL_API_URL="], vec![RELAY_TO_API]),
            (vec![USER_RELAY, "INTERNAL_API_URL="], vec![RELAY_TO_API]),
            (vec![remote, "INTERNAL_API_URL="], vec![remote]),
        ] {
            assert_eq!(
                env(Container::Console, &user),
                Some(console.iter().copied().map(ToOwned::to_owned).collect()),
                "{user:?}"
            );
        }
        let empty = ["BENCHER_API_URL=", "INTERNAL_API_URL="];
        assert_eq!(
            env(Container::Api, &empty),
            Some(empty.iter().copied().map(ToOwned::to_owned).collect())
        );
    }

    // Kills a `bencher up console` that leaves a running API container off the network, one that
    // connects a container already on it, and one that connects a stopped container.
    #[test]
    fn only_a_running_api_container_off_the_network_needs_it() {
        assert!(needs_network(&api(true, &["bridge"])));
        assert!(needs_network(&api(true, &[])));
        assert!(!needs_network(&api(true, &["bridge", "bencher"])));
        assert!(!needs_network(&api(false, &["bridge"])));
        assert!(!needs_network(&ContainerInspectResponse::default()));
    }

    // Kills a `bencher up console` that fails when another run connected the API container since
    // its inspect, and one that hides every other refusal.
    #[test]
    fn a_container_already_on_the_network_counts_as_connected() {
        let refusal = |status_code, message: &str| BollardError::DockerResponseServerError {
            status_code,
            message: message.to_owned(),
        };
        assert!(already_connected(&refusal(
            403,
            "endpoint with name bencher_api already exists in network bencher"
        )));
        assert!(!already_connected(&refusal(
            403,
            "container sharing network namespace with another container cannot be connected"
        )));
        assert!(!already_connected(&refusal(
            404,
            "No such container: bencher_api"
        )));
    }
}
