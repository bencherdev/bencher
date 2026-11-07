use std::fmt;

use anyhow::Context as _;
use serde::Deserialize;

/// One entry of the Firecracker releases feed.
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    prerelease: bool,
}

/// The releases the task chooses between.
///
/// Without a minor, that is every release of the major: a Firecracker minor
/// keeps the API it was given, so the newest of them is always safe to take.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    major: u64,
    minor: Option<u64>,
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { major, minor } = self;
        match minor {
            Some(minor) => write!(f, "v{major}.{minor}"),
            None => write!(f, "v{major}"),
        }
    }
}

impl Line {
    pub fn new(pinned_tag: &str, override_line: Option<&str>) -> anyhow::Result<Self> {
        if let Some(line) = override_line {
            let (major, minor) = parse_line(line).with_context(|| {
                format!("expected a Firecracker major.minor line such as `1.16`, got `{line}`")
            })?;
            return Ok(Self {
                major,
                minor: Some(minor),
            });
        }

        let (major, _, _) = parse_tag(pinned_tag).with_context(|| {
            format!("failed to parse the pinned Firecracker tag `{pinned_tag}`")
        })?;
        Ok(Self { major, minor: None })
    }
}

#[derive(Debug)]
pub struct Selection {
    pub tag: String,
    pub newer_major: Option<NewerMajor>,
}

/// A release past the line's major, which only a human takes.
#[derive(Debug)]
pub struct NewerMajor {
    tag: String,
    /// What `--firecracker-version` takes to move the pin onto that release.
    line: String,
}

impl NewerMajor {
    pub fn notice(&self, tracked: Line) -> String {
        let Self { tag, line } = self;
        format!(
            "Firecracker {tag} is out, past the {tracked} line this task tracks. A new major may change the API, so take it by hand: `cargo update-sandbox --firecracker-version {line}`"
        )
    }
}

pub fn latest(line: Line) -> anyhow::Result<Selection> {
    let releases = fetch_releases()?;
    select(&releases, line)
}

/// The newest release of the line, and the newest release above its major.
fn select(releases: &[Release], line: Line) -> anyhow::Result<Selection> {
    let Line { major, minor } = line;

    let versioned = || {
        releases
            .iter()
            .filter(|release| !release.prerelease)
            .filter_map(|release| {
                parse_tag(&release.tag_name).map(|version| (version, release.tag_name.as_str()))
            })
    };

    let tag = versioned()
        .filter(|&((release_major, release_minor, _), _)| {
            release_major == major && minor.is_none_or(|minor| release_minor == minor)
        })
        .max_by_key(|&(version, _)| version)
        .map(|(_, tag)| tag.to_owned())
        .with_context(|| format!("no Firecracker release found in the {line} line"))?;

    let newer_major = versioned()
        .filter(|&((release_major, _, _), _)| release_major > major)
        .max_by_key(|&(version, _)| version)
        .map(|((release_major, release_minor, _), tag)| NewerMajor {
            tag: tag.to_owned(),
            line: format!("{release_major}.{release_minor}"),
        });

    Ok(Selection { tag, newer_major })
}

pub struct Checksums {
    pub x86_64: String,
    pub aarch64: String,
}

pub fn checksums(tag: &str) -> anyhow::Result<Checksums> {
    Ok(Checksums {
        x86_64: download_sha256(tag, "x86_64")?,
        aarch64: download_sha256(tag, "aarch64")?,
    })
}

/// The feed leads with the newest release, so page 1 holds the newest of every
/// live major. A major too quiet for page 1 fails the task rather than pick
/// an older release.
fn fetch_releases() -> anyhow::Result<Vec<Release>> {
    let mut request = ureq::get(
        "https://api.github.com/repos/firecracker-microvm/firecracker/releases?per_page=100",
    )
    .header("Accept", "application/vnd.github+json")
    .header("User-Agent", "bencher-update-sandbox");

    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }

    let body = request
        .call()
        .context("failed to fetch Firecracker releases")?
        .body_mut()
        .read_to_string()
        .context("failed to read releases body")?;

    serde_json::from_str(&body).context("failed to parse releases JSON")
}

fn download_sha256(tag: &str, arch: &str) -> anyhow::Result<String> {
    let url = format!(
        "https://github.com/firecracker-microvm/firecracker/releases/download/{tag}/firecracker-{tag}-{arch}.tgz.sha256.txt"
    );

    let body = ureq::get(&url)
        .header("User-Agent", "bencher-update-sandbox")
        .call()
        .with_context(|| format!("failed to download SHA256 for {tag} {arch}"))?
        .body_mut()
        .read_to_string()
        .with_context(|| format!("failed to read SHA256 body for {tag} {arch}"))?;

    // Format: "{hash}  {filename}\n"
    let hash = body
        .split_whitespace()
        .next()
        .with_context(|| format!("empty SHA256 file for {tag} {arch}"))?;

    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("invalid SHA256 hash for {tag} {arch}: {hash}");
    }

    Ok(hash.to_owned())
}

