use bencher_json::{Architecture, Clock, Sha256};
use dashmap::DashMap;
use url::Url;

/// Base URL for stable channel runner downloads.
// Trailing slash is required on both default base URLs: Url::join appends to
// the last path segment, so without it the join would replace that segment
// instead of extending it.
const DEFAULT_STABLE_BASE_URL: &str = "https://github.com/bencherdev/bencher/releases/download/";

/// Base URL for canary channel runner downloads.
const DEFAULT_CANARY_BASE_URL: &str = "https://download.bencher.dev/";

/// Path segment that holds the canary channel artifacts.
const CANARY_PATH_SEGMENT: &str = "canary";

/// How long a fetched canary channel checksum stays fresh.
/// This is also the upper bound on rollout propagation delay: after a new
/// canary publish, runners may be offered the previous build until the cached
/// checksum expires. A runner offered a stale checksum during that window
/// downloads the newer binary, fails verification, and retries at its next
/// `Ready`, so transient "checksum mismatch" runner logs right after a deploy
/// are expected and self-healing. On expiry, concurrent `Ready`
/// polls may each re-fetch (no single-flight); the responses are tiny and
/// the canary fleet is small, so the duplicate fetches are not worth
/// guarding against.
const CANARY_CHECKSUM_TTL_SECS: i64 = 120;

/// Runner self-update state: download URL construction and a TTL cache of the
/// published canary channel checksums, one per architecture.
pub struct RunnerUpdate {
    stable_base_url: Url,
    canary_base_url: Url,
    ttl_secs: i64,
    cache: DashMap<Architecture, CanaryChecksum>,
}

impl RunnerUpdate {
    /// A configured `base_url` replaces the default base URL of both channels,
    /// so one mirror can serve the stable and the canary artifacts.
    pub fn new(base_url: Option<Url>) -> Self {
        let (stable_base_url, canary_base_url) = if let Some(base_url) = base_url {
            let base_url = ensure_trailing_slash(base_url);
            (base_url.clone(), base_url)
        } else {
            (
                default_base_url(DEFAULT_STABLE_BASE_URL),
                default_base_url(DEFAULT_CANARY_BASE_URL),
            )
        };
        Self {
            stable_base_url,
            canary_base_url,
            ttl_secs: CANARY_CHECKSUM_TTL_SECS,
            cache: DashMap::new(),
        }
    }

    /// Download URL for a stable channel runner binary published under a version tag.
    pub fn stable_url(&self, version: &str, arch: Architecture) -> Result<Url, url::ParseError> {
        let tag = format!("v{version}");
        let artifact = format!("runner-{tag}-{slug}", slug = arch.linux_artifact_slug());
        self.stable_base_url.join(&format!("{tag}/{artifact}"))
    }

    /// Download URL for the canary channel runner binary.
    pub fn canary_url(&self, arch: Architecture) -> Result<Url, url::ParseError> {
        let artifact = format!("runner-canary-{slug}", slug = arch.linux_artifact_slug());
        self.canary_base_url
            .join(&format!("{CANARY_PATH_SEGMENT}/{artifact}"))
    }

    /// Return the cached canary channel checksum for `arch`, if still fresh.
    pub fn cached_canary_checksum(&self, clock: &Clock, arch: Architecture) -> Option<Sha256> {
        let entry = self.cache.get(&arch)?;
        let age = clock.timestamp().saturating_sub(entry.fetched);
        (age < self.ttl_secs).then(|| entry.checksum.clone())
    }

    /// Cache a freshly fetched canary channel checksum for `arch`.
    pub fn store_canary_checksum(&self, clock: &Clock, arch: Architecture, checksum: Sha256) {
        self.cache.insert(
            arch,
            CanaryChecksum {
                checksum,
                fetched: clock.timestamp(),
            },
        );
    }
}

struct CanaryChecksum {
    checksum: Sha256,
    fetched: i64,
}

#[expect(clippy::expect_used, reason = "constant URL literal, infallible")]
fn default_base_url(base_url: &str) -> Url {
    base_url.parse().expect("valid default update base URL")
}

