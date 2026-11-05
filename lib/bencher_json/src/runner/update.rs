use bencher_valid::Architecture;
use url::Url;

/// Base URL for canary channel runner downloads.
// Trailing slash is required: `Url::join` appends to the last path segment, so
// without it the join would replace that segment instead of extending it.
pub const DEFAULT_CANARY_BASE_URL: &str = "https://download.bencher.dev/";

/// The canary channel's name: the path segment that holds its artifacts, and the
/// version component of each artifact's file name.
const CANARY: &str = "canary";

/// File name of the canary channel runner binary for `arch`.
pub fn canary_artifact(arch: Architecture) -> String {
    format!("runner-{CANARY}-{slug}", slug = arch.linux_artifact_slug())
}

/// Download URL of the canary channel runner binary for `arch` under `base_url`.
/// The checksum published beside it is the same URL with a `.sha256` suffix.
pub fn canary_url(base_url: &Url, arch: Architecture) -> Result<Url, url::ParseError> {
    base_url.join(&format!(
        "{CANARY}/{artifact}",
        artifact = canary_artifact(arch)
    ))
}
