//! OCI Distribution registry client.
//!
//! This module implements the OCI Distribution Specification for pulling images
//! from container registries. It supports Bearer token authentication.
//!
//! See: <https://github.com/opencontainers/distribution-spec/blob/main/spec.md>

use std::fs;
use std::time::{Duration, Instant, SystemTime};

use crate::digest::DigestHasher;
use crate::error::OciError;
use crate::oci_arch;
use crate::store::BlobStore;
use camino::{Utf8Path, Utf8PathBuf};

/// Media types for manifests.
const MANIFEST_MEDIA_TYPES: &[&str] = &[
    "application/vnd.oci.image.manifest.v1+json",
    "application/vnd.oci.image.index.v1+json",
    "application/vnd.docker.distribution.manifest.v2+json",
    "application/vnd.docker.distribution.manifest.list.v2+json",
];

pub use bencher_valid::ImageReference;

/// Bounds connecting to the registry, TLS handshake included, so a
/// black-holed host fails fast.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Bounds the wait for a response's headers, though not its body, so a
/// registry that never answers fails fast.
const RESPONSE_HEADERS_TIMEOUT: Duration = Duration::from_secs(60);

/// Select the best platform manifest from an index/manifest list.
///
/// Prefers `linux` + current architecture. Falls back to first manifest.
fn select_platform_manifest(
    manifests: &[bencher_json::oci::OciManifestDescriptor],
) -> Result<&bencher_json::oci::OciManifestDescriptor, OciError> {
    let first = manifests
        .first()
        .ok_or_else(|| OciError::Registry("Empty manifests array".to_owned()))?;

    let target_arch = oci_arch();

    // Find a manifest matching linux + current architecture
    let platform_match = manifests.iter().find(|m| {
        m.platform
            .as_ref()
            .is_some_and(|p| p.os == "linux" && p.architecture == target_arch)
    });

    // Fall back to first manifest if no platform match
    Ok(platform_match.unwrap_or(first))
}

/// Resolve the blob path for a given digest under the blobs directory.
///
/// OCI layout stores blobs at `blobs/<algorithm>/<hex>`.
fn blob_path_for_digest(blobs_base: &Utf8Path, digest: &str) -> Result<Utf8PathBuf, OciError> {
    let parsed: bencher_valid::ImageDigest = digest
        .parse()
        .map_err(|_err| OciError::InvalidReference(digest.to_owned()))?;
    let dir = blobs_base.join(parsed.algorithm());
    fs::create_dir_all(&dir)?;
    Ok(dir.join(parsed.hex_hash()))
}

/// OCI registry client for pulling images.
pub struct RegistryClient {
    /// HTTP agent.
    agent: ureq::Agent,

    /// URL scheme for registry requests.
    scheme: RegistryScheme,

    /// Base JWT token for authentication (provided at startup).
    base_token: Option<bencher_valid::Secret>,

    /// When the pull started and how long every request it makes may take in all.
    timeout: Option<(Instant, Duration)>,
}

impl RegistryClient {
    /// Create a new registry client without authentication.
    ///
    /// Requests to registries that require auth will fail with 401.
    pub fn new() -> Result<Self, OciError> {
        let config = ureq::Agent::config_builder()
            .user_agent("bencher-runner/1.0")
            .http_status_as_error(false)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_recv_response(Some(RESPONSE_HEADERS_TIMEOUT))
            .build();
        let agent = ureq::Agent::new_with_config(config);

        Ok(Self {
            agent,
            scheme: RegistryScheme::Https,
            base_token: None,
            timeout: None,
        })
    }

    /// Create a new registry client with a JWT for authentication.
    ///
    /// The token is sent directly as a Bearer token on all requests.
    /// If the registry rejects it (401), the request fails.
    pub fn with_token(token: &str) -> Result<Self, OciError> {
        let mut client = Self::new()?;
        client.base_token = Some(
            token
                .parse()
                .map_err(|e| OciError::Registry(format!("Invalid token: {e}")))?,
        );
        Ok(client)
    }

    /// Set the URL scheme for registry requests.
    #[must_use]
    pub fn with_scheme(mut self, scheme: RegistryScheme) -> Self {
        self.scheme = scheme;
        self
    }

