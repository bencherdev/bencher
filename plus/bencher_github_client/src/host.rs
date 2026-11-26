use std::{fmt, sync::LazyLock};

use bencher_valid::{GitHubAppSlug, GitHubRepositoryName, UserName};
use url::Url;

use crate::api::join_path;

#[expect(
    clippy::expect_used,
    reason = "constant URL literal, infallible at runtime"
)]
static GITHUB_COM: LazyLock<Url> = LazyLock::new(|| {
    "https://github.com"
        .parse()
        .expect("Invalid github.com URL")
});

#[expect(
    clippy::expect_used,
    reason = "constant URL literal, infallible at runtime"
)]
static GITHUB_COM_API: LazyLock<Url> = LazyLock::new(|| {
    "https://api.github.com"
        .parse()
        .expect("Invalid GitHub API URL")
});

/// The GitHub instance the server's GitHub App lives on, named by its web root: github.com, or a
/// GitHub Enterprise Server. Every other GitHub URL derives from the root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubHost {
    root: Url,
}

impl Default for GitHubHost {
    fn default() -> Self {
        Self {
            root: GITHUB_COM.clone(),
        }
    }
}

impl GitHubHost {
    pub fn new(root: Url) -> Self {
        let mut root = root;
        root.set_query(None);
        root.set_fragment(None);
        if let Ok(mut segments) = root.path_segments_mut() {
            segments.pop_if_empty();
        }
        Self { root }
    }

    pub fn is_github_com(&self) -> bool {
        same_location(&self.root, &GITHUB_COM)
    }

    /// The web root, as a user sees it in their browser.
    pub fn root(&self) -> &Url {
        &self.root
    }

    pub fn api(&self) -> Url {
        if self.is_github_com() {
            GITHUB_COM_API.clone()
        } else {
            join_path(&self.root, &["api", "v3"])
        }
    }

    /// GitHub Enterprise Server serves GraphQL beside its REST API, not under it.
    pub fn graphql(&self) -> Url {
        if self.is_github_com() {
            join_path(&GITHUB_COM_API, &["graphql"])
        } else {
            join_path(&self.root, &["api", "graphql"])
        }
    }

    pub fn authorize(&self) -> Url {
        join_path(&self.root, &["login", "oauth", "authorize"])
    }

    pub fn access_token(&self) -> Url {
        join_path(&self.root, &["login", "oauth", "access_token"])
    }

    pub fn install(&self, slug: &GitHubAppSlug) -> Url {
        let apps = if self.is_github_com() {
            "apps"
        } else {
            "github-apps"
        };
        join_path(&self.root, &[apps, slug.as_ref(), "installations", "new"])
    }

    pub fn repository(&self, full_name: &GitHubRepositoryName) -> Url {
        join_path(&self.root, &[full_name.owner(), full_name.name()])
    }

    pub fn user(&self, login: &UserName) -> Url {
        join_path(&self.root, &[login.as_ref()])
    }

    /// Whether `api_url`, as GitHub Actions names it in `GITHUB_API_URL`, is this host's API.
    pub fn is_api(&self, api_url: &Url) -> bool {
        same_location(&self.api(), api_url)
    }
}

impl fmt::Display for GitHubHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.root.as_str().trim_end_matches('/'))
    }
}

/// The same scheme, host, port, and path, with or without a trailing slash.
fn same_location(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
        && left.path().trim_end_matches('/') == right.path().trim_end_matches('/')
}

#[cfg(test)]
mod tests {
    use super::GitHubHost;

    const SLUG: &str = "bencher-test";

    fn host(root: &str) -> GitHubHost {
        GitHubHost::new(root.parse().unwrap())
    }

    fn urls(host: &GitHubHost) -> [String; 7] {
        [
            host.api(),
            host.graphql(),
            host.authorize(),
            host.access_token(),
            host.install(&SLUG.parse().unwrap()),
            host.repository(&"octo-cat/hello.world".parse().unwrap()),
            host.user(&"octo-cat".parse().unwrap()),
        ]
        .map(String::from)
    }

    // Kills any github.com URL on another host or path, the default naming any other root, and a
    // root that is github.com in another spelling read as GitHub Enterprise Server.
    #[test]
    fn github_com_keeps_its_own_urls() {
        let expected = [
            "https://api.github.com/",
            "https://api.github.com/graphql",
            "https://github.com/login/oauth/authorize",
            "https://github.com/login/oauth/access_token",
            "https://github.com/apps/bencher-test/installations/new",
            "https://github.com/octo-cat/hello.world",
            "https://github.com/octo-cat",
        ];
        for root in [
            None,
            Some("https://github.com"),
            Some("https://github.com/"),
            Some("https://GitHub.com:443"),
        ] {
            let host = root.map_or_else(GitHubHost::default, host);
            assert!(host.is_github_com(), "{root:?}");
            assert_eq!(urls(&host), expected, "{root:?}");
            assert_eq!(host.to_string(), "https://github.com", "{root:?}");
        }
    }

    // Kills a GitHub Enterprise Server URL built as github.com builds it, GraphQL under the REST
    // API, the install page under `apps`, and a root whose trailing slash doubles a separator.
    #[test]
    fn github_enterprise_server_derives_every_url_from_its_root() {
        let expected = [
            "https://github.example.com/api/v3",
            "https://github.example.com/api/graphql",
            "https://github.example.com/login/oauth/authorize",
            "https://github.example.com/login/oauth/access_token",
            "https://github.example.com/github-apps/bencher-test/installations/new",
            "https://github.example.com/octo-cat/hello.world",
            "https://github.example.com/octo-cat",
        ];
        for root in [
            "https://github.example.com",
            "https://github.example.com/",
            "https://github.example.com/?tab=1#top",
        ] {
            let host = host(root);
            assert!(!host.is_github_com(), "{root}");
            assert_eq!(urls(&host), expected, "{root}");
            assert_eq!(host.to_string(), "https://github.example.com", "{root}");
        }
        for root in [
            "http://github.com",
            "https://github.com:8443",
            "https://www.github.com",
            "https://github.com/enterprise",
        ] {
            assert!(!host(root).is_github_com(), "{root}");
        }
    }

    // Kills a comparison that reads the scheme, the host, the port, or the path wrongly, or that
    // refuses a trailing slash or a host in upper case.
    #[test]
    fn the_api_url_matches_only_the_host_api() {
        let github_com = GitHubHost::default();
        let enterprise = host("https://github.example.com");
        for (api_url, is_github_com, is_enterprise) in [
            ("https://api.github.com", true, false),
            ("https://api.github.com/", true, false),
            ("https://API.GitHub.com:443", true, false),
            ("http://api.github.com", false, false),
            ("https://api.github.com:8443", false, false),
            ("https://api.github.com/api/v3", false, false),
            ("https://github.com/api/v3", false, false),
            ("https://github.example.com/api/v3", false, true),
            ("https://github.example.com/api/v3/", false, true),
            ("https://github.example.com/api/v4", false, false),
            ("https://github.example.com", false, false),
            ("https://github.example.org/api/v3", false, false),
            ("https://github.example.com:8443/api/v3", false, false),
            ("http://github.example.com/api/v3", false, false),
        ] {
            let api_url = api_url.parse().unwrap();
            assert_eq!(github_com.is_api(&api_url), is_github_com, "{api_url}");
            assert_eq!(enterprise.is_api(&api_url), is_enterprise, "{api_url}");
        }
    }
}
