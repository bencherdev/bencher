//! OCI Distribution registry client.
//!
//! This module implements the OCI Distribution Specification for pulling images
//! from container registries. It supports Docker-style token authentication.
//!
//! See: <https://github.com/opencontainers/distribution-spec/blob/main/spec.md>

use std::collections::HashMap;
use std::fs;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use crate::error::OciError;

/// Media types for manifests.
const MANIFEST_MEDIA_TYPES: &[&str] = &[
    "application/vnd.oci.image.manifest.v1+json",
    "application/vnd.oci.image.index.v1+json",
    "application/vnd.docker.distribution.manifest.v2+json",
    "application/vnd.docker.distribution.manifest.list.v2+json",
];

/// A parsed OCI image reference.
#[derive(Debug, Clone)]
pub struct ImageReference {
    /// Registry host (e.g., "registry.example.com", "docker.io").
    pub registry: String,

    /// Repository name (e.g., "library/alpine", "myuser/myimage").
    pub repository: String,

    /// Tag or digest (e.g., "latest", "sha256:abc123...").
    pub reference: String,

    /// Whether the reference is a digest.
    pub is_digest: bool,
}

impl ImageReference {
    /// Parse an image reference string.
    ///
    /// Formats supported:
    /// - `image` -> docker.io/library/image:latest
    /// - `image:tag` -> docker.io/library/image:tag
    /// - `user/image` -> docker.io/user/image:latest
    /// - `registry.com/image` -> registry.com/image:latest
    /// - `registry.com/user/image:tag` -> registry.com/user/image:tag
    /// - `registry.com/image@sha256:...` -> registry.com/image@sha256:...
    pub fn parse(reference: &str) -> Result<Self, OciError> {
        let (name, tag_or_digest, is_digest) =
            if let Some((name, digest)) = reference.split_once('@') {
                (name, digest.to_owned(), true)
            } else if let Some((name, tag)) = reference.rsplit_once(':') {
                // Check if the colon is part of a port number
                if name.contains('/') || !tag.chars().all(|c| c.is_ascii_digit()) {
                    (name, tag.to_owned(), false)
                } else {
                    // It's a port, not a tag
                    (reference, "latest".to_owned(), false)
                }
            } else {
                (reference, "latest".to_owned(), false)
            };

        // Parse registry and repository
        let (registry, repository) = Self::parse_name(name);

        Ok(Self {
            registry,
            repository,
            reference: tag_or_digest,
            is_digest,
        })
    }

    /// Parse the name portion into registry and repository.
    fn parse_name(name: &str) -> (String, String) {
        let parts: Vec<&str> = name.splitn(2, '/').collect();

        match parts.as_slice() {
            [image] => {
                // Just image name: docker.io/library/image
                ("docker.io".to_owned(), format!("library/{image}"))
            },
            [first, rest]
                if first.contains('.') || first.contains(':') || *first == "localhost" =>
            {
                // Has a registry prefix
                ((*first).to_owned(), (*rest).to_owned())
            },
            _ => {
                // user/image format: docker.io/user/image
                ("docker.io".to_owned(), name.to_owned())
            },
        }
    }