    /// Bound the pull, from now, by `timeout`.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some((Instant::now(), timeout));
        self
    }

    /// Build a registry URL for the given path.
    fn registry_url(&self, registry: &str, path: &str) -> String {
        format!("{}://{registry}{path}", self.scheme.as_str())
    }

    /// Pull an image from a registry into an OCI layout at `output_dir`, which
    /// keeps the index and manifests, while the config and layer blobs go to
    /// `store`.
    ///
    /// The manifest always comes from the registry. A blob already in `store`
    /// is not downloaded again, and every blob the image uses is stamped.
    pub fn pull(
        &mut self,
        image_ref: &ImageReference,
        output_dir: &Utf8Path,
        store: &BlobStore,
    ) -> Result<PulledBlobs, OciError> {
        // Create output directory structure (sha256 dir always; sha512 created on demand)
        let blobs_base = output_dir.join("blobs");
        fs::create_dir_all(blobs_base.join("sha256"))?;

        // Write oci-layout file
        let layout_path = output_dir.join("oci-layout");
        fs::write(&layout_path, r#"{"imageLayoutVersion":"1.0.0"}"#)?;

        // Pull manifest
        let (manifest_digest, manifest_bytes) = self.pull_manifest(image_ref)?;

        // Save manifest blob
        let manifest_path = blob_path_for_digest(&blobs_base, &manifest_digest)?;
        fs::write(&manifest_path, &manifest_bytes)?;

        // Parse manifest to determine type and extract layers/config
        let parsed = bencher_json::oci::Manifest::from_bytes(&manifest_bytes)
            .map_err(|e| OciError::Registry(format!("Failed to parse manifest: {e}")))?;

        // If this is an index/manifest list, select the best platform manifest and pull it.
        // Update manifest_digest/manifest_bytes so index.json references the resolved
        // platform manifest, not the manifest list.
        let mut manifest_digest = manifest_digest;
        let mut manifest_bytes = manifest_bytes;
        let image_manifest = match &parsed {
            bencher_json::oci::Manifest::OciImageIndex(index) => {
                let desc = select_platform_manifest(&index.manifests)?;
                let (_, nested_bytes) = self.pull_manifest_by_digest(image_ref, &desc.digest)?;

                // Save nested manifest blob
                let nested_path = blob_path_for_digest(&blobs_base, &desc.digest)?;
                fs::write(&nested_path, &nested_bytes)?;

                let nested: bencher_json::oci::OciImageManifest =
                    serde_json::from_slice(&nested_bytes)?;
                manifest_digest.clone_from(&desc.digest);
                manifest_bytes = nested_bytes;
                nested
            },
            bencher_json::oci::Manifest::DockerManifestList(list) => {
                let desc = select_platform_manifest(&list.manifests)?;
                let (_, nested_bytes) = self.pull_manifest_by_digest(image_ref, &desc.digest)?;

                // Save nested manifest blob
                let nested_path = blob_path_for_digest(&blobs_base, &desc.digest)?;
                fs::write(&nested_path, &nested_bytes)?;

                let nested: bencher_json::oci::OciImageManifest =
                    serde_json::from_slice(&nested_bytes)?;
                manifest_digest.clone_from(&desc.digest);
                manifest_bytes = nested_bytes;
                nested
            },
            bencher_json::oci::Manifest::OciImageManifest(m) => m.clone(),
            bencher_json::oci::Manifest::DockerManifestV2(m) => {
                // Convert Docker V2 fields to OCI manifest fields
                bencher_json::oci::OciImageManifest {
                    schema_version: m.schema_version,
                    media_type: Some(m.media_type.clone()),
                    config: m.config.clone(),
                    layers: m.layers.clone(),
                    subject: None,
                    annotations: None,
                    artifact_type: None,
                }
            },
        };

        let mut pulled = PulledBlobs::default();
        let blobs = std::iter::once(&image_manifest.config).chain(&image_manifest.layers);
        for digest in blobs.map(|blob| blob.digest.as_str()) {
            let path = store.writable_path(digest)?;
            if path
                .symlink_metadata()
                .is_ok_and(|metadata| metadata.is_file())
            {
                pulled.reused += 1;
            } else {
                pulled.bytes += self.download_blob(image_ref, digest, &path)?;
                pulled.downloaded += 1;
            }
            stamp(&path)?;
        }

        // Write index.json
        let index = serde_json::json!({
            "schemaVersion": 2,
            "manifests": [{
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "digest": manifest_digest,
                "size": manifest_bytes.len()
            }]
        });
        let index_path = output_dir.join("index.json");
        fs::write(&index_path, serde_json::to_string_pretty(&index)?)?;

        Ok(pulled)
    }

    /// Pull an image manifest.
    fn pull_manifest(&self, image_ref: &ImageReference) -> Result<(String, Vec<u8>), OciError> {
        let url = self.registry_url(
            image_ref.registry(),
            &format!(
                "/v2/{}/manifests/{}",
                image_ref.repository(),
                image_ref.reference()
            ),
        );

        let accept = MANIFEST_MEDIA_TYPES.join(", ");

        let mut response = self.authenticated_request(&url, &accept)?;

        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok());

        let bytes = response
            .body_mut()
            .read_to_vec()
            .map_err(|e| registry_error("Failed to read manifest", &e))?;

        if let Some(expected_len) = content_length
            && bytes.len() != expected_len
        {
            return Err(OciError::Registry(format!(
                "Content-Length mismatch for manifest: expected {expected_len}, got {}",
                bytes.len()
            )));
        }

        // Compute digest: use the algorithm from the reference when pulling by digest,
        // otherwise default to SHA-256.
        let algorithm = if image_ref.is_digest() {
            let parsed: bencher_valid::ImageDigest = image_ref
                .reference()
                .parse()
                .map_err(|_err| OciError::InvalidReference(image_ref.reference().to_owned()))?;
            parsed.algorithm().to_owned()
        } else {
            "sha256".to_owned()
        };
        let computed_digest = DigestHasher::digest(&algorithm, &bytes)?;

        // Validate digest matches when pulling by digest
        if image_ref.is_digest() && computed_digest != image_ref.reference() {
            return Err(OciError::DigestMismatch {
                expected: image_ref.reference().to_owned(),
                actual: computed_digest,
            });
        }

        Ok((computed_digest, bytes))
    }

    fn pull_manifest_by_digest(
        &self,
        image_ref: &ImageReference,
        digest: &str,
    ) -> Result<(String, Vec<u8>), OciError> {
        let url = self.registry_url(
            image_ref.registry(),
            &format!("/v2/{}/manifests/{digest}", image_ref.repository()),
        );

        let accept = MANIFEST_MEDIA_TYPES.join(", ");
        let mut response = self.authenticated_request(&url, &accept)?;

        let bytes = response
            .body_mut()
            .read_to_vec()
            .map_err(|e| registry_error("Failed to read manifest", &e))?;

        let parsed: bencher_valid::ImageDigest = digest
            .parse()
            .map_err(|_err| OciError::InvalidReference(digest.to_owned()))?;
        let computed_digest = DigestHasher::digest(parsed.algorithm(), &bytes)?;
        if computed_digest != digest {
            return Err(OciError::DigestMismatch {
                expected: digest.to_owned(),
                actual: computed_digest,
            });
        }

        Ok((computed_digest, bytes))
    }

    /// Streams a blob to a temporary file beside `path`, and renames it into
    /// place only once its length and digest check out, so a file at a digest
    /// path is always the whole blob. Any failure removes the temporary file.
    fn download_blob(
        &self,
        image_ref: &ImageReference,
        digest: &str,
        path: &Utf8Path,
    ) -> Result<u64, OciError> {
        use std::io::{Read as _, Write as _};

        let url = self.registry_url(
            image_ref.registry(),
            &format!("/v2/{}/blobs/{digest}", image_ref.repository()),
        );

        let mut response = self.authenticated_request(&url, "*/*")?;

        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());

        let parsed: bencher_valid::ImageDigest = digest
            .parse()
            .map_err(|_err| OciError::InvalidReference(digest.to_owned()))?;

        let dir = path
            .parent()
            .ok_or_else(|| OciError::InvalidLayout(format!("No directory for blob {digest}")))?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        let mut hasher = DigestHasher::from_algorithm(parsed.algorithm())?;
        let mut total_bytes: u64 = 0;
        let mut buf = vec![0u8; 64 * 1024];

        let mut reader = response.body_mut().as_reader();
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| registry_error("Failed to read blob stream", &e.into()))?;
            if n == 0 {
                break;
            }
            let chunk = buf.get(..n).ok_or_else(|| {
                OciError::Registry("Read returned more bytes than buffer size".to_owned())
            })?;
            hasher.update(chunk);
            file.write_all(chunk)?;
            total_bytes += n as u64;
        }

        if let Some(expected_len) = content_length
            && total_bytes != expected_len
        {
            return Err(OciError::Registry(format!(
                "Content-Length mismatch for blob {digest}: expected {expected_len}, got {total_bytes}",
            )));
        }

        let computed = hasher.finalize();
        if computed != digest {
            return Err(OciError::DigestMismatch {
                expected: digest.to_owned(),
                actual: computed,
            });
        }

        file.as_file().sync_all()?;
        file.persist(path).map_err(std::io::Error::from)?;
        Ok(total_bytes)
    }

    /// Make an authenticated request to the registry.
    fn authenticated_request(
        &self,
        url: &str,
        accept: &str,
    ) -> Result<ureq::http::Response<ureq::Body>, OciError> {
        let mut request = self.agent.get(url).header("Accept", accept);

        if let Some(base_token) = &self.base_token {
            request = request.header(
                bencher_json::AUTHORIZATION,
                &bencher_json::bearer_header(base_token.as_ref()),
            );
        }

        let response = request
            .config()
            .timeout_global(self.remaining())
            .build()
            .call()
            .map_err(|e| registry_error("Request failed", &e))?;

        if !response.status().is_success() {
            return Err(OciError::Registry(format!(
                "Request failed with status: {}",
                response.status()
            )));
        }

        Ok(response)
    }

    /// What is left of the timeout, or none for one too large for ureq, which
    /// adds it to a later `Instant` of its own and panics past the clock's
    /// range, hence the headroom.
    fn remaining(&self) -> Option<Duration> {
        let (start, timeout) = self.timeout?;
        let remaining = timeout.saturating_sub(start.elapsed());
        Instant::now()
            .checked_add(remaining.saturating_mul(2))
            .map(|_| remaining)
    }
}