/// A tag carrying anything more is no release of a line: a suffix names a
/// candidate for one, and a part too large to hold names nothing at all.
fn parse_tag(tag: &str) -> Option<(u64, u64, u64)> {
    let mut parts = tag.strip_prefix('v')?.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some((major, minor, patch))
}

fn parse_line(line: &str) -> Option<(u64, u64)> {
    let mut parts = line.strip_prefix('v').unwrap_or(line).split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some((major, minor))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn releases(tags: &[(&str, bool)]) -> Vec<Release> {
        tags.iter()
            .map(|&(tag_name, prerelease)| Release {
                tag_name: tag_name.to_owned(),
                prerelease,
            })
            .collect()
    }

    fn line(override_line: Option<&str>) -> Line {
        Line::new("v1.15.1", override_line).unwrap()
    }

    fn newer_major(selection: &Selection) -> Option<(&str, &str)> {
        selection
            .newer_major
            .as_ref()
            .map(|newer| (newer.tag.as_str(), newer.line.as_str()))
    }

    #[test]
    fn the_newest_minor_of_the_pinned_major_wins() {
        // v1.16.2 shipped the same day as v1.17.0 and can lead the feed, so the pick
        // cannot follow the feed's order, nor rank a release by its patch number.
        let selection = select(
            &releases(&[("v1.16.2", false), ("v1.17.0", false), ("v1.15.1", false)]),
            line(None),
        )
        .unwrap();

        assert_eq!(selection.tag, "v1.17.0");
        assert_eq!(newer_major(&selection), None);
    }

    #[test]
    fn a_prerelease_never_wins() {
        let selection = select(
            &releases(&[("v1.18.0", true), ("v1.17.0", false)]),
            line(None),
        )
        .unwrap();

        assert_eq!(selection.tag, "v1.17.0");
    }

    #[test]
    fn a_newer_major_is_reported_and_never_taken() {
        // A pick blind to the major takes v2.1.0, which may change the API.
        let selection = select(
            &releases(&[
                ("v3.0.0", true),
                ("v2.1.0", false),
                ("v2.0.0", false),
                ("v1.17.0", false),
                ("v0.25.2", false),
            ]),
            line(None),
        )
        .unwrap();

        assert_eq!(selection.tag, "v1.17.0");
        assert_eq!(newer_major(&selection), Some(("v2.1.0", "2.1")));
    }

    #[test]
    fn an_override_holds_the_line_it_names() {
        let releases = releases(&[("v1.17.0", false), ("v1.16.2", false), ("v1.16.1", false)]);

        for named in ["1.16", "v1.16"] {
            let selection = select(&releases, line(Some(named))).unwrap();

            assert_eq!(selection.tag, "v1.16.2");
            assert_eq!(newer_major(&selection), None);
        }
    }

    #[test]
    fn an_override_takes_the_major_the_task_will_not() {
        let selection = select(
            &releases(&[("v2.0.0", false), ("v1.17.0", false)]),
            line(Some("2.0")),
        )
        .unwrap();

        assert_eq!(selection.tag, "v2.0.0");
        assert_eq!(newer_major(&selection), None);
    }

    #[test]
    fn a_tag_that_is_not_a_version_neither_wins_nor_panics() {
        // A parse that drops the parts it cannot read lets `v1.18.0-rc1` and
        // `firecracker-v1.19.0` outrank v1.17.0. The rest name no release at all.
        let selection = select(
            &releases(&[
                ("v1.18.0-rc1", false),
                ("v1.99999999999999999999.0", false),
                ("firecracker-v1.19.0", false),
                ("v1.18.0.1", false),
                ("v1.17", false),
                ("v1.17.0", false),
            ]),
            line(None),
        )
        .unwrap();

        assert_eq!(selection.tag, "v1.17.0");
        assert_eq!(newer_major(&selection), None);
    }

    #[test]
    fn a_line_with_no_release_is_an_error() {
        let pinned_past_the_feed = Line::new("v9.0.0", None).unwrap();

        let error = select(&releases(&[("v1.17.0", false)]), pinned_past_the_feed).unwrap_err();

        assert_eq!(
            error.to_string(),
            "no Firecracker release found in the v9 line"
        );
    }

    #[test]
    fn the_notice_names_the_release_and_the_command_that_takes_it() {
        let selection = select(
            &releases(&[("v2.1.0", false), ("v1.17.0", false)]),
            line(None),
        )
        .unwrap();

        let notice = selection.newer_major.unwrap().notice(line(None));

        assert!(notice.contains("v2.1.0"), "{notice}");
        assert!(notice.contains("--firecracker-version 2.1"), "{notice}");
    }

    #[test]
    fn a_line_that_cannot_be_read_is_refused() {
        Line::new("v1.15.1", Some("1.16.2")).unwrap_err();
        Line::new("v1.15.1", Some("latest")).unwrap_err();
        Line::new("1.15.1", None).unwrap_err();
    }
}
