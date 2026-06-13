use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ValidError;

#[derive(Debug, thiserror::Error)]
pub enum ImageRegistryError {
    #[error(
        "External registry '{image_registry}' is not supported. Expected '{expected_registry}' or an unqualified image name. Push your image to the Bencher registry or omit the registry prefix."
    )]
    UnsupportedRegistry {
        image_registry: String,
        expected_registry: String,
    },
}

const DEFAULT_OCI_REGISTRY: &str = "docker.io";

/// A parsed OCI image reference.
///
/// Supports formats like:
/// - `image` -> docker.io/library/image:latest
/// - `image:tag` -> docker.io/library/image:tag
/// - `user/image` -> docker.io/user/image:latest
/// - `registry.com/image` -> registry.com/image:latest
/// - `registry.com/user/image:tag` -> registry.com/user/image:tag
/// - `registry.com/image@sha256:...` -> registry.com/image@sha256:...
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ImageReference {
    raw: String,
    /// Registry host (e.g., "registry.example.com").
    registry: String,
    /// Repository name (e.g., "library/alpine", "myuser/myimage").
    repository: String,
    /// Tag or digest (e.g., "latest", "sha256:abc123...").
    reference: String,
    /// Whether the reference is a digest.
    is_digest: bool,
}

impl ImageReference {
    /// Parse an image reference string.
    pub fn parse(reference: &str) -> Result<Self, ValidError> {
        if reference.is_empty() {
            return Err(ValidError::ImageReference(reference.to_owned()));
        }

        let (name, tag_or_digest, is_digest) =
            if let Some((name, digest)) = reference.split_once('@') {
                (name, digest.to_owned(), true)
            } else if let Some((name, tag)) = reference.rsplit_once(':') {
                // A valid tag never contains `/`. If the suffix after the last
                // colon has a `/`, the colon must be part of a port
                // (e.g. `myregistry:5000/myimage`).
                if tag.contains('/') {
                    (reference, "latest".to_owned(), false)
                // Only treat an all-digit suffix as a port when a `/` is present
                // (e.g. `localhost:5000/image`). Without a `/`, the first component
                // can't be a registry domain, so the digit suffix is a tag
                // (e.g. `myimage:5000` → tag "5000"), matching Docker behavior.
                } else if !name.contains('/') || !tag.chars().all(|c| c.is_ascii_digit()) {
                    (name, tag.to_owned(), false)
                } else {
                    // Has a slash and all-digit suffix: it's a port (e.g. localhost:5000/image)
                    (reference, "latest".to_owned(), false)
                }
            } else {
                (reference, "latest".to_owned(), false)
            };

        // Parse registry and repository
        let (registry, repository) = Self::parse_name(name);

        Ok(Self {
            raw: reference.to_owned(),
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
                (DEFAULT_OCI_REGISTRY.to_owned(), format!("library/{image}"))
            },
            [first, rest]
                if first.contains('.') || first.contains(':') || *first == "localhost" =>
            {
                // Has a registry prefix
                ((*first).to_owned(), (*rest).to_owned())
            },
            _ => {
                // user/image format: docker.io/user/image
                (DEFAULT_OCI_REGISTRY.to_owned(), name.to_owned())
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

    /// Registry host.
    #[must_use]
    pub fn registry(&self) -> &str {
        &self.registry
    }

    /// Repository name.
    #[must_use]
    pub fn repository(&self) -> &str {
        &self.repository
    }

    /// The repository as a candidate project resource ID.
    ///
    /// Bencher registry images are named `[{registry}/]{project}:{tag}`,
    /// so a repository with a single path segment is a project candidate.
    /// The implied `library/` namespace for default-registry images is
    /// ignored, matching Docker semantics where `{name}` and
    /// `library/{name}` are equivalent.
    /// Multi-segment repositories (e.g. `{user}/{image}`) are not supported
    /// by the Bencher registry, so `None` is returned for them.
    #[must_use]
    pub fn project_repository(&self) -> Option<&str> {
        let repository = if self.registry == DEFAULT_OCI_REGISTRY {
            self.repository
                .strip_prefix("library/")
                .unwrap_or(&self.repository)
        } else {
            &self.repository
        };
        (!repository.contains('/')).then_some(repository)
    }

    /// Tag or digest reference.
    #[must_use]
    pub fn reference(&self) -> &str {
        &self.reference
    }

    /// Whether the reference is a digest.
    #[must_use]
    pub fn is_digest(&self) -> bool {
        self.is_digest
    }

    /// Validate that this image's registry is either the default (`docker.io`)
    /// or the expected Bencher registry.
    ///
    /// The expected registry matches either as a bare host (`expected_host`)
    /// or as a full authority (`expected_host:expected_port`), so both
    /// `registry.bencher.dev/...` and `registry.bencher.dev:443/...` are
    /// accepted for a registry served on that host and port. Pass the registry
    /// URL's `port_or_known_default()` as `expected_port` so the scheme's
    /// default port (e.g. 443 for HTTPS) is matched as well as an explicit
    /// port (e.g. a self-hosted `:8443`).
    pub fn validate_registry(
        &self,
        expected_host: &str,
        expected_port: Option<u16>,
    ) -> Result<(), ImageRegistryError> {
        let image_registry = self.registry();
        if image_registry == DEFAULT_OCI_REGISTRY || image_registry == expected_host {
            return Ok(());
        }
        if let Some(port) = expected_port
            && image_registry == format!("{expected_host}:{port}")
        {
            return Ok(());
        }
        Err(ImageRegistryError::UnsupportedRegistry {
            image_registry: image_registry.to_owned(),
            expected_registry: expected_host.to_owned(),
        })
    }
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for ImageReference {
    fn schema_name() -> String {
        "ImageReference".to_owned()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        String::json_schema(generator)
    }
}

impl From<ImageReference> for String {
    fn from(image_reference: ImageReference) -> Self {
        image_reference.raw
    }
}

impl fmt::Display for ImageReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl FromStr for ImageReference {
    type Err = ValidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for ImageReference {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.raw)
    }
}

impl<'de> Deserialize<'de> for ImageReference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_simple_image() {
        let ref_ = ImageReference::parse("alpine").unwrap();
        assert_eq!(ref_.registry(), DEFAULT_OCI_REGISTRY);
        assert_eq!(ref_.repository(), "library/alpine");
        assert_eq!(ref_.reference(), "latest");
        assert!(!ref_.is_digest());
    }

    #[test]
    fn parse_image_with_tag() {
        let ref_ = ImageReference::parse("alpine:3.18").unwrap();
        assert_eq!(ref_.registry(), DEFAULT_OCI_REGISTRY);
        assert_eq!(ref_.repository(), "library/alpine");
        assert_eq!(ref_.reference(), "3.18");
        assert!(!ref_.is_digest());
    }

    #[test]
    fn parse_user_image() {
        let ref_ = ImageReference::parse("myuser/myimage:v1").unwrap();
        assert_eq!(ref_.registry(), DEFAULT_OCI_REGISTRY);
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
        assert_eq!(ref_.registry(), DEFAULT_OCI_REGISTRY);
        assert_eq!(ref_.repository(), "library/alpine");
        assert_eq!(ref_.reference(), "sha256:abc123");
        assert!(ref_.is_digest());
    }

    #[test]
    fn empty_string_fails() {
        ImageReference::parse("").unwrap_err();
    }

    #[test]
    fn project_repository_unqualified() {
        let ref_ = ImageReference::parse("my-project:v1").unwrap();
        assert_eq!(ref_.project_repository(), Some("my-project"));
    }

    #[test]
    fn project_repository_qualified() {
        let ref_ = ImageReference::parse("localhost/my-project:v1").unwrap();
        assert_eq!(ref_.project_repository(), Some("my-project"));
    }

    #[test]
    fn project_repository_qualified_with_port() {
        let ref_ = ImageReference::parse("localhost:5000/my-project:v1").unwrap();
        assert_eq!(ref_.project_repository(), Some("my-project"));
    }

    #[test]
    fn project_repository_explicit_library() {
        let ref_ = ImageReference::parse("library/my-project:v1").unwrap();
        assert_eq!(ref_.project_repository(), Some("my-project"));
    }

    #[test]
    fn project_repository_user_image() {
        let ref_ = ImageReference::parse("myuser/myimage:v1").unwrap();
        assert_eq!(ref_.project_repository(), None);
    }

    #[test]
    fn project_repository_custom_registry_multi_segment() {
        let ref_ = ImageReference::parse("ghcr.io/owner/repo:latest").unwrap();
        assert_eq!(ref_.project_repository(), None);
    }

    #[test]
    fn round_trip_serde() {
        let original = ImageReference::parse("ghcr.io/owner/repo:v1").unwrap();
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: ImageReference = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn from_str_impl() {
        let ref_: ImageReference = "alpine:3.18".parse().unwrap();
        assert_eq!(ref_.registry(), DEFAULT_OCI_REGISTRY);
        assert_eq!(ref_.reference(), "3.18");
    }

    #[test]
    fn display_impl() {
        let ref_ = ImageReference::parse("alpine:3.18").unwrap();
        // Display uses the raw input form, not the expanded full_name()
        assert_eq!(ref_.to_string(), "alpine:3.18");
        assert_eq!(ref_.full_name(), "docker.io/library/alpine:3.18");
    }

    #[test]
    fn parse_bare_name_with_numeric_tag() {
        let ref_ = ImageReference::parse("myregistry:5000").unwrap();
        assert_eq!(ref_.registry(), DEFAULT_OCI_REGISTRY);
        assert_eq!(ref_.repository(), "library/myregistry");
        assert_eq!(ref_.reference(), "5000");
        assert!(!ref_.is_digest());
    }

    #[test]
    fn parse_bare_name_with_numeric_tag_full_name() {
        let ref_ = ImageReference::parse("myregistry:5000").unwrap();
        assert_eq!(ref_.full_name(), "docker.io/library/myregistry:5000");
    }

    #[test]
    fn parse_registry_port_with_path() {
        let ref_ = ImageReference::parse("myregistry:5000/myimage").unwrap();
        assert_eq!(ref_.registry(), "myregistry:5000");
        assert_eq!(ref_.repository(), "myimage");
        assert_eq!(ref_.reference(), "latest");
    }

    #[test]
    fn parse_registry_port_with_path_and_tag() {
        let ref_ = ImageReference::parse("myregistry:5000/myimage:v2").unwrap();
        assert_eq!(ref_.registry(), "myregistry:5000");
        assert_eq!(ref_.repository(), "myimage");
        assert_eq!(ref_.reference(), "v2");
    }

    #[test]
    fn parse_localhost_with_port_no_path() {
        let ref_ = ImageReference::parse("localhost:5000").unwrap();
        assert_eq!(ref_.registry(), DEFAULT_OCI_REGISTRY);
        assert_eq!(ref_.repository(), "library/localhost");
        assert_eq!(ref_.reference(), "5000");
    }

    #[test]
    fn parse_dotted_registry_with_port_no_path() {
        let ref_ = ImageReference::parse("registry.io:5000").unwrap();
        assert_eq!(ref_.registry(), DEFAULT_OCI_REGISTRY);
        assert_eq!(ref_.repository(), "library/registry.io");
        assert_eq!(ref_.reference(), "5000");
    }

    #[test]
    fn validate_registry_default_ok() {
        // Images from docker.io are always accepted
        let ref_ = ImageReference::parse("alpine:3.18").unwrap();
        ref_.validate_registry("registry.bencher.dev", Some(443))
            .unwrap();
    }

    #[test]
    fn validate_registry_expected_ok() {
        // Images from the expected registry (bare host) are accepted
        let ref_ = ImageReference::parse("registry.bencher.dev/owner/repo:v1").unwrap();
        ref_.validate_registry("registry.bencher.dev", Some(443))
            .unwrap();
    }

    #[test]
    fn validate_registry_default_port_ok() {
        // The scheme's default port (443 for HTTPS) is accepted explicitly
        let ref_ = ImageReference::parse("registry.bencher.dev:443/owner/repo:v1").unwrap();
        ref_.validate_registry("registry.bencher.dev", Some(443))
            .unwrap();
    }

    #[test]
    fn validate_registry_custom_port_ok() {
        // A self-hosted registry on a non-default port accepts the host:port form
        let ref_ = ImageReference::parse("localhost:61016/my-project:v1").unwrap();
        ref_.validate_registry("localhost", Some(61016)).unwrap();
    }

    #[test]
    fn validate_registry_bare_host_for_custom_port_ok() {
        // The bare host is accepted even when the registry serves on a non-default port
        let ref_ = ImageReference::parse("bencher.example.com/my-project:v1").unwrap();
        ref_.validate_registry("bencher.example.com", Some(8443))
            .unwrap();
    }

    #[test]
    fn validate_registry_unsupported() {
        // Images from an external registry are rejected
        let ref_ = ImageReference::parse("ghcr.io/owner/repo:v1").unwrap();
        let err = ref_
            .validate_registry("registry.bencher.dev", Some(443))
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("ghcr.io"),
            "Error should mention the image registry: {msg}"
        );
        assert!(
            msg.contains("registry.bencher.dev"),
            "Error should mention the expected registry: {msg}"
        );
    }

    #[test]
    fn validate_registry_wrong_port_unsupported() {
        // The right host on the wrong port is rejected
        let ref_ = ImageReference::parse("registry.bencher.dev:8080/owner/repo:v1").unwrap();
        ref_.validate_registry("registry.bencher.dev", Some(443))
            .unwrap_err();
    }

    #[test]
    fn validate_registry_user_image_ok() {
        // user/image format defaults to docker.io, which is allowed
        let ref_ = ImageReference::parse("myuser/myimage:v1").unwrap();
        ref_.validate_registry("registry.bencher.dev", Some(443))
            .unwrap();
    }
}
