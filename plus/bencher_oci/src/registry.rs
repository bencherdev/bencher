//! OCI Distribution registry client.
//!
//! This module implements the OCI Distribution Specification for pulling images
//! from container registries. It supports Bearer token authentication.
//!
//! See: <https://github.com/opencontainers/distribution-spec/blob/main/spec.md>

use std::fs;
use std::time::{Duration, Instant};

use crate::digest::DigestHasher;
use crate::error::OciError;
use crate::oci_arch;
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

    /// Pull an image from a registry and save it in OCI layout format.
    pub fn pull(
        &mut self,
        image_ref: &ImageReference,
        output_dir: &Utf8Path,
    ) -> Result<Utf8PathBuf, OciError> {
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

        // Pull config blob
        let config_digest = &image_manifest.config.digest;
        let (_, config_bytes) = self.pull_blob(image_ref, config_digest)?;
        let config_path = blob_path_for_digest(&blobs_base, config_digest)?;
        fs::write(&config_path, &config_bytes)?;

        // Pull layer blobs
        for layer in &image_manifest.layers {
            let layer_digest = &layer.digest;
            let layer_path = blob_path_for_digest(&blobs_base, layer_digest)?;

            // Skip if already downloaded
            if layer_path.exists() {
                continue;
            }

            self.pull_blob_to_file(image_ref, layer_digest, &layer_path)?;
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

        Ok(output_dir.to_owned())
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

        // Compute digest — use the algorithm from the reference when pulling by digest,
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

    /// Pull a blob from the registry into memory.
    fn pull_blob(
        &self,
        image_ref: &ImageReference,
        digest: &str,
    ) -> Result<(String, Vec<u8>), OciError> {
        let url = self.registry_url(
            image_ref.registry(),
            &format!("/v2/{}/blobs/{digest}", image_ref.repository()),
        );

        let mut response = self.authenticated_request(&url, "*/*")?;

        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok());

        let bytes = response
            .body_mut()
            .read_to_vec()
            .map_err(|e| registry_error("Failed to read blob", &e))?;

        if let Some(expected_len) = content_length
            && bytes.len() != expected_len
        {
            return Err(OciError::Registry(format!(
                "Content-Length mismatch for blob {digest}: expected {expected_len}, got {}",
                bytes.len()
            )));
        }

        // Verify digest
        let parsed: bencher_valid::ImageDigest = digest
            .parse()
            .map_err(|_err| OciError::InvalidReference(digest.to_owned()))?;
        let computed = DigestHasher::digest(parsed.algorithm(), &bytes)?;

        if computed != digest {
            return Err(OciError::DigestMismatch {
                expected: digest.to_owned(),
                actual: computed,
            });
        }

        Ok((digest.to_owned(), bytes))
    }

    /// Pull a blob from the registry and stream it directly to a file.
    ///
    /// This avoids loading entire layer blobs into memory.
    fn pull_blob_to_file(
        &self,
        image_ref: &ImageReference,
        digest: &str,
        output_path: &Utf8Path,
    ) -> Result<String, OciError> {
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

        let mut file = fs::File::create(output_path)?;
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

        Ok(computed)
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
        let started = Instant::now();

        let result = RegistryClient::new()
            .unwrap()
            .with_scheme(RegistryScheme::Http)
            .with_timeout(Duration::from_secs(2))
            .pull(&registry.image, Utf8Path::from_path(output.path()).unwrap());

        let elapsed = started.elapsed();
        assert!(registry.stop(), "the pull never reached the layer");
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

        let result = RegistryClient::new()
            .unwrap()
            .with_scheme(RegistryScheme::Http)
            .with_timeout(Duration::MAX)
            .pull(&registry.image, Utf8Path::from_path(output.path()).unwrap());

        registry.stop();
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn every_request_draws_on_one_timeout() {
        // Kills giving each request the whole timeout, under which three
        // responses that each take two of its three seconds all arrive.
        let registry = FakeRegistry::start(Duration::from_secs(2), LayerResponse::Whole);
        let output = tempfile::tempdir().unwrap();

        let result = RegistryClient::new()
            .unwrap()
            .with_scheme(RegistryScheme::Http)
            .with_timeout(Duration::from_secs(3))
            .pull(&registry.image, Utf8Path::from_path(output.path()).unwrap());

        registry.stop();
        assert!(
            matches!(result, Err(OciError::PullTimeout(ureq::Timeout::Global))),
            "{result:?}"
        );
    }

    /// A registry on localhost serving one image.
    struct FakeRegistry {
        image: ImageReference,
        release: mpsc::SyncSender<()>,
        server: JoinHandle<bool>,
    }

    impl FakeRegistry {
        /// Answers each request `late`.
        fn start(late: Duration, layer_response: LayerResponse) -> Self {
            let config = br#"{"architecture":"amd64","os":"linux"}"#.to_vec();
            let layer = b"layer".to_vec();
            let config_digest = DigestHasher::digest("sha256", &config).unwrap();
            let layer_digest = DigestHasher::digest("sha256", &layer).unwrap();
            let manifest = serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "config": {
                    "mediaType": "application/vnd.oci.image.config.v1+json",
                    "digest": config_digest,
                    "size": config.len(),
                },
                "layers": [{
                    "mediaType": "application/vnd.oci.image.layer.v1.tar",
                    "digest": layer_digest,
                    "size": layer.len(),
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
                let mut stalled = false;
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
                    let is_layer = path.ends_with(&layer_digest);
                    if is_layer && matches!(layer_response, LayerResponse::Stalled) {
                        write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                            layer.len() + 1024
                        )
                        .unwrap();
                        stream.write_all(&layer).unwrap();
                        stalled = true;
                        // Silent, with the connection open, until the test is done.
                        released.recv().unwrap_err();
                        continue;
                    }
                    let body = if is_layer {
                        &layer
                    } else if path.ends_with(&config_digest) {
                        &config
                    } else {
                        &manifest
                    };
                    let mut response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .into_bytes();
                    response.extend_from_slice(body);
                    // A client that has run out of time has already hung up.
                    drop(stream.write_all(&response));
                }
                stalled
            });
            Self {
                image,
                release,
                server,
            }
        }

        /// Shuts the registry down and returns whether it stalled a layer.
        fn stop(self) -> bool {
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
}
