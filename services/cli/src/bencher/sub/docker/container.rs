use std::fmt;

use bencher_json::{BENCHER_API_PORT, BENCHER_CONSOLE_PORT};

const BENCHER_API_IMAGE: &str = "ghcr.io/bencherdev/bencher-api";
const BENCHER_API_CONTAINER: &str = "bencher_api";

const BENCHER_CONSOLE_IMAGE: &str = "ghcr.io/bencherdev/bencher-console";
const BENCHER_CONSOLE_CONTAINER: &str = "bencher_console";

#[derive(Debug, Clone, Copy)]
pub enum Container {
    Api,
    Console,
}

impl AsRef<str> for Container {
    fn as_ref(&self) -> &str {
        match self {
            Self::Api => BENCHER_API_CONTAINER,
            Self::Console => BENCHER_CONSOLE_CONTAINER,
        }
    }
}

impl fmt::Display for Container {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl Container {
    pub fn image(self, tag: &str) -> String {
        let image = match self {
            Self::Api => BENCHER_API_IMAGE,
            Self::Console => BENCHER_CONSOLE_IMAGE,
        };
        format!("{image}:{tag}")
    }

    pub fn external_port(self, port: Option<u16>) -> u16 {
        if let Some(port) = port {
            port
        } else {
            self.internal_port()
        }
    }

    pub fn internal_port(self) -> u16 {
        match self {
            Self::Api => BENCHER_API_PORT,
            Self::Console => BENCHER_CONSOLE_PORT,
        }
    }

    pub fn url(self, port: Option<u16>) -> String {
        format!("http://localhost:{}", self.external_port(port))
    }
}
