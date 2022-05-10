use dropshot::ConfigDropshot;

const PORT_KEY: &str = "PORT";
const DEFAULT_IP: &str = "0.0.0.0";
const DEFAULT_PORT: &str = "8080";

pub fn get_config() -> ConfigDropshot {
    let port = std::env::var(PORT_KEY).unwrap_or(DEFAULT_PORT.into());
    let address = format!("{DEFAULT_IP}:{port}");

    ConfigDropshot {
        bind_address: address.parse().unwrap(),
        request_body_max_bytes: 1024,
        tls: None,
    }
}