fn ensure_trailing_slash(mut url: Url) -> Url {
    if !url.path().ends_with('/') {
        let path = format!("{path}/", path = url.path());
        url.set_path(&path);
    }
    url
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bencher_json::DateTime;
    use pretty_assertions::assert_eq;

    use super::*;

    fn test_checksum() -> Sha256 {
        "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
            .parse()
            .unwrap()
    }

    fn other_checksum() -> Sha256 {
        "b3a8e0e1f9ab1bfe3a36f231f676f78bb30a519d2b21e6c530c0eee8ebb4a5d0"
            .parse()
            .unwrap()
    }

    /// A clock frozen at `DateTime::TEST` plus the given offset in seconds.
    fn clock_at(offset_secs: i64) -> Clock {
        Clock::Custom(Arc::new(move || {
            DateTime::try_from(DateTime::TEST.timestamp() + offset_secs).unwrap()
        }))
    }

    #[test]
    fn stable_url_construction() {
        let update = RunnerUpdate::new(None);
        let url = update.stable_url("0.6.8", Architecture::X86_64).unwrap();
        assert_eq!(
            url.as_str(),
            "https://github.com/bencherdev/bencher/releases/download/v0.6.8/runner-v0.6.8-linux-x86-64"
        );
        let url = update.stable_url("0.6.8", Architecture::Aarch64).unwrap();
        assert_eq!(
            url.as_str(),
            "https://github.com/bencherdev/bencher/releases/download/v0.6.8/runner-v0.6.8-linux-arm-64"
        );
    }

    #[test]
    fn canary_url_construction() {
        let update = RunnerUpdate::new(None);
        let url = update.canary_url(Architecture::X86_64).unwrap();
        assert_eq!(
            url.as_str(),
            "https://download.bencher.dev/canary/runner-canary-linux-x86-64"
        );
        let url = update.canary_url(Architecture::Aarch64).unwrap();
        assert_eq!(
            url.as_str(),
            "https://download.bencher.dev/canary/runner-canary-linux-arm-64"
        );
    }

    #[test]
    fn custom_base_url_without_trailing_slash() {
        let base: Url = "http://localhost:8080/downloads".parse().unwrap();
        let update = RunnerUpdate::new(Some(base));
        let url = update.stable_url("0.6.8", Architecture::X86_64).unwrap();
        assert_eq!(
            url.as_str(),
            "http://localhost:8080/downloads/v0.6.8/runner-v0.6.8-linux-x86-64"
        );
        let url = update.canary_url(Architecture::X86_64).unwrap();
        assert_eq!(
            url.as_str(),
            "http://localhost:8080/downloads/canary/runner-canary-linux-x86-64"
        );
    }

    #[test]
    fn cache_miss_when_empty() {
        let update = RunnerUpdate::new(None);
        let clock = clock_at(0);
        assert_eq!(
            None,
            update.cached_canary_checksum(&clock, Architecture::X86_64)
        );
    }

    #[test]
    fn cache_hit_within_ttl() {
        let update = RunnerUpdate::new(None);
        let clock = clock_at(0);
        update.store_canary_checksum(&clock, Architecture::X86_64, test_checksum());

        let clock = clock_at(CANARY_CHECKSUM_TTL_SECS - 1);
        assert_eq!(
            Some(test_checksum()),
            update.cached_canary_checksum(&clock, Architecture::X86_64)
        );
    }

    #[test]
    fn cache_expires_at_ttl() {
        let update = RunnerUpdate::new(None);
        let clock = clock_at(0);
        update.store_canary_checksum(&clock, Architecture::X86_64, test_checksum());

        let clock = clock_at(CANARY_CHECKSUM_TTL_SECS);
        assert_eq!(
            None,
            update.cached_canary_checksum(&clock, Architecture::X86_64)
        );
    }

    #[test]
    fn cache_is_per_architecture() {
        let update = RunnerUpdate::new(None);
        let clock = clock_at(0);
        update.store_canary_checksum(&clock, Architecture::X86_64, test_checksum());
        update.store_canary_checksum(&clock, Architecture::Aarch64, other_checksum());

        assert_eq!(
            Some(test_checksum()),
            update.cached_canary_checksum(&clock, Architecture::X86_64)
        );
        assert_eq!(
            Some(other_checksum()),
            update.cached_canary_checksum(&clock, Architecture::Aarch64)
        );
    }
}