/// Marks a blob used now, in its modification time, since a `relatime` mount
/// does not keep access times.
fn stamp(path: &Utf8Path) -> std::io::Result<()> {
    fs::File::options()
        .write(true)
        .open(path)?
        .set_modified(SystemTime::now())
}

/// The config and layer blobs of one pull: those its store already held, and
/// those it downloaded.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PulledBlobs {
    pub reused: usize,
    pub downloaded: usize,
    /// The bytes of the blobs downloaded.
    pub bytes: u64,
}

/// Names a timeout as one, rather than as a generic registry failure.
fn registry_error(context: &str, error: &ureq::Error) -> OciError {
    if let ureq::Error::Timeout(timeout) = error {
        OciError::PullTimeout(*timeout)
    } else {
        OciError::Registry(format!("{context}: {error}"))
    }
}

/// URL scheme for registry connections.
#[derive(Debug, Default, Clone, Copy)]
pub enum RegistryScheme {
    /// HTTPS (default for production registries).
    #[default]
    Https,
    /// HTTP (for local/insecure registries).
    Http,
}

impl RegistryScheme {
    fn as_str(self) -> &'static str {
        match self {
            Self::Https => "https",
            Self::Http => "http",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Read as _, Write as _};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc::{self, TryRecvError};
    use std::thread::JoinHandle;

