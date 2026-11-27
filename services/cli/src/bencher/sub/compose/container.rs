use std::fmt;

use bencher_json::{BENCHER_API_PORT, BENCHER_CONSOLE_PORT};
use url::{Host, Url};

const BENCHER_API_IMAGE: &str = "ghcr.io/bencherdev/bencher-api";
const BENCHER_API_CONTAINER: &str = "bencher_api";

const BENCHER_CONSOLE_IMAGE: &str = "ghcr.io/bencherdev/bencher-console";
const BENCHER_CONSOLE_CONTAINER: &str = "bencher_console";

const BENCHER_API_URL: &str = "BENCHER_API_URL";
const INTERNAL_API_URL: &str = "INTERNAL_API_URL";

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

    pub fn internal_port(self) -> u16 {
        match self {
            Self::Api => BENCHER_API_PORT,
            Self::Console => BENCHER_CONSOLE_PORT,
        }
    }

    pub fn url(port: u16) -> String {
        format!("http://localhost:{port}")
    }

    /// The environment `env` the user passed. For a console, an API URL whose last value is empty
    /// is left out, so the image's default applies, and a console whose API is unnamed or on a
    /// loopback address relays to the API container over their shared network, unless the user
    /// named a relay.
    pub fn env(self, env: Option<Vec<String>>) -> Option<Vec<String>> {
        let Self::Console = self else {
            return env;
        };
        let mut env = env.unwrap_or_default();
        for name in [BENCHER_API_URL, INTERNAL_API_URL] {
            if named(&env, name).is_some_and(str::is_empty) {
                env.retain(|var| var.split_once('=').is_none_or(|(key, _)| key != name));
            }
        }
        if named(&env, INTERNAL_API_URL).is_some()
            || named(&env, BENCHER_API_URL).is_some_and(|url| !is_loopback(url))
        {
            return Some(env);
        }
        env.push(format!(
            "{INTERNAL_API_URL}=http://{BENCHER_API_CONTAINER}:{BENCHER_API_PORT}"
        ));
        Some(env)
    }
}

/// The value of `name` in `env`, where Docker takes the last.
fn named<'env>(env: &'env [String], name: &str) -> Option<&'env str> {
    env.iter().rev().find_map(|var| {
        var.split_once('=')
            .and_then(|(key, value)| (key == name).then_some(value))
    })
}

/// Whether `url` names this machine, which inside the console's container is the container.
fn is_loopback(url: &str) -> bool {
    Url::parse(url)
        .ok()
        .and_then(|url| {
            url.host().map(|host| match host {
                Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
                Host::Ipv4(ip) => ip.is_loopback(),
                Host::Ipv6(ip) => ip.is_loopback(),
            })
        })
        .unwrap_or_default()
}