    /// Get the full image name for display.
    #[must_use]
    pub fn full_name(&self) -> String {
        let sep = if self.is_digest { "@" } else { ":" };
        format!(
            "{}/{}{}{}",
            self.registry, self.repository, sep, self.reference
        )
    }
}

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
        client.base_token = token
            .parse()
            .map_err(|e| OciError::Registry(format!("Invalid token: {e}")))
            .ok();
        Ok(client)
    }

    /// Pull an image from a registry and save it in OCI layout format.
    pub fn pull(
        &mut self,
        image_ref: &ImageReference,
        output_dir: &Utf8Path,
    ) -> Result<Utf8PathBuf, OciError> {
        // Create output directory structure
        let blobs_dir = output_dir.join("blobs").join("sha256");
        fs::create_dir_all(&blobs_dir)?;

        // Write oci-layout file
        let layout_path = output_dir.join("oci-layout");
        fs::write(&layout_path, r#"{"imageLayoutVersion":"1.0.0"}"#)?;

        // Pull manifest
        let (manifest_digest, manifest_bytes) = self.pull_manifest(image_ref)?;

        // Save manifest blob
        let manifest_hash = manifest_digest
            .strip_prefix("sha256:")
            .unwrap_or(&manifest_digest);
        let manifest_path = blobs_dir.join(manifest_hash);
        fs::write(&manifest_path, &manifest_bytes)?;

        // Parse manifest to get layers and config
        let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)?;

        // Check if this is an index (multi-platform) or a manifest
        let manifest = if manifest.get("manifests").is_some() {
            // It's an index, get the first manifest
            let manifests = manifest
                .get("manifests")
                .and_then(|m| m.as_array())
                .ok_or_else(|| {
                    OciError::Registry("Invalid index: no manifests array".to_owned())
                })?;

            let first = manifests
                .first()
                .ok_or_else(|| OciError::Registry("Empty manifests array".to_owned()))?;

            let nested_digest = first
                .get("digest")
                .and_then(|d| d.as_str())
                .ok_or_else(|| OciError::Registry("Manifest missing digest".to_owned()))?;

            // Pull the actual manifest
            let (_, nested_bytes) = self.pull_blob(image_ref, nested_digest)?;

            // Save nested manifest blob
            let nested_hash = nested_digest
                .strip_prefix("sha256:")
                .unwrap_or(nested_digest);
            let nested_path = blobs_dir.join(nested_hash);
            fs::write(&nested_path, &nested_bytes)?;

            serde_json::from_slice(&nested_bytes)?
        } else {
            manifest
        };

        // Pull config blob
        let config_digest = manifest
            .get("config")
            .and_then(|c| c.get("digest"))
            .and_then(|d| d.as_str())
            .ok_or_else(|| OciError::Registry("Manifest missing config digest".to_owned()))?;

        let (_, config_bytes) = self.pull_blob(image_ref, config_digest)?;
        let config_hash = config_digest
            .strip_prefix("sha256:")
            .unwrap_or(config_digest);
        let config_path = blobs_dir.join(config_hash);
        fs::write(&config_path, &config_bytes)?;

        // Pull layer blobs
        if let Some(layers) = manifest.get("layers").and_then(|l| l.as_array()) {
            for layer in layers {
                let layer_digest = layer
                    .get("digest")
                    .and_then(|d| d.as_str())
                    .ok_or_else(|| OciError::Registry("Layer missing digest".to_owned()))?;

                let layer_hash = layer_digest.strip_prefix("sha256:").unwrap_or(layer_digest);
                let layer_path = blobs_dir.join(layer_hash);

                // Skip if already downloaded
                if layer_path.exists() {
                    continue;
                }

                let (_, layer_bytes) = self.pull_blob(image_ref, layer_digest)?;
                fs::write(&layer_path, &layer_bytes)?;
            }
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
            image_ref.registry, image_ref.repository, image_ref.reference
        );

        let accept = MANIFEST_MEDIA_TYPES.join(", ");

        let mut response = self.authenticated_request(&url, image_ref, &accept)?;

        let bytes = response
            .body_mut()
            .read_to_vec()
            .map_err(|e| OciError::Registry(format!("Failed to read manifest: {e}")))?;

        // Compute digest
        let hash = Sha256::digest(&bytes);
        let computed_digest = format!("sha256:{hash:x}");

        Ok((computed_digest, bytes))
    }

    /// Pull a blob from the registry.
    fn pull_blob(
        &mut self,
        image_ref: &ImageReference,
        digest: &str,
    ) -> Result<(String, Vec<u8>), OciError> {
        let url = format!(
            "https://{}/v2/{}/blobs/{digest}",
            image_ref.registry, image_ref.repository
        );

        let mut response = self.authenticated_request(&url, image_ref, "*/*")?;

        let bytes = response
            .body_mut()
            .read_to_vec()
            .map_err(|e| OciError::Registry(format!("Failed to read blob: {e}")))?;

        // Verify digest
        let hash = Sha256::digest(&bytes);
        let computed = format!("sha256:{hash:x}");

        let expected = digest.to_owned();
        if computed != expected {
            return Err(OciError::DigestMismatch {
                expected,
                actual: computed,
            });
        }

        Ok((digest.to_owned(), bytes))
    }

    /// Make an authenticated request to the registry.
    fn authenticated_request(
        &mut self,
        url: &str,
        image_ref: &ImageReference,
        accept: &str,
    ) -> Result<ureq::http::Response<ureq::Body>, OciError> {
        // Build the scope for this request
        let scope = format!("repository:{}:pull", image_ref.repository);

        // Build request with cached token if available
        let mut request = self.agent.get(url).header("Accept", accept);

        if let Some(token) = self.bearer_tokens.get(&scope) {
            request = request.header("Authorization", &format!("Bearer {token}"));
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
                .header("Authorization", &format!("Bearer {token}"))
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
            request = request.header("Authorization", &format!("Basic {credentials}"));
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
    fn test_parse_simple_image() {
        let ref_ = ImageReference::parse("alpine").unwrap();
        assert_eq!(ref_.registry, "docker.io");
        assert_eq!(ref_.repository, "library/alpine");
        assert_eq!(ref_.reference, "latest");
        assert!(!ref_.is_digest);
    }

    #[test]
    fn test_parse_image_with_tag() {
        let ref_ = ImageReference::parse("alpine:3.18").unwrap();
        assert_eq!(ref_.registry, "docker.io");
        assert_eq!(ref_.repository, "library/alpine");
        assert_eq!(ref_.reference, "3.18");
        assert!(!ref_.is_digest);
    }

    #[test]
    fn test_parse_user_image() {
        let ref_ = ImageReference::parse("myuser/myimage:v1").unwrap();
        assert_eq!(ref_.registry, "docker.io");
        assert_eq!(ref_.repository, "myuser/myimage");
        assert_eq!(ref_.reference, "v1");
    }

    #[test]
    fn test_parse_custom_registry() {
        let ref_ = ImageReference::parse("ghcr.io/owner/repo:latest").unwrap();
        assert_eq!(ref_.registry, "ghcr.io");
        assert_eq!(ref_.repository, "owner/repo");
        assert_eq!(ref_.reference, "latest");
    }

    #[test]
    fn test_parse_registry_with_port() {
        let ref_ = ImageReference::parse("localhost:5000/myimage:v1").unwrap();
        assert_eq!(ref_.registry, "localhost:5000");
        assert_eq!(ref_.repository, "myimage");
        assert_eq!(ref_.reference, "v1");
    }

    #[test]
    fn test_parse_digest() {
        let ref_ = ImageReference::parse("alpine@sha256:abc123").unwrap();
        assert_eq!(ref_.registry, "docker.io");
        assert_eq!(ref_.repository, "library/alpine");
        assert_eq!(ref_.reference, "sha256:abc123");
        assert!(ref_.is_digest);
    }

    #[test]
    fn test_parse_www_authenticate() {
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