    use super::*;

    #[test]
    fn parse_simple_image() {
        let ref_ = ImageReference::parse("alpine").unwrap();
        assert_eq!(ref_.registry(), "docker.io");
        assert_eq!(ref_.repository(), "library/alpine");
        assert_eq!(ref_.reference(), "latest");
        assert!(!ref_.is_digest());
    }

    #[test]
    fn parse_image_with_tag() {
        let ref_ = ImageReference::parse("alpine:3.18").unwrap();
        assert_eq!(ref_.registry(), "docker.io");
        assert_eq!(ref_.repository(), "library/alpine");
        assert_eq!(ref_.reference(), "3.18");
        assert!(!ref_.is_digest());
    }

    #[test]
    fn parse_user_image() {
        let ref_ = ImageReference::parse("myuser/myimage:v1").unwrap();
        assert_eq!(ref_.registry(), "docker.io");
        assert_eq!(ref_.repository(), "myuser/myimage");
        assert_eq!(ref_.reference(), "v1");
    }

    #[test]
    fn parse_custom_registry() {
        let ref_ = ImageReference::parse("ghcr.io/owner/repo:latest").unwrap();
        assert_eq!(ref_.registry(), "ghcr.io");
        assert_eq!(ref_.repository(), "owner/repo");
        assert_eq!(ref_.reference(), "latest");
    }

