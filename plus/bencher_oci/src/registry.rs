//! OCI Distribution registry client.
//!
//! This module implements the OCI Distribution Specification for pulling images
//! from container registries. It supports Docker-style token authentication.
//!
//! See: <https://github.com/opencontainers/distribution-spec/blob/main/spec.md>

use std::collections::HashMap;
use std::fs;

use crate::digest::DigestHasher;
use crate::error::OciError;
use crate::oci_arch;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

/// Media types for manifests.
const MANIFEST_MEDIA_TYPES: &[&str] = &[
    "application/vnd.oci.image.manifest.v1+json",
    "application/vnd.oci.image.index.v1+json",
    "application/vnd.docker.distribution.manifest.v2+json",
    "application/vnd.docker.distribution.manifest.list.v2+json",
];

pub use bencher_valid::ImageReference;

/// Token response from the registry authentication service.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    token: Option<String>,
    access_token: Option<String>,
    // expires_in is included in the response but not currently used
    #[serde(default)]
    #[expect(dead_code)]
    expires_in: Option<u64>,
}

impl TokenResponse {
    fn token(&self) -> Option<&str> {
        self.token.as_deref().or(self.access_token.as_deref())
    }
}

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

    /// Base JWT token for authentication (provided at startup).
    base_token: Option<bencher_valid::Secret>,

    /// Cached bearer tokens per scope.
    bearer_tokens: HashMap<String, String>,
}

impl RegistryClient {
    /// Create a new registry client.
    pub fn new() -> Result<Self, OciError> {
        let config = ureq::Agent::config_builder()
            .user_agent("bencher-runner/1.0")
            .http_status_as_error(false)
            .build();
        let agent = ureq::Agent::new_with_config(config);

        Ok(Self {
            agent,
            base_token: None,
            bearer_tokens: HashMap::new(),
        })
    }

    /// Create a new registry client with a JWT token for authentication.
    pub fn with_token(token: &str) -> Result<Self, OciError> {
        let mut client = Self::new()?;
        client.base_token = Some(
            token
                .parse()
                .map_err(|e| OciError::Registry(format!("Invalid token: {e}")))?,
        );
        Ok(client)
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
                let (_, nested_bytes) = self.pull_blob(image_ref, &desc.digest)?;

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
                let (_, nested_bytes) = self.pull_blob(image_ref, &desc.digest)?;

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
    fn pull_manifest(&mut self, image_ref: &ImageReference) -> Result<(String, Vec<u8>), OciError> {
        let url = format!(
            "https://{}/v2/{}/manifests/{}",
            image_ref.registry(),
            image_ref.repository(),
            image_ref.reference()
        );

        let accept = MANIFEST_MEDIA_TYPES.join(", ");

        let mut response = self.authenticated_request(&url, image_ref, &accept)?;

        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok());

        let bytes = response
            .body_mut()
            .read_to_vec()
            .map_err(|e| OciError::Registry(format!("Failed to read manifest: {e}")))?;

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

    /// Pull a blob from the registry into memory.
    fn pull_blob(
        &mut self,
        image_ref: &ImageReference,
        digest: &str,
    ) -> Result<(String, Vec<u8>), OciError> {
        let url = format!(
            "https://{}/v2/{}/blobs/{digest}",
            image_ref.registry(),
            image_ref.repository()
        );

        let mut response = self.authenticated_request(&url, image_ref, "*/*")?;

        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok());

        let bytes = response
            .body_mut()
            .read_to_vec()
            .map_err(|e| OciError::Registry(format!("Failed to read blob: {e}")))?;

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
        &mut self,
        image_ref: &ImageReference,
        digest: &str,
        output_path: &Utf8Path,
    ) -> Result<String, OciError> {
        use std::io::{Read as _, Write as _};

        let url = format!(
            "https://{}/v2/{}/blobs/{digest}",
            image_ref.registry(),
            image_ref.repository()
        );

        let mut response = self.authenticated_request(&url, image_ref, "*/*")?;

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
                .map_err(|e| OciError::Registry(format!("Failed to read blob stream: {e}")))?;
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
        &mut self,
        url: &str,
        image_ref: &ImageReference,
        accept: &str,
    ) -> Result<ureq::http::Response<ureq::Body>, OciError> {
        // Build the scope for this request
        let scope = format!("repository:{}:pull", image_ref.repository());

        // Build request with cached token if available
        let mut request = self.agent.get(url).header("Accept", accept);

        if let Some(token) = self.bearer_tokens.get(&scope) {
            request = request.header(
                bencher_json::AUTHORIZATION,
                &bencher_json::bearer_header(token),
            );
        }

        let response = request
            .call()
            .map_err(|e| OciError::Registry(format!("Request failed: {e}")))?;

        // If unauthorized, get a token and retry
        if response.status() == 401 {
            let www_auth = response
                .headers()
                .get("www-authenticate")
                .and_then(|h| h.to_str().ok())
                .ok_or_else(|| OciError::Registry("Missing WWW-Authenticate header".to_owned()))?
                .to_owned();

            let token = self.get_token(&www_auth, &scope)?;
            self.bearer_tokens.insert(scope.clone(), token.clone());

            // Retry with token
            let response = self
                .agent
                .get(url)
                .header("Accept", accept)
                .header(
                    bencher_json::AUTHORIZATION,
                    &bencher_json::bearer_header(&token),
                )
                .call()
                .map_err(|e| OciError::Registry(format!("Request failed: {e}")))?;

            if !response.status().is_success() {
                return Err(OciError::Registry(format!(
                    "Request failed with status: {}",
                    response.status()
                )));
            }

            return Ok(response);
        }

        if !response.status().is_success() {
            return Err(OciError::Registry(format!(
                "Request failed with status: {}",
                response.status()
            )));
        }

        Ok(response)
    }

