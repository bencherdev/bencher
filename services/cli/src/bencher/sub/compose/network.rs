use bollard::{
    Docker,
    errors::Error as BollardError,
    models::{NetworkCreateRequest, NetworkInspect},
};

use crate::cli_println;

use super::DockerError;

/// The Docker network both containers join, where the console reaches the API by its container
/// name.
pub const NETWORK: &str = "bencher";
/// The label that marks the network as one `bencher up` made, as Docker Compose labels its own.
pub const NETWORK_LABEL: &str = "dev.bencher.network";

/// Create the network, or reuse one an earlier run left behind.
pub(super) async fn create_network(docker: &Docker) -> Result<(), DockerError> {
    if let Ok(network) = docker.inspect_network(NETWORK, None).await {
        return ours(&network);
    }
    cli_println!("Creating `{NETWORK}` network...");
    if let Err(err) = docker.create_network(network_config()).await {
        let BollardError::DockerResponseServerError {
            status_code: 409, ..
        } = err
        else {
            return Err(DockerError::CreateNetwork(err));
        };
        // Made since the inspect, by another run or by someone else.
        let network = docker
            .inspect_network(NETWORK, None)
            .await
            .map_err(DockerError::CreateNetwork)?;
        ours(&network)?;
    }
    cli_println!("");
    Ok(())
}

/// Remove the network once no container is left on it, if `bencher up` made it.
pub(super) async fn remove_network(docker: &Docker) -> Result<(), DockerError> {
    let Ok(network) = docker.inspect_network(NETWORK, None).await else {
        return Ok(());
    };
    if !is_ours(&network)
        || network
            .containers
            .is_some_and(|containers| !containers.is_empty())
    {
        return Ok(());
    }
    cli_println!("Removing `{NETWORK}` network...");
    docker
        .remove_network(NETWORK)
        .await
        .map_err(DockerError::RemoveNetwork)?;
    cli_println!("");
    Ok(())
}

fn network_config() -> NetworkCreateRequest {
    NetworkCreateRequest {
        name: NETWORK.to_owned(),
        driver: Some("bridge".to_owned()),
        labels: Some(literally::hmap! {
            NETWORK_LABEL.to_owned() => NETWORK.to_owned(),
        }),
        ..Default::default()
    }
}

fn ours(network: &NetworkInspect) -> Result<(), DockerError> {
    if is_ours(network) {
        Ok(())
    } else {
        Err(DockerError::ForeignNetwork)
    }
}

fn is_ours(network: &NetworkInspect) -> bool {
    network
        .labels
        .as_ref()
        .and_then(|labels| labels.get(NETWORK_LABEL))
        .is_some_and(|value| value == NETWORK)
}

#[cfg(test)]
mod tests {
    use bollard::models::NetworkInspect;

    use super::{NETWORK_LABEL, is_ours, network_config};

    fn labeled(key: &str, value: &str) -> NetworkInspect {
        NetworkInspect {
            labels: Some(literally::hmap! { key.to_owned() => value.to_owned() }),
            ..Default::default()
        }
    }

    // Kills a network made without its label, which `bencher up` would then refuse to reuse and
    // `bencher down` would leave behind, and a check that takes someone else's network for its own.
    #[test]
    fn only_the_network_bencher_up_makes_is_its_own() {
        let made = NetworkInspect {
            labels: network_config().labels,
            ..Default::default()
        };
        assert!(is_ours(&made));
        assert!(!is_ours(&NetworkInspect::default()));
        assert!(!is_ours(&labeled(NETWORK_LABEL, "other")));
        assert!(!is_ours(&labeled("com.docker.compose.network", "bencher")));
    }
}