    #[test]
    fn parse_registry_with_port() {
        let ref_ = ImageReference::parse("localhost:5000/myimage:v1").unwrap();
        assert_eq!(ref_.registry(), "localhost:5000");
        assert_eq!(ref_.repository(), "myimage");
        assert_eq!(ref_.reference(), "v1");
    }

    #[test]
    fn parse_digest() {
        let ref_ = ImageReference::parse("alpine@sha256:abc123").unwrap();
        assert_eq!(ref_.registry(), "docker.io");
        assert_eq!(ref_.repository(), "library/alpine");
        assert_eq!(ref_.reference(), "sha256:abc123");
        assert!(ref_.is_digest());
    }

    #[test]
    fn a_stalled_layer_download_times_out() {
        // Kills a layer stream the pull's timeout does not reach, which blocks
        // on a stalled registry for as long as it holds the connection open.
        let registry = FakeRegistry::start(Duration::ZERO, LayerResponse::Stalled);
        let output = tempfile::tempdir().unwrap();
        let output = Utf8Path::from_path(output.path()).unwrap();
        let started = Instant::now();

        let result = RegistryClient::new()
            .unwrap()
            .with_scheme(RegistryScheme::Http)
            .with_timeout(Duration::from_secs(2))
            .pull(&registry.image, output, &BlobStore::of_layout(output));

        let elapsed = started.elapsed();
        assert_eq!(
            registry.stop().layers,
            1,
            "the pull never reached the layer"
        );
        assert!(
            matches!(result, Err(OciError::PullTimeout(ureq::Timeout::Global))),
            "{result:?}"
        );
        assert!(elapsed < Duration::from_secs(30), "{elapsed:?}");
    }