    /// Get a bearer token from the authentication service.
    fn get_token(&self, www_auth: &str, scope: &str) -> Result<String, OciError> {
        // Parse WWW-Authenticate header
        let params = Self::parse_www_authenticate(www_auth);

        let realm = params
            .get("realm")
            .ok_or_else(|| OciError::Registry("Missing realm in WWW-Authenticate".to_owned()))?;

        let service = params.get("service").map_or("", String::as_str);

        // Build token request URL
        let token_url = format!("{realm}?service={service}&scope={scope}");

        // Make token request
        let mut request = self.agent.get(&token_url);

        // If we have a base token, use it as Basic auth password
        if let Some(base_token) = &self.base_token {
            let credentials = BASE64_STANDARD.encode(format!("_token:{}", base_token.as_ref()));
            request = request.header(bencher_json::AUTHORIZATION, &format!("Basic {credentials}"));
        }

        let response = request
            .call()
            .map_err(|e| OciError::Registry(format!("Token request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.into_body().read_to_string().unwrap_or_default();
            return Err(OciError::Registry(format!(
                "Token request failed with status {status}: {body}"
            )));
        }

        let token_response: TokenResponse = response
            .into_body()
            .read_json()
            .map_err(|e| OciError::Registry(format!("Failed to parse token response: {e}")))?;

        token_response
            .token()
            .map(ToOwned::to_owned)
            .ok_or_else(|| OciError::Registry("No token in response".to_owned()))
    }

    /// Parse WWW-Authenticate header into key-value pairs.
    fn parse_www_authenticate(header: &str) -> HashMap<String, String> {
        let mut params = HashMap::new();

        // Skip "Bearer " prefix
        let content = header.strip_prefix("Bearer ").unwrap_or(header);

        // Parse key="value" pairs
        for part in content.split(',') {
            let part = part.trim();
            if let Some((key, value)) = part.split_once('=') {
                let value = value.trim_matches('"');
                params.insert(key.to_owned(), value.to_owned());
            }
        }

        params
    }
}

#[cfg(test)]
mod tests {
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
    fn parse_www_authenticate_header() {
        let header = r#"Bearer realm="https://auth.docker.io/token",service="registry.docker.io",scope="repository:library/alpine:pull""#;
        let params = RegistryClient::parse_www_authenticate(header);

        assert_eq!(
            params.get("realm"),
            Some(&"https://auth.docker.io/token".to_owned())
        );
        assert_eq!(
            params.get("service"),
            Some(&"registry.docker.io".to_owned())
        );
    }
}