    #[test]
    fn a_slow_pull_with_the_largest_timeout_completes() {
        // Kills handing ureq a timeout that overflows its `Instant`, which
        // panics, and a timeout that counts up from the start instead of down,
        // which runs out before the first slow response.
        let registry = FakeRegistry::start(Duration::from_millis(500), LayerResponse::Whole);
        let output = tempfile::tempdir().unwrap();
        let output = Utf8Path::from_path(output.path()).unwrap();

        let result = RegistryClient::new()
            .unwrap()
            .with_scheme(RegistryScheme::Http)
            .with_timeout(Duration::MAX)
            .pull(&registry.image, output, &BlobStore::of_layout(output));

        registry.stop();
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn every_request_draws_on_one_timeout() {
        // Kills giving each request the whole timeout, under which three
        // responses that each take two of its three seconds all arrive.
        let registry = FakeRegistry::start(Duration::from_secs(2), LayerResponse::Whole);
        let output = tempfile::tempdir().unwrap();
        let output = Utf8Path::from_path(output.path()).unwrap();

        let result = RegistryClient::new()
            .unwrap()
            .with_scheme(RegistryScheme::Http)
            .with_timeout(Duration::from_secs(3))
            .pull(&registry.image, output, &BlobStore::of_layout(output));

        registry.stop();
        assert!(
            matches!(result, Err(OciError::PullTimeout(ureq::Timeout::Global))),
            "{result:?}"
        );
    }

    #[test]
    fn a_layer_cut_off_partway_is_downloaded_again() {
        // Kills writing a blob in place, which leaves the cut-off layer at its
        // digest path for the next pull to take as whole, and leaving the
        // cut-off write behind.
        let (_dir, store) = store();
        let cut = FakeRegistry::start(Duration::ZERO, LayerResponse::Cut);

        let result = pull(&cut, &store);

        assert_eq!(cut.stop().layers, 1, "the pull never reached the layer");
        assert!(result.is_err(), "{result:?}");
        assert_eq!(stored(&store), [hex(CONFIG)]);

        let whole = FakeRegistry::start(Duration::ZERO, LayerResponse::Whole);
        let pulled = pull(&whole, &store).unwrap();

        assert_eq!(
            whole.stop(),
            Served {
                manifests: 1,
                configs: 0,
                layers: 1,
            }
        );
        assert_eq!(
            pulled,
            PulledBlobs {
                reused: 1,
                downloaded: 1,
                bytes: 5,
            }
        );
        assert_eq!(
            fs::read(store.blob_path(&digest(LAYER)).unwrap()).unwrap(),
            LAYER
        );
    }

    #[test]
    fn a_layer_that_misses_its_digest_leaves_nothing_at_its_path() {
        // Kills a rename ahead of the digest check, which keeps bytes under a
        // digest they do not have for every later pull to trust, and leaving
        // the refused write behind.
        let (_dir, store) = store();
        let corrupt = FakeRegistry::start(Duration::ZERO, LayerResponse::Corrupt);

        let result = pull(&corrupt, &store);

        assert_eq!(corrupt.stop().layers, 1, "the pull never reached the layer");
        assert!(
            matches!(result, Err(OciError::DigestMismatch { .. })),
            "{result:?}"
        );
        assert_eq!(stored(&store), [hex(CONFIG)]);
    }

    #[test]
    fn a_second_pull_downloads_no_blob_and_stamps_every_one() {
        // Kills downloading a blob the store holds, and a hit left unstamped,
        // which eviction then takes for a blob no Job has used since.
        let (_dir, store) = store();
        let registry = FakeRegistry::start(Duration::ZERO, LayerResponse::Whole);
        let first = pull(&registry, &store).unwrap();
        let blobs = [digest(CONFIG), digest(LAYER)].map(|digest| store.blob_path(&digest).unwrap());
        for blob in &blobs {
            stamp_at(blob, SystemTime::UNIX_EPOCH);
        }

        let second = pull(&registry, &store).unwrap();

        assert_eq!(
            registry.stop(),
            Served {
                manifests: 2,
                configs: 1,
                layers: 1,
            }
        );
        assert_eq!(
            (first, second),
            (
                PulledBlobs {
                    reused: 0,
                    downloaded: 2,
                    bytes: u64::try_from(CONFIG.len() + LAYER.len()).unwrap(),
                },
                PulledBlobs {
                    reused: 2,
                    downloaded: 0,
                    bytes: 0,
                }
            )
        );
        for blob in &blobs {
            let stamp = fs::metadata(blob).unwrap().modified().unwrap();
            assert!(stamp > SystemTime::UNIX_EPOCH, "{blob} was not stamped");
        }
    }

    #[test]
    fn only_a_file_at_a_digest_path_is_taken_for_its_blob() {
        // Kills taking whatever sits at a digest path for the blob: a link,
        // which hands every Job naming the digest the file it points at, or a
        // directory, on which each of those Jobs fails.
        use std::os::unix::fs::symlink;

        let (dir, store) = store();
        let layer = store.blob_path(&digest(LAYER)).unwrap();
        fs::create_dir_all(&layer).unwrap();
        let registry = FakeRegistry::start(Duration::ZERO, LayerResponse::Whole);
        let result = pull(&registry, &store);
        assert_eq!(
            registry.stop().layers,
            1,
            "a directory passed for the layer: {result:?}"
        );

        fs::remove_dir(&layer).unwrap();
        let elsewhere = Utf8Path::from_path(dir.path()).unwrap().join("elsewhere");
        fs::write(&elsewhere, b"not the layer").unwrap();
        symlink(&elsewhere, &layer).unwrap();
        let registry = FakeRegistry::start(Duration::ZERO, LayerResponse::Whole);
        pull(&registry, &store).unwrap();
        assert_eq!(registry.stop().layers, 1, "a link passed for the layer");
        assert!(layer.symlink_metadata().unwrap().is_file());
        assert_eq!(fs::read(&layer).unwrap(), LAYER);
        assert_eq!(fs::read(&elsewhere).unwrap(), b"not the layer");
    }

    #[test]
    fn a_pull_goes_below_no_level_that_is_not_the_stores_own() {
        // Kills a pull that writes through a link where an algorithm directory
        // belongs, or into a level somebody else may write, either of which
        // hands every later Job a blob somebody else could have put there.
        use std::os::unix::fs::{
            DirBuilderExt as _, MetadataExt as _, PermissionsExt as _, symlink,
        };

        let levels: [(&str, MakeLevel); 3] = [
            ("a linked algorithm directory", |blobs, theirs| {
                symlink(theirs, blobs.join("sha256")).unwrap();
            }),
            (
                "an algorithm directory others may write",
                |blobs, _theirs| {
                    fs::create_dir(blobs.join("sha256")).unwrap();
                    fs::set_permissions(blobs.join("sha256"), fs::Permissions::from_mode(0o777))
                        .unwrap();
                },
            ),
            ("a store others may write", |blobs, _theirs| {
                fs::set_permissions(blobs, fs::Permissions::from_mode(0o777)).unwrap();
            }),
        ];
        for (level, make) in levels {
            let dir = tempfile::tempdir().unwrap();
            let root = Utf8Path::from_path(dir.path()).unwrap();
            let blobs = root.join("blobs");
            fs::DirBuilder::new().mode(0o700).create(&blobs).unwrap();
            let store = BlobStore::private(blobs.clone(), fs::metadata(&blobs).unwrap().uid());
            let theirs = root.join("theirs");
            // Private, so only the link itself, never its target, can refuse it.
            fs::DirBuilder::new().mode(0o700).create(&theirs).unwrap();
            make(&blobs, &theirs);
            let registry = FakeRegistry::start(Duration::ZERO, LayerResponse::Whole);

            let result = pull(&registry, &store);

            let served = registry.stop();
            assert!(
                matches!(result, Err(OciError::ForeignBlobStore(_))),
                "{level}: {result:?}"
            );
            assert_eq!((served.configs, served.layers), (0, 0), "{level}");
            assert_eq!(fs::read_dir(&theirs).unwrap().count(), 0, "{level}");
        }
    }

    const CONFIG: &[u8] = br#"{"architecture":"amd64","os":"linux"}"#;
    const LAYER: &[u8] = b"layer";

    fn digest(bytes: &[u8]) -> String {
        DigestHasher::digest("sha256", bytes).unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        digest(bytes).trim_start_matches("sha256:").to_owned()
    }

    /// A store of its own, apart from the image layout each pull writes.
    fn store() -> (tempfile::TempDir, BlobStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new(Utf8Path::from_path(dir.path()).unwrap().join("store"));
        (dir, store)
    }

    /// Pulls the image into a layout of its own, with its blobs in `store`.
    #[expect(
        clippy::unwrap_in_result,
        reason = "the setup failing fails the test, while the pull's result is the test's"
    )]
    fn pull(registry: &FakeRegistry, store: &BlobStore) -> Result<PulledBlobs, OciError> {
        let output = tempfile::tempdir().unwrap();
        RegistryClient::new()
            .unwrap()
            .with_scheme(RegistryScheme::Http)
            .with_timeout(Duration::from_secs(30))
            .pull(
                &registry.image,
                Utf8Path::from_path(output.path()).unwrap(),
                store,
            )
    }

    /// Every file in the store's `sha256` directory, temporary or not.
    fn stored(store: &BlobStore) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(store.dir().join("sha256"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    fn stamp_at(path: &Utf8Path, stamp: SystemTime) {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(stamp)
            .unwrap();
    }

    /// A registry on localhost serving one image.
    struct FakeRegistry {
        image: ImageReference,
        release: mpsc::SyncSender<()>,
        server: JoinHandle<Served>,
    }

    /// What the fake registry was asked for.
    #[derive(Debug, Default, PartialEq, Eq)]
    struct Served {
        manifests: usize,
        configs: usize,
        layers: usize,
    }

    impl FakeRegistry {
        /// Answers each request `late`.
        fn start(late: Duration, layer_response: LayerResponse) -> Self {
            let config_digest = digest(CONFIG);
            let layer_digest = digest(LAYER);
            let manifest = serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "config": {
                    "mediaType": "application/vnd.oci.image.config.v1+json",
                    "digest": config_digest,
                    "size": CONFIG.len(),
                },
                "layers": [{
                    "mediaType": "application/vnd.oci.image.layer.v1.tar",
                    "digest": layer_digest,
                    "size": LAYER.len(),
                }],
            }))
            .unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let image =
                ImageReference::parse(&format!("{}/bench:v1", listener.local_addr().unwrap()))
                    .unwrap();
            let (release, released) = mpsc::sync_channel(0);
            let server = std::thread::spawn(move || {
                let mut served = Served::default();
                while let Err(TryRecvError::Empty) = released.try_recv() {
                    let mut stream = match listener.accept() {
                        Ok((stream, _)) => stream,
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(10));
                            continue;
                        },
                        Err(e) => panic!("accept failed: {e}"),
                    };
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(10)))
                        .unwrap();
                    let path = request_path(&mut stream);
                    std::thread::sleep(late);
                    let body = if path.ends_with(&layer_digest) {
                        served.layers += 1;
                        match layer_response {
                            LayerResponse::Stalled => {
                                write!(
                                    stream,
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                                    LAYER.len() + 1024
                                )
                                .unwrap();
                                stream.write_all(LAYER).unwrap();
                                // Silent, with the connection open, until the test is done.
                                released.recv().unwrap_err();
                                continue;
                            },
                            LayerResponse::Cut => {
                                write!(
                                    stream,
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                                    LAYER.len()
                                )
                                .unwrap();
                                stream.write_all(&LAYER[..2]).unwrap();
                                // Closed, partway through the body.
                                continue;
                            },
                            LayerResponse::Corrupt => {
                                let mut layer = LAYER.to_vec();
                                *layer.last_mut().unwrap() ^= 1;
                                layer
                            },
                            LayerResponse::Whole => LAYER.to_vec(),
                        }
                    } else if path.ends_with(&config_digest) {
                        served.configs += 1;
                        CONFIG.to_vec()
                    } else {
                        served.manifests += 1;
                        manifest.clone()
                    };
                    let mut response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .into_bytes();
                    response.extend_from_slice(&body);
                    // A client that has run out of time has already hung up.
                    drop(stream.write_all(&response));
                }
                served
            });
            Self {
                image,
                release,
                server,
            }
        }

        /// Shuts the registry down and returns what it was asked for.
        fn stop(self) -> Served {
            let Self {
                image: _,
                release,
                server,
            } = self;
            drop(release);
            server.join().unwrap()
        }
    }

    /// How the fake registry answers for the layer.
    enum LayerResponse {
        /// A body it never finishes, on a connection it holds open.
        Stalled,
        /// A body it closes the connection on partway through.
        Cut,
        /// The whole layer with one byte flipped.
        Corrupt,
        /// The whole layer.
        Whole,
    }

    /// Reads a request's head and returns its path.
    fn request_path(stream: &mut TcpStream) -> String {
        let mut head = Vec::new();
        let mut byte = [0];
        while !head.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            head.extend_from_slice(&byte);
        }
        let head = String::from_utf8(head).unwrap();
        head.split(' ').nth(1).unwrap().to_owned()
    }

    /// Makes one level of a store in `blobs` that is not the store's own, its
    /// victim the directory a link points at.
    type MakeLevel = fn(&Utf8Path, &Utf8Path);
}
